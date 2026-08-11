use super::*;

pub(super) fn log_texture_plan(plan: &CompositorTexturePlan, depth: usize) {
    let indent = "  ".repeat(depth);
    eprintln!(
        "[pipeline] {}plan key={} bounds=({}, {}, {}x{}) scene={} clip={} transform=({:.1},{:.1}) transform_clip={} children={}",
        indent,
        plan.key,
        plan.bounds.origin.x,
        plan.bounds.origin.y,
        plan.bounds.size.width,
        plan.bounds.size.height,
        plan.scene.is_some(),
        plan.clip.is_some(),
        plan.transform.map(|m| m[12]).unwrap_or(0.0),
        plan.transform.map(|m| m[13]).unwrap_or(0.0),
        plan.transform_clip,
        plan.children.len()
    );
    for child in &plan.children {
        log_texture_plan(child, depth + 1);
    }
}

pub(super) fn layer_mut_at_path<'a>(
    scene: &'a mut RenderScene,
    path: &[usize],
) -> Option<&'a mut RenderLayer> {
    let (root_index, tail) = path.split_first()?;
    let node = scene.roots.get_mut(*root_index)?;
    layer_mut_in_node(node, tail)
}

pub(super) fn render_node_mut_at_path<'a>(
    scene: &'a mut RenderScene,
    path: &[usize],
) -> Option<&'a mut RenderNode> {
    let (root_index, tail) = path.split_first()?;
    let node = scene.roots.get_mut(*root_index)?;
    render_node_mut_in_node(node, tail)
}

pub(super) fn render_node_mut_in_node<'a>(
    node: &'a mut RenderNode,
    path: &[usize],
) -> Option<&'a mut RenderNode> {
    if path.is_empty() {
        return Some(node);
    }
    match node {
        RenderNode::Layer(layer) => {
            let (child_index, tail) = path.split_first()?;
            let child = layer.children.get_mut(*child_index)?;
            render_node_mut_in_node(child, tail)
        }
        RenderNode::Paint(_) => None,
    }
}

pub(super) fn layer_mut_in_node<'a>(
    node: &'a mut RenderNode,
    path: &[usize],
) -> Option<&'a mut RenderLayer> {
    match node {
        RenderNode::Layer(layer) => {
            if path.is_empty() {
                return Some(layer);
            }
            let (child_index, tail) = path.split_first()?;
            let child = layer.children.get_mut(*child_index)?;
            layer_mut_in_node(child, tail)
        }
        RenderNode::Paint(_) => None,
    }
}

pub(super) fn layer_ref_at_path<'a>(
    scene: &'a RenderScene,
    path: &[usize],
) -> Option<&'a RenderLayer> {
    let (root_index, tail) = path.split_first()?;
    let node = scene.roots.get(*root_index)?;
    layer_ref_in_node(node, tail)
}

pub(super) fn layer_ref_in_node<'a>(
    node: &'a RenderNode,
    path: &[usize],
) -> Option<&'a RenderLayer> {
    match node {
        RenderNode::Layer(layer) => {
            if path.is_empty() {
                return Some(layer);
            }
            let (child_index, tail) = path.split_first()?;
            let child = layer.children.get(*child_index)?;
            layer_ref_in_node(child, tail)
        }
        RenderNode::Paint(_) => None,
    }
}

pub(super) fn count_render_paint_ops(scene: &RenderScene) -> usize {
    scene.roots.iter().map(count_render_node_paint_ops).sum()
}

pub(super) fn count_render_node_paint_ops(node: &RenderNode) -> usize {
    match node {
        RenderNode::Paint(list) => list.ops.len(),
        RenderNode::Layer(layer) => layer.children.iter().map(count_render_node_paint_ops).sum(),
    }
}

pub(super) fn render_node_bounds(node: &RenderNode) -> LayoutRect {
    match node {
        RenderNode::Paint(list) => list.bounds,
        RenderNode::Layer(layer) => layer.bounds,
    }
}

pub(super) fn find_texture_compositor_split_layer_path(scene: &RenderScene) -> Option<Vec<usize>> {
    let Some(RenderNode::Layer(presentation_root)) = scene.roots.first() else {
        return None;
    };
    if presentation_root.children.len() != 1 {
        return None;
    }
    let Some(RenderNode::Layer(layer)) = presentation_root.children.first() else {
        return None;
    };
    let mut layer = layer;
    let mut path = vec![0, 0];
    loop {
        let only_child = match layer.children.as_slice() {
            [RenderNode::Layer(child)] => Some(child),
            _ => None,
        };
        let is_plain_wrapper = layer.style.clip.is_none()
            && (layer.style.opacity - 1.0).abs() <= 0.001
            && layer.style.transform.is_none();
        if let (true, Some(child)) = (is_plain_wrapper, only_child) {
            layer = child;
            path.push(0);
        } else {
            return Some(path);
        }
    }
}

#[derive(Debug)]
struct TexturePlanCandidate<'a> {
    node: &'a RenderNode,
    path: Vec<usize>,
}

pub(super) fn build_texture_plan_for_node(
    node: &RenderNode,
    node_path: &[usize],
    force: bool,
    runtime_dynamic_nodes: &HashSet<WidgetId>,
    scroll_nodes: &HashSet<WidgetId>,
    runtime_dynamic_subtrees: &HashMap<WidgetId, bool>,
) -> Option<CompositorTexturePlan> {
    let candidate = find_nested_texture_plan_candidate(
        node,
        node_path,
        force,
        runtime_dynamic_nodes,
        scroll_nodes,
        runtime_dynamic_subtrees,
    )?;
    let bounds = render_node_bounds(candidate.node);
    if bounds.size.width <= 0.0 || bounds.size.height <= 0.0 {
        return None;
    }

    match candidate.node {
        RenderNode::Paint(list) => {
            let scene = localized_scene_for_compositor_children(
                vec![RenderNode::Paint(list.clone())],
                bounds,
            );
            let scene_cache_key = scene_cache_key(&scene);
            let content_key = plan_content_key(Some(scene_cache_key), &[]);
            Some(CompositorTexturePlan {
                key: texture_plan_key_for_paint(list),
                bounds,
                scene: Some(scene),
                scene_cache_key: Some(scene_cache_key),
                content_key,
                local_dynamic: false,
                composite_dynamic: false,
                opacity: 1.0,
                transform: None,
                transform_clip: true,
                clip: None,
                children: Vec::new(),
                source_layer_path: None,
            })
        }
        RenderNode::Layer(layer) => {
            let wrapper_only_scroll_plan = !layer.style.transform_clip;
            let mut child_plans = Vec::new();
            let mut local_children = Vec::new();
            for (child_index, child) in layer.children.iter().enumerate() {
                let mut child_path = candidate.path.clone();
                child_path.push(child_index);
                if wrapper_only_scroll_plan {
                    child_plans.extend(build_descending_wrapper_plans(
                        child,
                        &child_path,
                        runtime_dynamic_nodes,
                        scroll_nodes,
                        runtime_dynamic_subtrees,
                    ));
                } else {
                    if let Some(child_plan) = build_texture_plan_for_node(
                        child,
                        &child_path,
                        false,
                        runtime_dynamic_nodes,
                        scroll_nodes,
                        runtime_dynamic_subtrees,
                    ) {
                        child_plans.push(child_plan);
                    } else {
                        local_children.push(child.clone());
                    }
                }
            }

            let local_dynamic = local_children
                .iter()
                .any(|child| render_node_or_subtree_is_dynamic(child, runtime_dynamic_subtrees));
            let scene = if local_children.is_empty() {
                None
            } else {
                Some(localized_scene_for_compositor_children(
                    local_children,
                    bounds,
                ))
            };
            let scene_cache_key = if scene.is_none() {
                None
            } else {
                layer
                    .style
                    .content_cache_key
                    .or(layer.style.cache_key)
                    .or_else(|| scene.as_ref().map(scene_cache_key))
            };
            let content_key = plan_content_key(scene_cache_key, &child_plans);
            let composite_dynamic = layer
                .node_id
                .map(|id| runtime_dynamic_nodes.contains(&id))
                .unwrap_or(false);
            Some(CompositorTexturePlan {
                key: texture_plan_key_for_layer(layer),
                bounds,
                scene,
                scene_cache_key,
                content_key,
                local_dynamic,
                composite_dynamic,
                opacity: layer.style.opacity,
                transform: layer.style.transform,
                transform_clip: layer.style.transform_clip,
                clip: layer.style.clip.clone(),
                children: child_plans,
                source_layer_path: Some(candidate.path),
            })
        }
    }
}

pub(super) fn build_descending_wrapper_plans(
    node: &RenderNode,
    node_path: &[usize],
    runtime_dynamic_nodes: &HashSet<WidgetId>,
    scroll_nodes: &HashSet<WidgetId>,
    runtime_dynamic_subtrees: &HashMap<WidgetId, bool>,
) -> Vec<CompositorTexturePlan> {
    match node {
        RenderNode::Paint(_) => build_texture_plan_for_node(
            node,
            node_path,
            true,
            runtime_dynamic_nodes,
            scroll_nodes,
            runtime_dynamic_subtrees,
        )
        .into_iter()
        .collect(),
        RenderNode::Layer(layer) => {
            let mut children = Vec::new();
            for (child_index, child) in layer.children.iter().enumerate() {
                let mut child_path = node_path.to_vec();
                child_path.push(child_index);
                children.extend(build_descending_wrapper_plans(
                    child,
                    &child_path,
                    runtime_dynamic_nodes,
                    scroll_nodes,
                    runtime_dynamic_subtrees,
                ));
            }

            if children.is_empty() {
                return build_texture_plan_for_node(
                    node,
                    node_path,
                    true,
                    runtime_dynamic_nodes,
                    scroll_nodes,
                    runtime_dynamic_subtrees,
                )
                .into_iter()
                .collect();
            }

            let composite_dynamic = layer
                .node_id
                .map(|id| runtime_dynamic_nodes.contains(&id))
                .unwrap_or(false);
            vec![CompositorTexturePlan {
                key: texture_plan_key_for_layer(layer),
                bounds: layer.bounds,
                scene: None,
                scene_cache_key: None,
                content_key: plan_content_key(None, &children),
                local_dynamic: false,
                composite_dynamic,
                opacity: layer.style.opacity,
                transform: layer.style.transform,
                transform_clip: layer.style.transform_clip,
                clip: layer.style.clip.clone(),
                children,
                source_layer_path: Some(node_path.to_vec()),
            }]
        }
    }
}

pub(super) fn find_nested_texture_plan_candidate<'a>(
    node: &'a RenderNode,
    node_path: &[usize],
    force: bool,
    runtime_dynamic_nodes: &HashSet<WidgetId>,
    scroll_nodes: &HashSet<WidgetId>,
    runtime_dynamic_subtrees: &HashMap<WidgetId, bool>,
) -> Option<TexturePlanCandidate<'a>> {
    match node {
        RenderNode::Paint(_) => force.then_some(TexturePlanCandidate {
            node,
            path: node_path.to_vec(),
        }),
        RenderNode::Layer(layer) => {
            let own_dynamic = layer
                .node_id
                .map(|id| runtime_dynamic_nodes.contains(&id))
                .unwrap_or(false);
            let is_scroll_node = layer
                .node_id
                .map(|id| scroll_nodes.contains(&id))
                .unwrap_or(false);

            if !force && !own_dynamic && !is_scroll_node {
                if let Some(child) = descend_through_plain_wrapper(layer) {
                    let mut child_path = node_path.to_vec();
                    child_path.push(0);
                    return find_nested_texture_plan_candidate(
                        child,
                        &child_path,
                        false,
                        runtime_dynamic_nodes,
                        scroll_nodes,
                        runtime_dynamic_subtrees,
                    );
                }
            }

            let subtree_dynamic = render_node_or_subtree_is_dynamic(node, runtime_dynamic_subtrees);
            if force
                || layer_should_extract_as_plan(layer, subtree_dynamic, own_dynamic, is_scroll_node)
            {
                Some(TexturePlanCandidate {
                    node,
                    path: node_path.to_vec(),
                })
            } else {
                for (child_index, child) in layer.children.iter().enumerate() {
                    let mut child_path = node_path.to_vec();
                    child_path.push(child_index);
                    if let Some(candidate) = find_nested_texture_plan_candidate(
                        child,
                        &child_path,
                        false,
                        runtime_dynamic_nodes,
                        scroll_nodes,
                        runtime_dynamic_subtrees,
                    ) {
                        return Some(candidate);
                    }
                }
                None
            }
        }
    }
}

pub(super) fn descend_through_plain_wrapper<'a>(layer: &'a RenderLayer) -> Option<&'a RenderNode> {
    let only_child = match layer.children.as_slice() {
        [child] => Some(child),
        _ => None,
    }?;
    if layer.style.clip.is_none()
        && (layer.style.opacity - 1.0).abs() <= 0.001
        && layer.style.transform.is_none()
    {
        match only_child {
            RenderNode::Layer(_) => Some(only_child),
            RenderNode::Paint(_) => None,
        }
    } else {
        None
    }
}

pub(super) fn layer_should_extract_as_plan(
    layer: &RenderLayer,
    subtree_dynamic: bool,
    own_dynamic: bool,
    is_scroll_node: bool,
) -> bool {
    const MIN_PLAN_AREA: f32 = 64.0 * 64.0;
    if layer.children.is_empty() {
        return false;
    }
    if is_scroll_node {
        return false;
    }
    if own_dynamic {
        return true;
    }
    if !subtree_dynamic {
        return false;
    }
    let has_style = layer.style.clip.is_some()
        || (layer.style.opacity - 1.0).abs() > 0.001
        || layer.style.transform.is_some();
    let has_local_paint = layer
        .children
        .iter()
        .any(|child| matches!(child, RenderNode::Paint(_)));
    let has_multiple_children = layer.children.len() > 1;
    (has_style || has_local_paint || has_multiple_children)
        && layer.bounds.size.width * layer.bounds.size.height >= MIN_PLAN_AREA
}

pub(super) fn localized_scene_for_compositor_children(
    children: Vec<RenderNode>,
    bounds: LayoutRect,
) -> RenderScene {
    let local_bounds = LayoutRect::new(0.0, 0.0, bounds.size.width, bounds.size.height);
    let mut root = RenderLayer::new(local_bounds);
    root.style.transform = Some(translation_matrix(-bounds.origin.x, -bounds.origin.y));
    root.children.extend(children);

    let mut scene = RenderScene::new(local_bounds);
    scene.roots.push(RenderNode::Layer(root));
    scene
}

pub(super) fn render_node_or_subtree_is_dynamic(
    node: &RenderNode,
    runtime_dynamic_subtrees: &HashMap<WidgetId, bool>,
) -> bool {
    match node {
        RenderNode::Paint(_) => false,
        RenderNode::Layer(layer) => {
            layer
                .node_id
                .and_then(|id| runtime_dynamic_subtrees.get(&id).copied())
                .unwrap_or(false)
                || layer
                    .children
                    .iter()
                    .any(|child| render_node_or_subtree_is_dynamic(child, runtime_dynamic_subtrees))
        }
    }
}

pub(super) fn texture_plan_key_for_layer(layer: &RenderLayer) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    layer.node_id.hash(&mut hasher);
    layer.bounds.size.width.to_bits().hash(&mut hasher);
    layer.bounds.size.height.to_bits().hash(&mut hasher);
    hash_serde_value(&layer.style.clip, &mut hasher);
    hasher.finish()
}

pub(super) fn texture_plan_key_for_paint(list: &DisplayList) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    list.bounds.size.width.to_bits().hash(&mut hasher);
    list.bounds.size.height.to_bits().hash(&mut hasher);
    hash_serde_value(list, &mut hasher);
    hasher.finish()
}

pub(super) fn scene_cache_key(scene: &RenderScene) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    hash_serde_value(scene, &mut hasher);
    hasher.finish()
}

pub(super) fn plan_content_key(
    scene_cache_key: Option<u64>,
    children: &[CompositorTexturePlan],
) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    scene_cache_key.hash(&mut hasher);
    for child in children {
        child.key.hash(&mut hasher);
        child.content_key.hash(&mut hasher);
        child.bounds.origin.x.to_bits().hash(&mut hasher);
        child.bounds.origin.y.to_bits().hash(&mut hasher);
        child.bounds.size.width.to_bits().hash(&mut hasher);
        child.bounds.size.height.to_bits().hash(&mut hasher);
        child.opacity.to_bits().hash(&mut hasher);
        hash_serde_value(&child.transform, &mut hasher);
        hash_serde_value(&child.clip, &mut hasher);
    }
    hasher.finish()
}

pub(super) fn patch_texture_compositor_plans(
    plans: &mut [CompositorTexturePlan],
    scene: &RenderScene,
) {
    for plan in plans {
        patch_texture_compositor_plan(plan, scene);
    }
}

pub(super) fn patch_texture_compositor_plan(plan: &mut CompositorTexturePlan, scene: &RenderScene) {
    for child in &mut plan.children {
        patch_texture_compositor_plan(child, scene);
    }

    if let Some(path) = plan.source_layer_path.as_deref() {
        if let Some(layer) = layer_ref_at_path(scene, path) {
            plan.bounds = layer.bounds;
            plan.opacity = layer.style.opacity;
            plan.transform = layer.style.transform;
            plan.transform_clip = layer.style.transform_clip;
            plan.clip = layer.style.clip.clone();
        }
    }

    plan.content_key = plan_content_key(plan.scene_cache_key, &plan.children);
}

pub(super) fn hash_serde_value<T: Serialize, H: Hasher>(value: &T, hasher: &mut H) {
    if let Ok(bytes) = bincode::serialize(value) {
        bytes.hash(hasher);
    }
}

pub(super) fn presentation_transform_matrix(
    render_viewport_size: LayoutSize,
    layout_viewport_size: LayoutSize,
    resize_preview: bool,
) -> Option<[f32; 16]> {
    if !resize_preview
        || render_viewport_size.width <= 0.0
        || render_viewport_size.height <= 0.0
        || layout_viewport_size.width <= 0.0
        || layout_viewport_size.height <= 0.0
    {
        return None;
    }

    // Do not non-uniformly scale the retained UI during live resize.
    // Text-heavy surfaces look visibly distorted; we keep the last committed
    // layout anchored in place and rely on throttled relayouts instead.
    None
}

pub(super) fn compose_dynamic_layer_transform(
    binding: &TransformBinding,
    scroll_map: &ScrollStateMap,
    animation_map: &MotionStateMap,
) -> Option<[f32; 16]> {
    let mut matrix: Option<[f32; 16]> = None;

    if let Some(scroll) = &binding.scroll {
        let offset = scroll_map.get_offset(scroll.node_id);
        let scroll_matrix = match scroll.direction {
            FlexDirection::Row => translation_matrix(-offset, 0.0),
            FlexDirection::Column => translation_matrix(0.0, -offset),
        };
        matrix = append_transform(matrix, scroll_matrix);
    }

    if let Some(layout_transform) = binding.layout_transform {
        matrix = append_transform(matrix, layout_transform);
    }

    let translate_x = binding
        .translate_x
        .as_ref()
        .map(|scalar| resolve_scalar_value(scalar, animation_map, MotionPropertyId::TranslateX))
        .unwrap_or(0.0);
    let translate_y = binding
        .translate_y
        .as_ref()
        .map(|scalar| resolve_scalar_value(scalar, animation_map, MotionPropertyId::TranslateY))
        .unwrap_or(0.0);
    let scale = binding
        .scale
        .as_ref()
        .map(|scalar| resolve_scalar_value(scalar, animation_map, MotionPropertyId::Scale))
        .unwrap_or(1.0);
    let rotation = binding
        .rotation
        .as_ref()
        .map(|scalar| resolve_scalar_value(scalar, animation_map, MotionPropertyId::Rotation))
        .unwrap_or(0.0);

    let has_composite_transform = translate_x.abs() > 0.001
        || translate_y.abs() > 0.001
        || (scale - 1.0).abs() > 0.001
        || rotation.abs() > 0.001;
    if has_composite_transform {
        matrix = append_transform(
            matrix,
            composite_transform_matrix(binding.rect, translate_x, translate_y, scale, rotation),
        );
    }

    matrix.filter(|value| !is_identity_matrix(value))
}

pub(super) fn append_transform(current: Option<[f32; 16]>, next: [f32; 16]) -> Option<[f32; 16]> {
    Some(match current {
        Some(existing) => multiply_matrix(existing, next),
        None => next,
    })
}
