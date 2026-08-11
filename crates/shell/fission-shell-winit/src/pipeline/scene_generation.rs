use super::*;

pub(super) fn generate_render_layer_recursive(
    node_id: WidgetId,
    ir: &CoreIR,
    snapshot: &LayoutSnapshot,
    scroll_map: &ScrollStateMap,
    animation_map: &MotionStateMap,
    paint_cache: &mut HashMap<WidgetId, (u64, DisplayList)>,
    boundary_cache: &mut HashMap<WidgetId, BoundaryCacheEntry>,
    runtime_dynamic_subtrees: &HashMap<WidgetId, bool>,
    miss_count: &mut usize,
    hit_count: &mut usize,
    scene_cache_allowed: bool,
    visited: &mut HashSet<WidgetId>,
    bindings: &mut RetainedDynamicOps,
    layer_path: Vec<usize>,
) -> Option<RenderLayer> {
    if !visited.insert(node_id) {
        return None;
    }

    let (Some(node), Some(geom)) = (ir.nodes.get(&node_id), snapshot.nodes.get(&node_id)) else {
        return None;
    };

    let rect = geom.rect;
    let can_use_boundary_cache = !runtime_dynamic_subtrees
        .get(&node_id)
        .copied()
        .unwrap_or(false);

    let scene_cache_key = boundary_hash(node, rect);
    let can_cache_scene = scene_cache_allowed && can_use_boundary_cache && node.parent.is_some();
    if can_cache_scene {
        if let Some(entry) = boundary_cache.get(&node_id) {
            if entry.hash == scene_cache_key {
                *hit_count += 1;
                return Some(entry.layer.clone());
            }
        }
    } else if can_use_boundary_cache {
        if let Some(entry) = boundary_cache.get(&node_id) {
            if entry.hash == scene_cache_key {
                *hit_count += 1;
                return Some(entry.layer.clone());
            }
        }
    }

    let composite_opacity = resolve_composite_scalar(
        node.composite.opacity.as_ref(),
        animation_map,
        MotionPropertyId::Opacity,
    );
    let composite_tx = resolve_composite_scalar(
        node.composite.translate_x.as_ref(),
        animation_map,
        MotionPropertyId::TranslateX,
    );
    let composite_ty = resolve_composite_scalar(
        node.composite.translate_y.as_ref(),
        animation_map,
        MotionPropertyId::TranslateY,
    );
    let composite_scale = resolve_composite_scalar(
        node.composite.scale.as_ref(),
        animation_map,
        MotionPropertyId::Scale,
    )
    .unwrap_or(1.0);
    let composite_rotation = resolve_composite_scalar(
        node.composite.rotation.as_ref(),
        animation_map,
        MotionPropertyId::Rotation,
    )
    .unwrap_or(0.0);

    let _has_composite_transform = composite_tx.unwrap_or(0.0).abs() > 0.001
        || composite_ty.unwrap_or(0.0).abs() > 0.001
        || (composite_scale - 1.0).abs() > 0.001
        || composite_rotation.abs() > 0.001;
    let has_opacity_layer = composite_opacity
        .map(|value| (value - 1.0).abs() > 0.001)
        .unwrap_or(false);
    let needs_dynamic_opacity = node
        .composite
        .opacity
        .as_ref()
        .and_then(|value| value.motion_target)
        .is_some();
    let needs_dynamic_transform = node
        .composite
        .translate_x
        .as_ref()
        .and_then(|value| value.motion_target)
        .is_some()
        || node
            .composite
            .translate_y
            .as_ref()
            .and_then(|value| value.motion_target)
            .is_some()
        || node
            .composite
            .scale
            .as_ref()
            .and_then(|value| value.motion_target)
            .is_some()
        || node
            .composite
            .rotation
            .as_ref()
            .and_then(|value| value.motion_target)
            .is_some();
    let emit_opacity_layer = has_opacity_layer || needs_dynamic_opacity;
    let has_runtime_clip = node.composite.clip_to_bounds;
    let scroll = match &node.op {
        Op::Layout(LayoutOp::Scroll { direction, .. }) => Some(ScrollTransform {
            node_id,
            direction: *direction,
        }),
        _ => None,
    };
    let layout_transform = match &node.op {
        Op::Layout(LayoutOp::Transform { transform }) => Some(*transform),
        _ => None,
    };
    let has_own_transform = needs_dynamic_transform || layout_transform.is_some();
    let has_dynamic_transform = has_own_transform || scroll.is_some();
    let has_dynamic_style = emit_opacity_layer || has_dynamic_transform || has_runtime_clip;
    let has_dynamic_children = node.children.iter().any(|child| {
        runtime_dynamic_subtrees
            .get(child)
            .copied()
            .unwrap_or(false)
    });
    let mut layer = RenderLayer::new(rect);
    layer.node_id = Some(node_id);
    if can_cache_scene {
        layer.style.cache_key = Some(scene_cache_key);
    } else if has_dynamic_style && !has_dynamic_children {
        layer.style.content_cache_key = Some(scene_cache_key ^ 0x9E37_79B9_7F4A_7C15);
    }

    layer.style.clip = match &node.op {
        Op::Layout(LayoutOp::Scroll { .. }) | Op::Layout(LayoutOp::Clip { .. }) => {
            Some(LayerClip::Rect(rect))
        }
        _ if has_runtime_clip => Some(LayerClip::Rect(rect)),
        _ => None,
    };
    if emit_opacity_layer {
        layer.style.opacity = composite_opacity.unwrap_or(1.0);
    }

    if let Some(transform) = compose_dynamic_layer_transform(
        &TransformBinding {
            layer_path: layer_path.clone(),
            rect,
            layout_transform,
            scroll: None,
            translate_x: node.composite.translate_x.clone(),
            translate_y: node.composite.translate_y.clone(),
            scale: node.composite.scale.clone(),
            rotation: node.composite.rotation.clone(),
        },
        scroll_map,
        animation_map,
    ) {
        layer.style.transform = Some(transform);
    }

    let local_hash = local_paint_hash(node);
    let local_paint = if let Some((cached_hash, cached_ops)) = paint_cache.get(&node_id) {
        if *cached_hash == local_hash {
            *hit_count += 1;
            Some(cached_ops.clone())
        } else {
            *miss_count += 1;
            let ops = build_local_paint_list(ir, node_id, node, rect);
            if let Some(ops) = ops.clone() {
                paint_cache.insert(node_id, (local_hash, ops));
            } else {
                paint_cache.remove(&node_id);
            }
            ops
        }
    } else {
        *miss_count += 1;
        let ops = build_local_paint_list(ir, node_id, node, rect);
        if let Some(ops) = ops.clone() {
            paint_cache.insert(node_id, (local_hash, ops));
        }
        ops
    };

    if let Some(local_paint) = local_paint {
        layer.children.push(RenderNode::Paint(local_paint));
    }

    if needs_dynamic_opacity {
        if let Some(scalar) = node.composite.opacity.as_ref() {
            bindings.opacity.push(OpacityBinding {
                layer_path: layer_path.clone(),
                scalar: scalar.clone(),
            });
        }
    }
    if has_own_transform {
        bindings.transform.push(TransformBinding {
            layer_path: layer_path.clone(),
            rect,
            layout_transform,
            scroll: None,
            translate_x: node.composite.translate_x.clone(),
            translate_y: node.composite.translate_y.clone(),
            scale: node.composite.scale.clone(),
            rotation: node.composite.rotation.clone(),
        });
    }

    if let Some(scroll) = scroll {
        let content_index = layer.children.len();
        let mut content_path = layer_path.clone();
        content_path.push(content_index);
        let mut content_layer = RenderLayer::new(rect);
        content_layer.style.transform = compose_dynamic_layer_transform(
            &TransformBinding {
                layer_path: content_path.clone(),
                rect,
                layout_transform: None,
                scroll: Some(scroll.clone()),
                translate_x: None,
                translate_y: None,
                scale: None,
                rotation: None,
            },
            scroll_map,
            animation_map,
        );
        content_layer.style.transform_clip = false;
        bindings.transform.push(TransformBinding {
            layer_path: content_path.clone(),
            rect,
            layout_transform: None,
            scroll: Some(scroll),
            translate_x: None,
            translate_y: None,
            scale: None,
            rotation: None,
        });

        for child in &node.children {
            let child_index = content_layer.children.len();
            let mut child_path = content_path.clone();
            child_path.push(child_index);
            if let Some(child_layer) = generate_render_layer_recursive(
                *child,
                ir,
                snapshot,
                scroll_map,
                animation_map,
                paint_cache,
                boundary_cache,
                runtime_dynamic_subtrees,
                miss_count,
                hit_count,
                scene_cache_allowed,
                visited,
                bindings,
                child_path,
            ) {
                content_layer.children.push(RenderNode::Layer(child_layer));
            }
        }

        if !content_layer.children.is_empty() {
            layer.children.push(RenderNode::Layer(content_layer));
        }
    } else {
        for child in &node.children {
            let child_index = layer.children.len();
            let mut child_path = layer_path.clone();
            child_path.push(child_index);
            if let Some(child_layer) = generate_render_layer_recursive(
                *child,
                ir,
                snapshot,
                scroll_map,
                animation_map,
                paint_cache,
                boundary_cache,
                runtime_dynamic_subtrees,
                miss_count,
                hit_count,
                scene_cache_allowed,
                visited,
                bindings,
                child_path,
            ) {
                layer.children.push(RenderNode::Layer(child_layer));
            }
        }
    }

    if let Some(scrollbar) = build_scrollbar_paint(ir, node_id, snapshot, scroll_map) {
        let mut scrollbar_path = layer_path.clone();
        scrollbar_path.push(layer.children.len());
        layer.children.push(RenderNode::Paint(scrollbar));
        bindings.scrollbar.push(ScrollbarBinding {
            node_path: scrollbar_path,
            node_id,
        });
    }

    if can_use_boundary_cache {
        boundary_cache.insert(
            node_id,
            BoundaryCacheEntry {
                hash: scene_cache_key,
                layer: layer.clone(),
            },
        );
    }

    Some(layer)
}

pub(super) fn push_video_surface(
    video_surfaces: &mut Vec<VideoSurfaceFrame>,
    widget_id: WidgetId,
    rect: LayoutRect,
    visible_rect: LayoutRect,
    transform: Option<[f32; 16]>,
    opacity: f32,
    paint_order: u32,
    video_map: &VideoStateMap,
) {
    if let Some(state) = video_map.states.get(&widget_id) {
        let surface_id = state.surface_id.unwrap_or(0);
        video_surfaces.push(VideoSurfaceFrame {
            widget_id,
            surface_id,
            rect,
            visible_rect,
            transform,
            opacity,
            paint_order,
        });
    }
}

pub(super) fn push_web_surface(
    web_surfaces: &mut Vec<WebSurfaceFrame>,
    widget_id: WidgetId,
    rect: LayoutRect,
    visible_rect: LayoutRect,
    transform: Option<[f32; 16]>,
    opacity: f32,
    paint_order: u32,
    web_map: &WebStateMap,
) {
    if let Some(state) = web_map.states.get(&widget_id) {
        if !state.url.trim().is_empty() {
            web_surfaces.push(WebSurfaceFrame {
                widget_id,
                url: state.url.clone(),
                user_agent: state.user_agent.clone(),
                rect,
                visible_rect,
                transform,
                opacity,
                paint_order,
            });
        }
    }
}

/// Computes the intersection of two rectangles, returning `None` when they do
/// not overlap at all.
pub(super) fn intersect_rects(a: LayoutRect, b: LayoutRect) -> Option<LayoutRect> {
    let x = a.x().max(b.x());
    let y = a.y().max(b.y());
    let right = a.right().min(b.right());
    let bottom = a.bottom().min(b.bottom());
    if right > x && bottom > y {
        Some(LayoutRect::new(x, y, right - x, bottom - y))
    } else {
        None
    }
}

pub(super) fn transform_rect_bounds(rect: LayoutRect, transform: Option<[f32; 16]>) -> LayoutRect {
    let Some(matrix) = transform else {
        return rect;
    };
    let points = [
        (rect.x(), rect.y()),
        (rect.right(), rect.y()),
        (rect.right(), rect.bottom()),
        (rect.x(), rect.bottom()),
    ];
    let mut min_x = f32::INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    for (x, y) in points {
        let transformed_x = matrix[0] * x + matrix[4] * y + matrix[12];
        let transformed_y = matrix[1] * x + matrix[5] * y + matrix[13];
        min_x = min_x.min(transformed_x);
        min_y = min_y.min(transformed_y);
        max_x = max_x.max(transformed_x);
        max_y = max_y.max(transformed_y);
    }
    LayoutRect::new(min_x, min_y, max_x - min_x, max_y - min_y)
}

pub(super) fn collect_video_surfaces(
    node_id: WidgetId,
    ir: &CoreIR,
    snapshot: &LayoutSnapshot,
    video_map: &VideoStateMap,
    web_map: &WebStateMap,
    scroll_map: &ScrollStateMap,
    animation_map: &MotionStateMap,
    accumulated_offset: LayoutPoint,
    accumulated_clip: LayoutRect,
    accumulated_transform: Option<[f32; 16]>,
    accumulated_opacity: f32,
    paint_order: &mut u32,
    video_surfaces: &mut Vec<VideoSurfaceFrame>,
    web_surfaces: &mut Vec<WebSurfaceFrame>,
    native_surfaces: &mut Vec<NativeSurfaceFrame>,
) {
    let mut visited = HashSet::new();
    collect_video_surfaces_with_visited(
        node_id,
        ir,
        snapshot,
        video_map,
        web_map,
        scroll_map,
        animation_map,
        accumulated_offset,
        accumulated_clip,
        accumulated_transform,
        accumulated_opacity,
        paint_order,
        video_surfaces,
        web_surfaces,
        native_surfaces,
        &mut visited,
    );
}

pub(super) fn collect_video_surfaces_with_visited(
    node_id: WidgetId,
    ir: &CoreIR,
    snapshot: &LayoutSnapshot,
    video_map: &VideoStateMap,
    web_map: &WebStateMap,
    scroll_map: &ScrollStateMap,
    animation_map: &MotionStateMap,
    accumulated_offset: LayoutPoint,
    accumulated_clip: LayoutRect,
    accumulated_transform: Option<[f32; 16]>,
    accumulated_opacity: f32,
    paint_order: &mut u32,
    video_surfaces: &mut Vec<VideoSurfaceFrame>,
    web_surfaces: &mut Vec<WebSurfaceFrame>,
    native_surfaces: &mut Vec<NativeSurfaceFrame>,
    visited: &mut HashSet<WidgetId>,
) {
    if !visited.insert(node_id) {
        return;
    }
    if let (Some(node), Some(geom)) = (ir.nodes.get(&node_id), snapshot.nodes.get(&node_id)) {
        let mut child_offset = accumulated_offset;
        let mut child_clip = accumulated_clip;
        let translated_node_rect = translate_rect(geom.rect, accumulated_offset);
        let node_transform = compose_dynamic_layer_transform(
            &TransformBinding {
                layer_path: Vec::new(),
                rect: translated_node_rect,
                layout_transform: match &node.op {
                    Op::Layout(LayoutOp::Transform { transform }) => Some(*transform),
                    _ => None,
                },
                scroll: None,
                translate_x: node.composite.translate_x.clone(),
                translate_y: node.composite.translate_y.clone(),
                scale: node.composite.scale.clone(),
                rotation: node.composite.rotation.clone(),
            },
            scroll_map,
            animation_map,
        );
        let effective_transform = match node_transform {
            Some(transform) => append_transform(accumulated_transform, transform),
            None => accumulated_transform,
        };
        let node_opacity = node
            .composite
            .opacity
            .as_ref()
            .map(|opacity| resolve_scalar_value(opacity, animation_map, MotionPropertyId::Opacity))
            .unwrap_or(1.0);
        let effective_opacity = (accumulated_opacity * node_opacity).clamp(0.0, 1.0);

        // Scroll nodes shift children and clip to the scroll viewport.
        if let Op::Layout(LayoutOp::Scroll { direction, .. }) = &node.op {
            let offset = scroll_map.get_offset(node_id);
            child_offset = match direction {
                fission_ir::FlexDirection::Row => {
                    LayoutPoint::new(accumulated_offset.x - offset, accumulated_offset.y)
                }
                fission_ir::FlexDirection::Column => {
                    LayoutPoint::new(accumulated_offset.x, accumulated_offset.y - offset)
                }
            };
            let viewport_rect = transform_rect_bounds(translated_node_rect, effective_transform);
            child_clip = intersect_rects(child_clip, viewport_rect).unwrap_or(LayoutRect::new(
                viewport_rect.x(),
                viewport_rect.y(),
                0.0,
                0.0,
            ));
        }

        // Clip nodes restrict children to their bounds.
        if matches!(&node.op, Op::Layout(LayoutOp::Clip { .. })) {
            let clip_rect = transform_rect_bounds(translated_node_rect, effective_transform);
            child_clip = intersect_rects(child_clip, clip_rect).unwrap_or(LayoutRect::new(
                clip_rect.x(),
                clip_rect.y(),
                0.0,
                0.0,
            ));
        }

        // clip_to_bounds restricts children to this node's bounds.
        if node.composite.clip_to_bounds {
            let bounds_rect = transform_rect_bounds(translated_node_rect, effective_transform);
            child_clip = intersect_rects(child_clip, bounds_rect).unwrap_or(LayoutRect::new(
                bounds_rect.x(),
                bounds_rect.y(),
                0.0,
                0.0,
            ));
        }

        if let Op::Layout(LayoutOp::Embed {
            kind: EmbedKind::Video,
            widget_id,
            ..
        }) = &node.op
        {
            let translated_rect = transform_rect_bounds(translated_node_rect, effective_transform);
            if effective_opacity > 0.0 {
                if let Some(visible_rect) = intersect_rects(translated_rect, accumulated_clip) {
                    let order = *paint_order;
                    *paint_order += 1;
                    push_video_surface(
                        video_surfaces,
                        *widget_id,
                        translated_rect,
                        visible_rect,
                        effective_transform,
                        effective_opacity,
                        order,
                        video_map,
                    );
                }
            }
        } else if let Op::Layout(LayoutOp::Embed {
            kind: EmbedKind::Web,
            widget_id,
            ..
        }) = &node.op
        {
            let translated_rect = transform_rect_bounds(translated_node_rect, effective_transform);
            if effective_opacity > 0.0 {
                if let Some(visible_rect) = intersect_rects(translated_rect, accumulated_clip) {
                    let order = *paint_order;
                    *paint_order += 1;
                    push_web_surface(
                        web_surfaces,
                        *widget_id,
                        translated_rect,
                        visible_rect,
                        effective_transform,
                        effective_opacity,
                        order,
                        web_map,
                    );
                }
            }
        } else if let Op::Layout(LayoutOp::Embed {
            kind: EmbedKind::Custom(payload),
            widget_id,
            ..
        }) = &node.op
        {
            let translated_rect = transform_rect_bounds(translated_node_rect, effective_transform);
            if effective_opacity > 0.0 {
                if let Some(visible) = intersect_rects(translated_rect, accumulated_clip) {
                    let order = *paint_order;
                    *paint_order += 1;
                    native_surfaces.push(NativeSurfaceFrame {
                        widget_id: *widget_id,
                        rect: translated_rect,
                        payload: payload.clone(),
                        visible_rect: visible,
                        transform: effective_transform,
                        opacity: effective_opacity,
                        paint_order: order,
                    });
                }
            }
            // Fully clipped — omit entirely. The handler will receive an
            // empty slice via `present_surfaces`, which already means "hide."
        }

        for child in &node.children {
            collect_video_surfaces_with_visited(
                *child,
                ir,
                snapshot,
                video_map,
                web_map,
                scroll_map,
                animation_map,
                child_offset,
                child_clip,
                effective_transform,
                effective_opacity,
                paint_order,
                video_surfaces,
                web_surfaces,
                native_surfaces,
                visited,
            );
        }
    }
}

pub(super) fn local_paint_hash(node: &fission_ir::CoreNode) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    node.op.hash(&mut hasher);
    hasher.finish()
}

pub(super) fn boundary_hash(node: &fission_ir::CoreNode, rect: LayoutRect) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    node.hash.hash(&mut hasher);
    rect.origin.x.to_bits().hash(&mut hasher);
    rect.origin.y.to_bits().hash(&mut hasher);
    rect.size.width.to_bits().hash(&mut hasher);
    rect.size.height.to_bits().hash(&mut hasher);
    hasher.finish()
}

pub(super) fn build_local_paint_list(
    ir: &CoreIR,
    node_id: WidgetId,
    node: &fission_ir::CoreNode,
    rect: LayoutRect,
) -> Option<DisplayList> {
    let mut list = DisplayList::new(rect);
    match &node.op {
        Op::Paint(fission_ir::PaintOp::BackdropFilter {
            filter,
            corner_radius,
        }) => {
            list.push(DisplayOp::BackdropFilter {
                rect,
                filter: *filter,
                corner_radius: *corner_radius,
                bounds: rect,
                node_id: Some(node_id),
            });
        }
        Op::Paint(fission_ir::PaintOp::DrawRect {
            fill,
            stroke,
            corner_radius,
            shadow,
        }) => {
            let bounds = shadow
                .as_ref()
                .filter(|shadow| !shadow.inset)
                .map(|shadow| box_shadow_bounds(rect, shadow))
                .unwrap_or(rect);
            list.bounds = bounds;
            list.push(DisplayOp::DrawRect {
                rect,
                fill: fill.as_ref().map(map_fill),
                stroke: stroke.as_ref().map(map_stroke),
                corner_radius: *corner_radius,
                shadow: shadow.as_ref().map(|s| BoxShadow {
                    color: RenderColor {
                        r: s.color.r,
                        g: s.color.g,
                        b: s.color.b,
                        a: s.color.a,
                    },
                    blur_radius: s.blur_radius,
                    spread_radius: s.spread_radius,
                    offset: s.offset,
                    inset: s.inset,
                }),
                bounds,
                node_id: Some(node_id),
            });
        }
        Op::Paint(fission_ir::PaintOp::DrawText {
            text,
            size,
            color,
            underline,
            wrap,
            caret_index,
            caret_color,
            caret_width,
            caret_height,
            caret_radius,
            paragraph_style,
        }) => {
            list.push(DisplayOp::DrawText {
                text: text.clone(),
                position: rect.origin,
                size: *size,
                color: RenderColor {
                    r: color.r,
                    g: color.g,
                    b: color.b,
                    a: color.a,
                },
                bounds: rect,
                node_id: Some(node_id),
                underline: *underline,
                wrap: *wrap,
                caret_index: *caret_index,
                caret_color: caret_color.map(|color| RenderColor {
                    r: color.r,
                    g: color.g,
                    b: color.b,
                    a: color.a,
                }),
                caret_width: *caret_width,
                caret_height: *caret_height,
                caret_radius: *caret_radius,
                paragraph_style: *paragraph_style,
            });
        }
        Op::Paint(fission_ir::PaintOp::DrawRichText {
            runs,
            wrap,
            caret_index,
            caret_color,
            caret_width,
            caret_height,
            caret_radius,
            paragraph_style,
        }) => {
            let annotations = ir
                .custom_render_objects
                .get(&node_id)
                .and_then(|sidecar| {
                    sidecar.downcast_ref::<Vec<fission_ir::op::RichTextAnnotation>>()
                })
                .cloned()
                .unwrap_or_default();
            let render_runs = runs
                .iter()
                .map(|r| fission_render::TextRun {
                    text: r.text.clone(),
                    style: fission_render::TextStyle {
                        font_size: r.style.font_size,
                        color: RenderColor {
                            r: r.style.color.r,
                            g: r.style.color.g,
                            b: r.style.color.b,
                            a: r.style.color.a,
                        },
                        underline: r.style.underline,
                        font_family: r.style.font_family.clone(),
                        locale: r.style.locale.clone(),
                        font_weight: r.style.font_weight,
                        font_style: r.style.font_style,
                        line_height: r.style.line_height,
                        letter_spacing: r.style.letter_spacing,
                        background_color: r.style.background_color.map(|c| RenderColor {
                            r: c.r,
                            g: c.g,
                            b: c.b,
                            a: c.a,
                        }),
                    },
                })
                .collect();

            list.push(DisplayOp::DrawRichText {
                runs: render_runs,
                position: rect.origin,
                bounds: rect,
                node_id: Some(node_id),
                wrap: *wrap,
                caret_index: *caret_index,
                caret_color: caret_color.map(|color| RenderColor {
                    r: color.r,
                    g: color.g,
                    b: color.b,
                    a: color.a,
                }),
                caret_width: *caret_width,
                caret_height: *caret_height,
                caret_radius: *caret_radius,
                paragraph_style: *paragraph_style,
                annotations,
            });
        }
        Op::Paint(fission_ir::PaintOp::DrawImage {
            request,
            fit,
            alignment,
        }) => {
            list.push(DisplayOp::DrawImage {
                rect,
                request: request.clone(),
                fit: match fit {
                    fission_ir::op::ImageFit::Contain => fission_render::ImageFit::Contain,
                    fission_ir::op::ImageFit::Cover => fission_render::ImageFit::Cover,
                    fission_ir::op::ImageFit::Fill => fission_render::ImageFit::Fill,
                    fission_ir::op::ImageFit::None => fission_render::ImageFit::None,
                },
                alignment: *alignment,
                bounds: rect,
                node_id: Some(node_id),
            });
        }
        Op::Paint(fission_ir::PaintOp::DrawPath { path, fill, stroke }) => {
            list.push(DisplayOp::DrawPath {
                path: path.clone(),
                fill: fill.as_ref().map(map_fill),
                stroke: stroke.as_ref().map(map_stroke),
                bounds: rect,
                node_id: Some(node_id),
            });
        }
        Op::Paint(fission_ir::PaintOp::DrawSvg {
            content,
            fill,
            stroke,
        }) => {
            list.push(DisplayOp::DrawSvg {
                content: content.clone(),
                fill: fill.as_ref().map(map_fill),
                stroke: stroke.as_ref().map(map_stroke),
                bounds: rect,
                node_id: Some(node_id),
            });
        }
        Op::Layout(LayoutOp::Embed {
            kind, widget_id, ..
        }) => {
            list.push(DisplayOp::DrawSurface {
                rect,
                surface_id: embed_surface_id(kind, *widget_id),
                position: 0,
                bounds: rect,
                node_id: Some(node_id),
            });
        }
        _ => {}
    }
    if list.ops.is_empty() {
        None
    } else {
        Some(list)
    }
}

pub(super) fn box_shadow_bounds(
    rect: LayoutRect,
    shadow: &fission_ir::op::BoxShadow,
) -> LayoutRect {
    let extent = (shadow.blur_radius.max(0.0) + shadow.spread_radius).max(0.0);
    let shadow_left = rect.x() + shadow.offset.0 - extent;
    let shadow_top = rect.y() + shadow.offset.1 - extent;
    let shadow_right = rect.right() + shadow.offset.0 + extent;
    let shadow_bottom = rect.bottom() + shadow.offset.1 + extent;
    let left = rect.x().min(shadow_left);
    let top = rect.y().min(shadow_top);
    let right = rect.right().max(shadow_right);
    let bottom = rect.bottom().max(shadow_bottom);
    LayoutRect::new(left, top, right - left, bottom - top)
}

pub(super) fn build_scrollbar_paint(
    ir: &CoreIR,
    node_id: WidgetId,
    snapshot: &LayoutSnapshot,
    scroll_map: &ScrollStateMap,
) -> Option<DisplayList> {
    let geometry = scrollbar_geometry_for_node(ir, snapshot, scroll_map, node_id)?;
    let rail_fill = Some(Fill::Solid(RenderColor {
        r: 160,
        g: 168,
        b: 180,
        a: 80,
    }));
    let thumb_fill = Some(Fill::Solid(RenderColor {
        r: 82,
        g: 91,
        b: 108,
        a: 190,
    }));
    let mut list = DisplayList::new(geometry.rail_rect);
    let corner_radius = fission_core::scrollbar::SCROLLBAR_THICKNESS / 2.0;

    list.push(DisplayOp::DrawRect {
        rect: geometry.rail_rect,
        fill: rail_fill,
        stroke: None,
        corner_radius,
        shadow: None,
        bounds: geometry.rail_rect,
        node_id: Some(node_id),
    });
    list.push(DisplayOp::DrawRect {
        rect: geometry.thumb_rect,
        fill: thumb_fill,
        stroke: None,
        corner_radius,
        shadow: None,
        bounds: geometry.thumb_rect,
        node_id: Some(node_id),
    });

    Some(list)
}

pub(super) fn resolve_composite_scalar(
    scalar: Option<&fission_ir::CompositeScalar>,
    animation_map: &MotionStateMap,
    property: MotionPropertyId,
) -> Option<f32> {
    let scalar = scalar?;
    Some(resolve_scalar_value(scalar, animation_map, property))
}

pub(super) fn resolve_scalar_value(
    scalar: &fission_ir::CompositeScalar,
    animation_map: &MotionStateMap,
    property: MotionPropertyId,
) -> f32 {
    scalar
        .motion_target
        .map(|target| animation_map.scalar_value(target, property))
        .unwrap_or(scalar.base)
}

pub(super) fn composite_transform_matrix(
    rect: LayoutRect,
    translate_x: f32,
    translate_y: f32,
    scale: f32,
    rotation: f32,
) -> [f32; 16] {
    let center_x = rect.origin.x + rect.size.width * 0.5;
    let center_y = rect.origin.y + rect.size.height * 0.5;

    let to_center = translation_matrix(center_x, center_y);
    let from_center = translation_matrix(-center_x, -center_y);
    let scale_matrix = scale_matrix(scale);
    let rotation_matrix = rotation_z_matrix(rotation);
    let motion_translate = translation_matrix(translate_x, translate_y);

    multiply_matrix(
        motion_translate,
        multiply_matrix(
            to_center,
            multiply_matrix(rotation_matrix, multiply_matrix(scale_matrix, from_center)),
        ),
    )
}

pub(super) fn translation_matrix(tx: f32, ty: f32) -> [f32; 16] {
    [
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, tx, ty, 0.0, 1.0,
    ]
}

pub(super) fn scale_matrix(scale: f32) -> [f32; 16] {
    [
        scale, 0.0, 0.0, 0.0, 0.0, scale, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ]
}

pub(super) fn rotation_z_matrix(radians: f32) -> [f32; 16] {
    let sin = radians.sin();
    let cos = radians.cos();
    [
        cos, sin, 0.0, 0.0, -sin, cos, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ]
}

pub(super) fn multiply_matrix(a: [f32; 16], b: [f32; 16]) -> [f32; 16] {
    let mut out = [0.0; 16];
    for row in 0..4 {
        for col in 0..4 {
            let mut sum = 0.0;
            for k in 0..4 {
                sum += a[row * 4 + k] * b[k * 4 + col];
            }
            out[row * 4 + col] = sum;
        }
    }
    out
}

pub(super) fn is_identity_matrix(matrix: &[f32; 16]) -> bool {
    const IDENTITY: [f32; 16] = [
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ];
    matrix
        .iter()
        .zip(IDENTITY.iter())
        .all(|(lhs, rhs)| (*lhs - *rhs).abs() <= 0.000_1)
}

#[cfg(test)]
pub(super) fn scroll_offsets_changed(
    prev: &HashMap<WidgetId, u32>,
    scroll_map: &ScrollStateMap,
) -> bool {
    if prev.len() != scroll_map.offsets.len() {
        return true;
    }

    scroll_map
        .offsets
        .iter()
        .any(|(id, offset)| prev.get(id).copied() != Some(offset.to_bits()))
}
