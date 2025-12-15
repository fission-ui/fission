use anyhow::Result;
use fission_core::diff::diff_ir;
use fission_core::env::VideoStateMap;
use fission_core::lowering::{build_layout_tree, LoweringContext};
use fission_core::{LayoutPoint, ScrollStateMap};
use fission_ir::op::EmbedKind;
use fission_ir::{CoreIR, NodeId, WidgetNodeId};
use fission_layout::{LayoutEngine, LayoutRect, LayoutSize, LayoutSnapshot};
use fission_render::{
    BoxShadow, Color as RenderColor, DisplayList, DisplayOp, Fill, ImageFit, Renderer, Stroke,
};
use fission_shell::VideoSurfaceFrame;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

pub struct Pipeline {
    pub prev_ir: Option<CoreIR>,
    pub last_snapshot: Option<LayoutSnapshot>,
    pub paint_cache: HashMap<NodeId, (u64, Vec<DisplayOp>)>,
    video_surfaces: Vec<VideoSurfaceFrame>,
}

#[derive(Debug, Default)]
pub struct PipelineStats {
    pub dirty_nodes: usize,
    pub layout_updates: usize,
    pub paint_misses: usize,
    pub paint_hits: usize,
    pub video_surfaces: usize,
}

impl Pipeline {
    pub fn new() -> Self {
        Self {
            prev_ir: None,
            last_snapshot: None,
            paint_cache: HashMap::new(),
            video_surfaces: Vec::new(),
        }
    }

    pub fn render<'r>(
        &'r mut self,
        next_ir: CoreIR,
        viewport: LayoutSize,
        layout_engine: &mut LayoutEngine,
        scroll_map: &ScrollStateMap,
        renderer: &mut (impl Renderer + 'r + ?Sized),
        video_map: &VideoStateMap,
    ) -> Result<PipelineStats> {
        let dirty_set = if let Some(prev) = &self.prev_ir {
            let diff = diff_ir(prev, &next_ir);
            // println!("Diff: {} dirty nodes", diff.dirty_structural.len());
            diff.dirty_structural
        } else {
            // println!("Diff: Full Rebuild");
            next_ir.nodes.keys().cloned().collect()
        };

        let dirty_count = dirty_set.len();

        let layout_input_nodes = build_layout_tree(&next_ir);
        layout_engine.update(&layout_input_nodes, &dirty_set);

        let root_id = next_ir.root.unwrap();
        let snapshot = layout_engine.compute_layout(&layout_input_nodes, root_id, viewport)?;

        let mut display_list =
            DisplayList::new(LayoutRect::new(0.0, 0.0, viewport.width, viewport.height));
        let mut paint_misses = 0;
        let mut paint_hits = 0;

        self.generate_display_list_recursive(
            root_id,
            &next_ir,
            &snapshot,
            scroll_map,
            &mut display_list,
            &mut paint_misses,
            &mut paint_hits,
            video_map,
        );

        renderer.render(&display_list)?;

        let video_surface_count = self.video_surfaces.len();
        self.last_snapshot = Some(snapshot);
        self.paint_cache
            .retain(|k, _| next_ir.nodes.contains_key(k));
        self.prev_ir = Some(next_ir);

        Ok(PipelineStats {
            dirty_nodes: dirty_count,
            layout_updates: dirty_count,
            paint_misses,
            paint_hits,
            video_surfaces: video_surface_count,
        })
    }

    pub fn take_video_surfaces(&mut self) -> Vec<VideoSurfaceFrame> {
        std::mem::take(&mut self.video_surfaces)
    }

    fn generate_display_list_recursive(
        &mut self,
        node_id: NodeId,
        ir: &CoreIR,
        snapshot: &LayoutSnapshot,
        scroll_map: &ScrollStateMap,
        out_list: &mut DisplayList,
        miss_count: &mut usize,
        hit_count: &mut usize,
        video_map: &VideoStateMap,
    ) {
        if let (Some(node), Some(geom)) = (ir.nodes.get(&node_id), snapshot.nodes.get(&node_id)) {
            let mut hasher = DefaultHasher::new();
            node.hash.hash(&mut hasher);
            (geom.rect.origin.x.to_bits()).hash(&mut hasher);
            (geom.rect.origin.y.to_bits()).hash(&mut hasher);
            (geom.rect.size.width.to_bits()).hash(&mut hasher);
            (geom.rect.size.height.to_bits()).hash(&mut hasher);
            (geom.content_size.width.to_bits()).hash(&mut hasher);
            (geom.content_size.height.to_bits()).hash(&mut hasher);

            if let fission_ir::Op::Layout(fission_ir::LayoutOp::Scroll { .. }) = &node.op {
                let offset = scroll_map.get_offset(node_id);
                (offset.to_bits()).hash(&mut hasher);
            }

            let hash = hasher.finish();

            if let Some((cached_hash, cached_ops)) = self.paint_cache.get(&node_id) {
                if *cached_hash == hash {
                    *hit_count += 1;
                    for op in cached_ops {
                        out_list.push(op.clone());
                    }
                    return;
                }
            }

            *miss_count += 1;
            let mut segment = Vec::new();

            let mut pushed_clip = false;

            match &node.op {
                fission_ir::Op::Layout(fission_ir::LayoutOp::Scroll { show_scrollbar, .. }) => {
                    let offset = scroll_map.get_offset(node_id);
                    segment.push(DisplayOp::Save);
                    segment.push(DisplayOp::ClipRect(geom.rect));
                    segment.push(DisplayOp::Translate(LayoutPoint::new(0.0, -offset)));
                    pushed_clip = true;
                }
                fission_ir::Op::Paint(fission_ir::PaintOp::DrawRect {
                    fill,
                    stroke,
                    corner_radius,
                    shadow,
                }) => {
                    segment.push(DisplayOp::DrawRect {
                        rect: geom.rect,
                        fill: fill.map(|f| Fill {
                            color: RenderColor {
                                r: f.color.r,
                                g: f.color.g,
                                b: f.color.b,
                                a: f.color.a,
                            },
                        }),
                        stroke: stroke.map(|s| Stroke {
                            color: RenderColor {
                                r: s.color.r,
                                g: s.color.g,
                                b: s.color.b,
                                a: s.color.a,
                            },
                            width: s.width,
                        }),
                        corner_radius: *corner_radius,
                        shadow: shadow.map(|s| BoxShadow {
                            color: RenderColor {
                                r: s.color.r,
                                g: s.color.g,
                                b: s.color.b,
                                a: s.color.a,
                            },
                            blur_radius: s.blur_radius,
                            offset: s.offset,
                        }),
                        bounds: geom.rect,
                        node_id: Some(node_id),
                    });
                }
                fission_ir::Op::Paint(fission_ir::PaintOp::DrawText { text, size, color }) => {
                    segment.push(DisplayOp::DrawText {
                        text: text.clone(),
                        position: geom.rect.origin,
                        size: *size,
                        color: RenderColor {
                            r: color.r,
                            g: color.g,
                            b: color.b,
                            a: color.a,
                        },
                        bounds: geom.rect,
                        node_id: Some(node_id),
                    });
                }
                fission_ir::Op::Paint(fission_ir::PaintOp::DrawImage { source, fit }) => {
                    segment.push(DisplayOp::DrawImage {
                        rect: geom.rect,
                        source: source.clone(),
                        fit: match fit {
                            fission_ir::op::ImageFit::Contain => ImageFit::Contain,
                            fission_ir::op::ImageFit::Cover => ImageFit::Cover,
                            fission_ir::op::ImageFit::Fill => ImageFit::Fill,
                            fission_ir::op::ImageFit::None => ImageFit::None,
                        },
                        bounds: geom.rect,
                        node_id: Some(node_id),
                    });
                }
                fission_ir::Op::Layout(fission_ir::LayoutOp::Embed {
                    kind: EmbedKind::Video,
                    widget_id,
                }) => {
                    if let Some(state) = video_map.states.get(widget_id) {
                        if let Some(surface_id) = state.surface_id {
                            self.video_surfaces.push(VideoSurfaceFrame {
                                widget_id: *widget_id,
                                surface_id,
                                rect: geom.rect,
                            });
                        }
                    }

                    segment.push(DisplayOp::DrawRect {
                        rect: geom.rect,
                        fill: Some(Fill {
                            color: RenderColor {
                                r: 0,
                                g: 0,
                                b: 0,
                                a: 255,
                            },
                        }),
                        stroke: None,
                        corner_radius: 0.0,
                        shadow: None,
                        bounds: geom.rect,
                        node_id: Some(node_id),
                    });
                }
                _ => {}
            }

            let mut temp_dl = DisplayList {
                ops: Vec::new(),
                bounds: out_list.bounds,
            };

            for child in &node.children {
                self.generate_display_list_recursive(
                    *child,
                    ir,
                    snapshot,
                    scroll_map,
                    &mut temp_dl,
                    miss_count,
                    hit_count,
                    video_map,
                );
            }

            segment.extend(temp_dl.ops);

            if pushed_clip {
                segment.push(DisplayOp::Restore);

                if let fission_ir::Op::Layout(fission_ir::LayoutOp::Scroll {
                    show_scrollbar: true,
                    ..
                }) = &node.op
                {
                    let viewport_h = geom.rect.height();
                    let content_h = geom.content_size.height;

                    if content_h > viewport_h {
                        let offset = scroll_map.get_offset(node_id);
                        let ratio = viewport_h / content_h;
                        let thumb_h = (viewport_h * ratio).max(20.0);

                        let max_scroll = content_h - viewport_h;
                        let scroll_fraction = if max_scroll > 0.0 {
                            offset / max_scroll
                        } else {
                            0.0
                        };
                        let available_track = viewport_h - thumb_h;
                        let thumb_y = available_track * scroll_fraction.clamp(0.0, 1.0);

                        let thumb_rect = LayoutRect::new(
                            geom.rect.right() - 8.0,
                            geom.rect.y() + thumb_y,
                            6.0,
                            thumb_h,
                        );

                        segment.push(DisplayOp::DrawRect {
                            rect: thumb_rect,
                            fill: Some(Fill {
                                color: RenderColor {
                                    r: 0,
                                    g: 0,
                                    b: 0,
                                    a: 100,
                                },
                            }),
                            stroke: None,
                            corner_radius: 3.0,
                            shadow: None,
                            bounds: thumb_rect,
                            node_id: None,
                        });
                    }
                }
            }

            self.paint_cache.insert(node_id, (hash, segment.clone()));
            out_list.ops.extend(segment);
        }
    }
}
