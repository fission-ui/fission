use crate::env::ScrollStateMap;
use crate::input::viewport::ViewportStateMap;
use crate::ui::custom_render::downcast_render_object;
use fission_diagnostics::prelude as diag;
use fission_ir::{
    ActionEntry, ActionTrigger, CoreIR, KeyCode, LayoutOp, Op, PaintOp, PopupKind, Role,
    SemanticOrientation, StructuralOp, WidgetId,
};
use fission_layout::{LayoutPoint, LayoutSnapshot};
use glam::{Mat4, Vec4};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusDirection {
    Up,
    Down,
    Left,
    Right,
}

/// Result of applying a semantic composite's keyboard-navigation contract.
///
/// `Handled(None)` means the key belongs to the composite, but there is no
/// enabled destination. Callers must not fall back to page-wide spatial or Tab
/// traversal in that case.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SemanticFocusNavigation {
    NotApplicable,
    Handled(Option<WidgetId>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CompositeMove {
    Previous,
    Next,
    First,
    Last,
}

pub fn hit_test(
    ir: &CoreIR,
    layout: &LayoutSnapshot,
    scroll_map: &ScrollStateMap,
    point: LayoutPoint,
) -> Option<WidgetId> {
    hit_test_internal(ir, layout, Some(scroll_map), None, point)
}

pub fn hit_test_with_scroll(
    ir: &CoreIR,
    layout: &LayoutSnapshot,
    scroll_map: &ScrollStateMap,
    point: LayoutPoint,
) -> Option<WidgetId> {
    hit_test_internal(ir, layout, Some(scroll_map), None, point)
}

pub fn hit_test_with_viewports(
    ir: &CoreIR,
    layout: &LayoutSnapshot,
    scroll_map: &ScrollStateMap,
    viewport_map: &ViewportStateMap,
    point: LayoutPoint,
) -> Option<WidgetId> {
    hit_test_internal(ir, layout, Some(scroll_map), Some(viewport_map), point)
}

/// Maps a screen-space point into the layout coordinate space used by `target`.
///
/// Controllers use this after hit testing so interaction math remains correct
/// inside scrolled, transformed, and interactive-viewport ancestors.
pub fn point_to_node_space(
    ir: &CoreIR,
    layout: &LayoutSnapshot,
    scroll_map: &ScrollStateMap,
    viewport_map: &ViewportStateMap,
    target: WidgetId,
    point: LayoutPoint,
) -> LayoutPoint {
    let mut ancestors = Vec::new();
    let mut current = ir.nodes.get(&target).and_then(|node| node.parent);
    while let Some(id) = current {
        ancestors.push(id);
        current = ir.nodes.get(&id).and_then(|node| node.parent);
    }
    ancestors.reverse();

    let mut mapped = point;
    for id in ancestors {
        let Some(node) = ir.nodes.get(&id) else {
            continue;
        };
        let Some(geometry) = layout.get_node_geometry(id) else {
            continue;
        };
        match &node.op {
            Op::Layout(LayoutOp::Scroll { direction, .. }) => {
                let offset = scroll_map.get_offset(id);
                match direction {
                    fission_ir::FlexDirection::Column => mapped.y += offset,
                    fission_ir::FlexDirection::Row => mapped.x += offset,
                }
            }
            Op::Layout(LayoutOp::Transform { transform }) => {
                let inverse = Mat4::from_cols_array(transform).inverse();
                let local = Vec4::new(
                    mapped.x - geometry.rect.origin.x,
                    mapped.y - geometry.rect.origin.y,
                    0.0,
                    1.0,
                );
                let transformed = inverse * local;
                mapped = LayoutPoint::new(
                    transformed.x + geometry.rect.origin.x,
                    transformed.y + geometry.rect.origin.y,
                );
            }
            Op::Layout(LayoutOp::InteractiveViewport { .. }) => {
                if let Some(transform) = viewport_map.transform(id) {
                    let local = [
                        mapped.x - geometry.rect.origin.x,
                        mapped.y - geometry.rect.origin.y,
                    ];
                    let world = transform.screen_to_world(local);
                    mapped = LayoutPoint::new(
                        world[0] + geometry.rect.origin.x,
                        world[1] + geometry.rect.origin.y,
                    );
                }
            }
            _ => {}
        }
    }
    mapped
}

fn hit_test_internal(
    ir: &CoreIR,
    layout: &LayoutSnapshot,
    scroll_map: Option<&ScrollStateMap>,
    viewport_map: Option<&ViewportStateMap>,
    point: LayoutPoint,
) -> Option<WidgetId> {
    let result = ir
        .root
        .and_then(|root| hit_test_recursive(root, ir, layout, scroll_map, viewport_map, point));

    if let Some(id) = result {
        diag::emit(
            diag::DiagCategory::Input,
            diag::DiagLevel::Debug,
            diag::DiagEventKind::InputEvent {
                kind: "hit_test_result".into(),
                target: Some(id.as_u128()),
                position: Some((point.x, point.y)),
            },
        );
    }
    result
}

fn hit_test_recursive(
    node_id: WidgetId,
    ir: &CoreIR,
    layout: &LayoutSnapshot,
    scroll_map: Option<&ScrollStateMap>,
    viewport_map: Option<&ViewportStateMap>,
    point: LayoutPoint,
) -> Option<WidgetId> {
    let node = ir.nodes.get(&node_id)?;
    let geom = layout.get_node_geometry(node_id)?;

    if matches!(
        &node.op,
        Op::Structural(
            StructuralOp::PointerTransparent { .. } | StructuralOp::InteractionInert { .. }
        )
    ) {
        return None;
    }

    let is_clip_container = match &node.op {
        Op::Layout(LayoutOp::Clip { .. }) | Op::Layout(LayoutOp::Scroll { .. }) => true,
        Op::Layout(LayoutOp::InteractiveViewport { clip, .. }) => {
            !matches!(clip, fission_ir::ViewportClip::None)
        }
        _ => false,
    };

    if is_clip_container && !geom.rect.contains(point) {
        return None;
    }

    let mut child_point = point;

    if let (Some(map), Op::Layout(LayoutOp::Scroll { direction, .. })) = (scroll_map, &node.op) {
        let offset = map.get_offset(node_id);
        match direction {
            fission_ir::FlexDirection::Column => {
                child_point.y += offset;
            }
            fission_ir::FlexDirection::Row => {
                child_point.x += offset;
            }
        }
    }

    if let Op::Layout(LayoutOp::Transform { transform }) = &node.op {
        let mat = Mat4::from_cols_array(transform);
        let inv = mat.inverse();
        let local_x = point.x - geom.rect.origin.x;
        let local_y = point.y - geom.rect.origin.y;
        let p = Vec4::new(local_x, local_y, 0.0, 1.0);
        let transformed = inv * p;
        child_point = LayoutPoint::new(
            transformed.x + geom.rect.origin.x,
            transformed.y + geom.rect.origin.y,
        );
    }

    if let (Some(map), Op::Layout(LayoutOp::InteractiveViewport { .. })) = (viewport_map, &node.op)
    {
        if let Some(transform) = map.transform(node_id) {
            let local = [point.x - geom.rect.origin.x, point.y - geom.rect.origin.y];
            let world = transform.screen_to_world(local);
            child_point =
                LayoutPoint::new(world[0] + geom.rect.origin.x, world[1] + geom.rect.origin.y);
        }
    }

    for child_id in node.children.iter().rev() {
        if let Some(hit) =
            hit_test_recursive(*child_id, ir, layout, scroll_map, viewport_map, child_point)
        {
            return Some(hit);
        }
    }

    // --- Custom render object hit-test ----------------------------------
    // If this node has a custom render object, delegate to it before
    // falling through to the standard semantics-based check.
    if geom.rect.contains(point) {
        if let Some(any_ro) = ir.custom_render_objects.get(&node_id) {
            if let Some(render_obj) = downcast_render_object(any_ro) {
                let local_point =
                    LayoutPoint::new(point.x - geom.rect.origin.x, point.y - geom.rect.origin.y);
                let result = render_obj.hit_test(local_point, geom.rect);
                if result.hit {
                    return Some(node_id);
                }
            }
        }
    }

    if geom.rect.contains(point) && paint_op_blocks_hit_testing(&node.op) {
        return Some(node_id);
    }

    let semantic_hit = match &node.op {
        Op::Semantics(semantics) => match semantics.canvas_target.as_ref() {
            Some(target) if matches!(target.kind, fission_ir::CanvasTargetKind::Edge { .. }) => {
                canvas_target_hit(target, point)
            }
            _ => geom.rect.contains(point),
        },
        _ => geom.rect.contains(point),
    };
    let mut current_is_hit = false;
    if semantic_hit {
        match &node.op {
            Op::Layout(LayoutOp::Scroll { .. })
            | Op::Layout(LayoutOp::Embed { .. })
            | Op::Layout(LayoutOp::InteractiveViewport { .. }) => {
                current_is_hit = true;
            }
            Op::Semantics(semantics) => {
                if !semantics.actions.entries.is_empty()
                    || semantics.focusable
                    || semantics.draggable
                    || semantics.scrollable_x
                    || semantics.scrollable_y
                {
                    current_is_hit = true;
                }
            }
            _ => {}
        }
    }

    if current_is_hit {
        Some(node_id)
    } else {
        None
    }
}

fn canvas_target_hit(target: &fission_ir::CanvasTarget, point: LayoutPoint) -> bool {
    let fission_ir::CanvasTargetKind::Edge {
        points,
        cubic,
        hit_tolerance,
        ..
    } = &target.kind
    else {
        return false;
    };
    if points.len() < 2 {
        return false;
    }
    let tolerance_squared = hit_tolerance.max(1.0).powi(2);
    if *cubic && points.len() >= 4 {
        let mut previous = LayoutPoint::new(points[0][0], points[0][1]);
        for step in 1..=24 {
            let t = step as f32 / 24.0;
            let next = cubic_point(points, t);
            if point_segment_distance_squared(point, previous, next) <= tolerance_squared {
                return true;
            }
            previous = next;
        }
        false
    } else {
        points.windows(2).any(|segment| {
            let first = LayoutPoint::new(segment[0][0], segment[0][1]);
            let second = LayoutPoint::new(segment[1][0], segment[1][1]);
            point_segment_distance_squared(point, first, second) <= tolerance_squared
        })
    }
}

fn cubic_point(points: &[[f32; 2]], t: f32) -> LayoutPoint {
    let inverse = 1.0 - t;
    let weights = [
        inverse * inverse * inverse,
        3.0 * inverse * inverse * t,
        3.0 * inverse * t * t,
        t * t * t,
    ];
    LayoutPoint::new(
        points
            .iter()
            .zip(weights)
            .map(|(point, weight)| point[0] * weight)
            .sum(),
        points
            .iter()
            .zip(weights)
            .map(|(point, weight)| point[1] * weight)
            .sum(),
    )
}

fn point_segment_distance_squared(point: LayoutPoint, start: LayoutPoint, end: LayoutPoint) -> f32 {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let length_squared = dx * dx + dy * dy;
    if length_squared <= f32::EPSILON {
        return (point.x - start.x).powi(2) + (point.y - start.y).powi(2);
    }
    let t =
        (((point.x - start.x) * dx + (point.y - start.y) * dy) / length_squared).clamp(0.0, 1.0);
    let nearest = LayoutPoint::new(start.x + dx * t, start.y + dy * t);
    (point.x - nearest.x).powi(2) + (point.y - nearest.y).powi(2)
}

fn paint_op_blocks_hit_testing(op: &Op) -> bool {
    match op {
        Op::Paint(PaintOp::DrawRect {
            fill,
            stroke,
            shadow,
            ..
        }) => fill.is_some() || stroke.is_some() || shadow.is_some(),
        Op::Paint(PaintOp::DrawText { text, .. }) => !text.is_empty(),
        Op::Paint(PaintOp::DrawRichText { runs, .. }) => {
            runs.iter().any(|run| !run.text.is_empty())
        }
        Op::Paint(PaintOp::DrawImage { .. }) => true,
        Op::Paint(PaintOp::DrawPath { fill, stroke, .. })
        | Op::Paint(PaintOp::DrawSvg { fill, stroke, .. }) => fill.is_some() || stroke.is_some(),
        _ => false,
    }
}

pub fn find_next_focus_node(
    ir: &CoreIR,
    current: Option<WidgetId>,
    reverse: bool,
) -> Option<WidgetId> {
    let nodes_in_scope = if let Some(barrier_id) = topmost_focus_barrier(ir) {
        focusable_nodes_in_scope(ir, barrier_id)
    } else if let Some(scope_id) = current.and_then(|id| find_containing_focus_scope(id, ir)) {
        if is_focus_barrier(ir, scope_id) {
            focusable_nodes_in_scope(ir, scope_id)
        } else {
            get_all_focusable_nodes(ir)
        }
    } else {
        get_all_focusable_nodes(ir)
    };

    if nodes_in_scope.is_empty() {
        return None;
    }

    let idx = if let Some(curr_id) = current {
        nodes_in_scope.iter().position(|id| *id == curr_id)
    } else {
        None
    };

    match idx {
        Some(i) => {
            if reverse {
                if i == 0 {
                    Some(nodes_in_scope[nodes_in_scope.len() - 1])
                } else {
                    Some(nodes_in_scope[i - 1])
                }
            } else if i == nodes_in_scope.len() - 1 {
                Some(nodes_in_scope[0])
            } else {
                Some(nodes_in_scope[i + 1])
            }
        }
        None => {
            if reverse {
                Some(nodes_in_scope[nodes_in_scope.len() - 1])
            } else {
                Some(nodes_in_scope[0])
            }
        }
    }
}

/// Finds the next ordinary Tab target relative to a popup controller while
/// excluding the popup being dismissed.
///
/// This preserves the active focus-barrier boundary and makes the result
/// independent of whether a widget accidentally exposes more than one popup
/// descendant to sequential traversal.
pub(crate) fn next_focus_outside_controlled_popup(
    ir: &CoreIR,
    controller: WidgetId,
    popup: WidgetId,
    reverse: bool,
) -> Option<WidgetId> {
    let mut nodes_in_scope = if let Some(scope_id) = find_containing_focus_scope(controller, ir) {
        if is_focus_barrier(ir, scope_id) {
            focusable_nodes_in_scope(ir, scope_id)
        } else {
            get_all_focusable_nodes(ir)
        }
    } else {
        get_all_focusable_nodes(ir)
    };
    nodes_in_scope.retain(|candidate| !is_descendant_or_self(ir, *candidate, popup));
    if nodes_in_scope.is_empty() {
        return None;
    }

    let current_index = nodes_in_scope
        .iter()
        .position(|candidate| *candidate == controller);
    match current_index {
        Some(0) if reverse => nodes_in_scope.last().copied(),
        Some(index) if reverse => Some(nodes_in_scope[index - 1]),
        Some(index) if index + 1 == nodes_in_scope.len() => nodes_in_scope.first().copied(),
        Some(index) => Some(nodes_in_scope[index + 1]),
        None if reverse => nodes_in_scope.last().copied(),
        None => nodes_in_scope.first().copied(),
    }
}

pub fn get_all_focusable_nodes(ir: &CoreIR) -> Vec<WidgetId> {
    let mut list = Vec::new();
    if let Some(root) = ir.root {
        collect_focusable_nodes(root, ir, &mut list, false, true, 0);
    }
    sort_focusable_nodes(ir, list)
}

/// Returns focus barriers in tree order. The last barrier is the topmost active
/// barrier because overlays lower after their underlying content.
pub fn focus_barriers_in_tree_order(ir: &CoreIR) -> Vec<WidgetId> {
    let mut barriers = Vec::new();
    if let Some(root) = ir.root {
        collect_focus_barriers(root, ir, &mut barriers);
    }
    barriers
}

/// Returns the topmost active focus barrier in the current semantic tree.
pub fn topmost_focus_barrier(ir: &CoreIR) -> Option<WidgetId> {
    focus_barriers_in_tree_order(ir).last().copied()
}

/// Returns enabled focusable nodes inside `scope_id` in traversal order.
pub fn focusable_nodes_in_scope(ir: &CoreIR, scope_id: WidgetId) -> Vec<WidgetId> {
    let mut list = Vec::new();
    if let Some(scope) = ir.nodes.get(&scope_id) {
        let mut order = 0;
        for child in &scope.children {
            collect_focusable_nodes(*child, ir, &mut list, false, true, order);
            order = list.last().map(|(_, index)| *index + 1).unwrap_or(order);
        }
    }
    sort_focusable_nodes(ir, list)
}

/// Returns the preferred entry target for a focus scope.
pub fn preferred_focus_node_in_scope(ir: &CoreIR, scope_id: WidgetId) -> Option<WidgetId> {
    let mut nodes = Vec::new();
    if let Some(scope) = ir.nodes.get(&scope_id) {
        let mut order = 0;
        for child in &scope.children {
            collect_focusable_nodes(*child, ir, &mut nodes, false, false, order);
            order = nodes.last().map(|(_, index)| *index + 1).unwrap_or(order);
        }
    }
    let nodes = sort_focusable_nodes(ir, nodes);
    let autofocus = nodes
        .iter()
        .copied()
        .find(|id| semantics(ir, *id).is_some_and(|value| value.autofocus));
    let active_descendant = semantics(ir, scope_id)
        .and_then(|value| value.active_descendant)
        .filter(|candidate| nodes.contains(candidate));
    let selected = nodes
        .iter()
        .copied()
        .find(|id| semantics(ir, *id).is_some_and(|value| value.selected == Some(true)));
    let sequential = nodes.iter().copied().find(|id| {
        semantics(ir, *id).is_some_and(fission_ir::Semantics::is_sequentially_focusable)
    });
    let programmatic_only = nodes
        .iter()
        .copied()
        .find(|id| semantics(ir, *id).is_some_and(|value| !value.is_sequentially_focusable()));

    autofocus
        .or(active_descendant)
        .or(selected)
        .or(sequential)
        .or(programmatic_only)
}

/// Returns whether `node_id` is an enabled focus target.
pub fn is_enabled_focus_node(ir: &CoreIR, node_id: WidgetId) -> bool {
    !is_interaction_inert(ir, node_id)
        && (semantics(ir, node_id).is_some_and(|value| value.focusable && !value.disabled)
            || ir
                .custom_render_objects
                .get(&node_id)
                .and_then(downcast_render_object)
                .is_some_and(|render_object| render_object.accepts_text_input()))
}

/// Returns whether `node_id` belongs to a visual-only interaction-inert
/// subtree, including when the node itself is the inert boundary.
pub fn is_interaction_inert(ir: &CoreIR, node_id: WidgetId) -> bool {
    let mut current = Some(node_id);
    while let Some(id) = current {
        let Some(node) = ir.nodes.get(&id) else {
            return false;
        };
        if matches!(
            node.op,
            Op::Structural(StructuralOp::InteractionInert { .. })
        ) {
            return true;
        }
        current = node.parent;
    }
    false
}

/// Finds the key binding that should handle a press, given the focused node.
///
/// The walk goes from the focused node outward through its ancestors, so the
/// innermost declaration wins. That is what makes a binding on a container
/// behave as a scope: a dialog can bind Enter for everything inside it, and a
/// text field within that dialog can still bind Enter for itself and take
/// precedence.
///
/// Nodes marked disabled are skipped, matching every other interaction path —
/// a disabled control must not respond to a key any more than to a click.
/// Inert subtrees are not consulted, because a node exiting a transition is
/// still painted but is no longer logically present.
pub fn declared_key_action(
    ir: &CoreIR,
    focused: Option<WidgetId>,
    key: &KeyCode,
    modifiers: u8,
) -> Option<(WidgetId, ActionEntry)> {
    let mut current = focused;
    while let Some(node_id) = current {
        let node = ir.nodes.get(&node_id)?;
        if matches!(
            node.op,
            Op::Structural(StructuralOp::InteractionInert { .. })
        ) {
            return None;
        }
        if let Op::Semantics(semantics) = &node.op {
            if !semantics.disabled {
                if let Some(action) = semantics
                    .key_actions
                    .iter()
                    .find(|binding| binding.matches(key, modifiers))
                {
                    return Some((node_id, action.action.clone()));
                }
            }
        }
        current = node.parent;
    }
    None
}

/// Returns the last active semantic action with `trigger` in paint/tree order.
///
/// Overlay portals lower after their underlying content, so the last matching
/// action is the one that owns a global command such as Escape dismissal.
/// Interaction-inert exit-animation subtrees are deliberately excluded.
pub fn topmost_semantics_action(
    ir: &CoreIR,
    trigger: ActionTrigger,
) -> Option<(WidgetId, ActionEntry)> {
    fn visit(
        ir: &CoreIR,
        node_id: WidgetId,
        trigger: ActionTrigger,
        found: &mut Option<(WidgetId, ActionEntry)>,
    ) {
        let Some(node) = ir.nodes.get(&node_id) else {
            return;
        };
        if matches!(
            node.op,
            Op::Structural(StructuralOp::InteractionInert { .. })
        ) {
            return;
        }
        if let Op::Semantics(semantics) = &node.op {
            if let Some(entry) = semantics
                .actions
                .entries
                .iter()
                .find(|entry| entry.trigger == trigger)
            {
                *found = Some((node_id, entry.clone()));
            }
        }
        for child in &node.children {
            visit(ir, *child, trigger, found);
        }
    }

    let mut found = None;
    if let Some(root) = ir.root {
        visit(ir, root, trigger, &mut found);
    }
    found
}

/// Returns whether `node_id` is `ancestor_id` or belongs to its subtree.
pub fn is_descendant_or_self(ir: &CoreIR, node_id: WidgetId, ancestor_id: WidgetId) -> bool {
    let mut current = Some(node_id);
    while let Some(id) = current {
        if id == ancestor_id {
            return true;
        }
        current = ir.nodes.get(&id).and_then(|node| node.parent);
    }
    false
}

fn sort_focusable_nodes(ir: &CoreIR, mut list: Vec<(WidgetId, usize)>) -> Vec<WidgetId> {
    list.sort_by(|(id_a, order_a), (id_b, order_b)| {
        let idx_a = ir.nodes.get(id_a).and_then(|n| {
            if let Op::Semantics(s) = &n.op {
                s.focus_index
            } else {
                None
            }
        });
        let idx_b = ir.nodes.get(id_b).and_then(|n| {
            if let Op::Semantics(s) = &n.op {
                s.focus_index
            } else {
                None
            }
        });

        match (idx_a, idx_b) {
            (Some(a), Some(b)) => a.cmp(&b).then(order_a.cmp(order_b)),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => order_a.cmp(order_b),
        }
    });
    list.into_iter().map(|(id, _)| id).collect()
}

fn collect_focusable_nodes(
    node_id: WidgetId,
    ir: &CoreIR,
    list: &mut Vec<(WidgetId, usize)>,
    stop_at_barriers: bool,
    sequential_only: bool,
    mut order: usize,
) {
    if let Some(node) = ir.nodes.get(&node_id) {
        if matches!(
            node.op,
            Op::Structural(StructuralOp::InteractionInert { .. })
        ) {
            return;
        }
        let mut is_barrier = false;
        if let Op::Semantics(s) = &node.op {
            if s.focusable && (!sequential_only || s.is_sequentially_focusable()) && !s.disabled {
                list.push((node_id, order));
                order += 1;
            }
            is_barrier = s.is_focus_barrier;
        }

        if stop_at_barriers && is_barrier {
            return;
        }

        let mut children = node.children.clone();
        // Internal sort within branches still useful for tree-order
        children.sort_by_key(|cid| {
            ir.nodes
                .get(cid)
                .and_then(|n| {
                    if let Op::Semantics(s) = &n.op {
                        s.focus_index
                    } else {
                        None
                    }
                })
                .unwrap_or(i32::MAX)
        });

        for child in children {
            collect_focusable_nodes(child, ir, list, stop_at_barriers, sequential_only, order);
            order = list.last().map(|(_, o)| *o + 1).unwrap_or(order);
        }
    }
}

fn collect_focus_barriers(node_id: WidgetId, ir: &CoreIR, barriers: &mut Vec<WidgetId>) {
    let Some(node) = ir.nodes.get(&node_id) else {
        return;
    };
    if matches!(
        node.op,
        Op::Structural(StructuralOp::InteractionInert { .. })
    ) {
        return;
    }
    if matches!(&node.op, Op::Semantics(value) if value.is_focus_scope && value.is_focus_barrier) {
        barriers.push(node_id);
    }
    for child in &node.children {
        collect_focus_barriers(*child, ir, barriers);
    }
}

fn find_containing_focus_scope(node_id: WidgetId, ir: &CoreIR) -> Option<WidgetId> {
    let mut curr = Some(node_id);
    while let Some(pid) = curr {
        if let Some(node) = ir.nodes.get(&pid) {
            if let Op::Semantics(s) = &node.op {
                if s.is_focus_scope {
                    return Some(pid);
                }
            }
            curr = node.parent;
        } else {
            break;
        }
    }
    None
}

fn is_focus_barrier(ir: &CoreIR, node_id: WidgetId) -> bool {
    semantics(ir, node_id).is_some_and(|value| value.is_focus_barrier)
}

fn semantics(ir: &CoreIR, node_id: WidgetId) -> Option<&fission_ir::Semantics> {
    match &ir.nodes.get(&node_id)?.op {
        Op::Semantics(value) => Some(value),
        _ => None,
    }
}

/// How a composite role moves keyboard focus between its items.
///
/// A composite owns one tab stop and moves an inner cursor with the arrow keys,
/// the way a menu, a list box or a tab strip does. Adding a role here is all it
/// takes to give a new composite the shared navigation contract: wrapping,
/// Home and End, disabled skipping, and typeahead where it applies.
#[derive(Debug, Clone, Copy)]
pub(crate) struct CompositeContract {
    /// The role its focusable items carry.
    pub item_role: Role,
    /// Axis used when the composite does not declare one itself.
    pub default_orientation: SemanticOrientation,
    /// Whether typing a character jumps to the next item starting with it.
    ///
    /// Right for menus and lists of names; wrong for a tab strip, where typing
    /// belongs to the panel rather than the strip.
    pub supports_typeahead: bool,
}

impl CompositeContract {
    const fn new(
        item_role: Role,
        default_orientation: SemanticOrientation,
        supports_typeahead: bool,
    ) -> Self {
        Self {
            item_role,
            default_orientation,
            supports_typeahead,
        }
    }
}

/// Every composite whose items share one tab stop.
///
/// Keep this as the single place the contract is declared. A composite absent
/// from this table falls back to plain Tab traversal, which is why TreeView and
/// DataTable had no keyboard model before their roles were listed.
const COMPOSITE_CONTRACTS: &[(Role, CompositeContract)] = &[
    (
        Role::Menu,
        CompositeContract::new(Role::MenuItem, SemanticOrientation::Vertical, true),
    ),
    (
        Role::ListBox,
        CompositeContract::new(Role::Option, SemanticOrientation::Vertical, true),
    ),
    (
        Role::TabList,
        CompositeContract::new(Role::Tab, SemanticOrientation::Horizontal, false),
    ),
    (
        Role::Tree,
        CompositeContract::new(Role::TreeItem, SemanticOrientation::Vertical, true),
    ),
    (
        Role::Toolbar,
        CompositeContract::new(Role::Button, SemanticOrientation::Horizontal, false),
    ),
    (
        Role::RadioGroup,
        CompositeContract::new(Role::Radio, SemanticOrientation::Vertical, false),
    ),
];

fn composite_contract(role: Role) -> Option<CompositeContract> {
    COMPOSITE_CONTRACTS
        .iter()
        .find_map(|(candidate, contract)| (*candidate == role).then_some(*contract))
}

fn containing_composite(
    ir: &CoreIR,
    focused: WidgetId,
) -> Option<(WidgetId, Role, SemanticOrientation, bool)> {
    let mut current = Some(focused);
    while let Some(node_id) = current {
        let node = ir.nodes.get(&node_id)?;
        if let Op::Semantics(value) = &node.op {
            if let Some(contract) = composite_contract(value.role) {
                return Some((
                    node_id,
                    contract.item_role,
                    value.orientation.unwrap_or(contract.default_orientation),
                    contract.supports_typeahead,
                ));
            }
        }
        current = node.parent;
    }
    None
}

fn collect_composite_items(
    ir: &CoreIR,
    node_id: WidgetId,
    composite_id: WidgetId,
    item_role: Role,
    items: &mut Vec<WidgetId>,
) {
    let Some(node) = ir.nodes.get(&node_id) else {
        return;
    };
    if matches!(
        node.op,
        Op::Structural(StructuralOp::InteractionInert { .. })
    ) {
        return;
    }
    if let Op::Semantics(value) = &node.op {
        if node_id != composite_id && composite_contract(value.role).is_some() {
            return;
        }
        if value.role == item_role {
            if value.focusable && !value.disabled {
                items.push(node_id);
            }
            return;
        }
    }
    for child in &node.children {
        collect_composite_items(ir, *child, composite_id, item_role, items);
    }
}

fn composite_items(ir: &CoreIR, composite_id: WidgetId, item_role: Role) -> Vec<WidgetId> {
    let mut items = Vec::new();
    collect_composite_items(ir, composite_id, composite_id, item_role, &mut items);
    items
}

fn focused_composite_item(
    ir: &CoreIR,
    focused: WidgetId,
    composite_id: WidgetId,
    items: &[WidgetId],
) -> Option<usize> {
    let mut current = Some(focused);
    while let Some(node_id) = current {
        if let Some(index) = items.iter().position(|candidate| *candidate == node_id) {
            return Some(index);
        }
        if node_id == composite_id {
            break;
        }
        current = ir.nodes.get(&node_id).and_then(|node| node.parent);
    }
    None
}

/// Applies wrapped arrow/Home/End navigation within Menu, ListBox, and TabList.
///
/// Composite items deliberately ignore `sequential_focusable`: roving-tabindex
/// descendants remain valid programmatic focus targets even though only one of
/// them participates in ordinary Tab traversal.
pub(crate) fn composite_focus_target(
    ir: &CoreIR,
    focused: WidgetId,
    direction: Option<FocusDirection>,
    boundary: Option<CompositeMove>,
) -> SemanticFocusNavigation {
    let Some((composite_id, item_role, orientation, _)) = containing_composite(ir, focused) else {
        return SemanticFocusNavigation::NotApplicable;
    };

    let movement = if let Some(boundary) = boundary {
        boundary
    } else {
        match (orientation, direction, ir.layout_direction) {
            (SemanticOrientation::Vertical, Some(FocusDirection::Up), _)
            | (
                SemanticOrientation::Horizontal,
                Some(FocusDirection::Left),
                fission_ir::LayoutDirection::LeftToRight,
            )
            | (
                SemanticOrientation::Horizontal,
                Some(FocusDirection::Right),
                fission_ir::LayoutDirection::RightToLeft,
            ) => CompositeMove::Previous,
            (SemanticOrientation::Vertical, Some(FocusDirection::Down), _)
            | (
                SemanticOrientation::Horizontal,
                Some(FocusDirection::Right),
                fission_ir::LayoutDirection::LeftToRight,
            )
            | (
                SemanticOrientation::Horizontal,
                Some(FocusDirection::Left),
                fission_ir::LayoutDirection::RightToLeft,
            ) => CompositeMove::Next,
            (_, Some(_), _) => return SemanticFocusNavigation::Handled(None),
            (_, None, _) => return SemanticFocusNavigation::NotApplicable,
        }
    };

    let items = composite_items(ir, composite_id, item_role);
    if items.is_empty() {
        return SemanticFocusNavigation::Handled(None);
    }
    let current_index = focused_composite_item(ir, focused, composite_id, &items);
    let target = match movement {
        CompositeMove::First => items.first().copied(),
        CompositeMove::Last => items.last().copied(),
        CompositeMove::Previous => current_index
            .map(|index| items[(index + items.len() - 1) % items.len()])
            .or_else(|| items.last().copied()),
        CompositeMove::Next => current_index
            .map(|index| items[(index + 1) % items.len()])
            .or_else(|| items.first().copied()),
    };
    SemanticFocusNavigation::Handled(target)
}

/// Finds the next matching Menu/ListBox item for one-character typeahead.
pub(crate) fn composite_typeahead_target(
    ir: &CoreIR,
    focused: WidgetId,
    character: char,
) -> SemanticFocusNavigation {
    let Some((composite_id, item_role, _, supports_typeahead)) = containing_composite(ir, focused)
    else {
        return SemanticFocusNavigation::NotApplicable;
    };
    if !supports_typeahead || character.is_control() {
        return SemanticFocusNavigation::NotApplicable;
    }

    let items = composite_items(ir, composite_id, item_role);
    if items.is_empty() {
        return SemanticFocusNavigation::Handled(None);
    }
    let needle = character.to_lowercase().collect::<String>();
    let start =
        focused_composite_item(ir, focused, composite_id, &items).map_or(0, |index| index + 1);
    let target = (0..items.len())
        .map(|offset| items[(start + offset) % items.len()])
        .find(|candidate| {
            semantics(ir, *candidate)
                .and_then(|value| value.label.as_deref())
                .is_some_and(|label| label.trim_start().to_lowercase().starts_with(&needle))
        });
    SemanticFocusNavigation::Handled(target)
}

fn popup_role(kind: PopupKind) -> Option<Role> {
    match kind {
        PopupKind::Menu => Some(Role::Menu),
        PopupKind::ListBox => Some(Role::ListBox),
        PopupKind::Tree | PopupKind::Grid | PopupKind::Dialog => None,
    }
}

fn controlled_popup(ir: &CoreIR, controller: WidgetId) -> Option<(WidgetId, Role, Vec<WidgetId>)> {
    let controller_semantics = semantics(ir, controller)?;
    if !controller_semantics.focusable
        || controller_semantics.disabled
        || controller_semantics.expanded != Some(true)
    {
        return None;
    }
    let popup_role = popup_role(controller_semantics.has_popup?)?;
    for popup_id in &controller_semantics.controls {
        let Some(popup_semantics) = semantics(ir, *popup_id) else {
            continue;
        };
        if popup_semantics.role != popup_role || is_interaction_inert(ir, *popup_id) {
            continue;
        }
        let item_role = composite_contract(popup_role)?.item_role;
        let items = composite_items(ir, *popup_id, item_role);
        return Some((*popup_id, item_role, items));
    }
    None
}

fn editable_combobox_popup(ir: &CoreIR, controller: WidgetId) -> Option<(WidgetId, Vec<WidgetId>)> {
    let controller_semantics = semantics(ir, controller)?;
    if controller_semantics.role != Role::ComboBox || !controller_semantics.text_editable {
        return None;
    }
    let (popup, item_role, items) = controlled_popup(ir, controller)?;
    (item_role == Role::Option).then_some((popup, items))
}

/// Returns whether `target` is an enabled option in the open list box owned by
/// an editable combobox controller.
pub(crate) fn is_valid_editable_combobox_active_descendant(
    ir: &CoreIR,
    controller: WidgetId,
    target: WidgetId,
) -> bool {
    editable_combobox_popup(ir, controller).is_some_and(|(_, items)| items.contains(&target))
}

/// Moves the virtual active option for an editable combobox while leaving
/// platform focus on its text controller.
///
/// On first entry, a valid semantic active descendant or selected option wins;
/// otherwise navigation starts at the directional boundary. Further presses
/// move relative to the runtime-owned option and wrap, skipping disabled items.
pub(crate) fn editable_combobox_popup_navigation_target(
    ir: &CoreIR,
    focused: WidgetId,
    runtime_active: Option<WidgetId>,
    reverse: bool,
) -> Option<(WidgetId, Option<WidgetId>)> {
    let controller = editable_combobox_popup(ir, focused)
        .map(|_| focused)
        .or_else(|| {
            popup_controller_for_descendant(ir, focused).and_then(|(controller, _)| {
                editable_combobox_popup(ir, controller).map(|_| controller)
            })
        })?;
    let (_, items) = editable_combobox_popup(ir, controller)?;
    if items.is_empty() {
        return Some((controller, None));
    }

    let current = runtime_active
        .filter(|candidate| items.contains(candidate))
        .or_else(|| (focused != controller && items.contains(&focused)).then_some(focused));
    let target = if let Some(current) = current {
        let index = items
            .iter()
            .position(|candidate| *candidate == current)
            .expect("active combobox option was filtered against the item list");
        let next = if reverse {
            index.checked_sub(1).unwrap_or(items.len() - 1)
        } else {
            (index + 1) % items.len()
        };
        Some(items[next])
    } else {
        semantics(ir, controller)
            .and_then(|value| value.active_descendant)
            .filter(|candidate| items.contains(candidate))
            .or_else(|| {
                items.iter().copied().find(|candidate| {
                    semantics(ir, *candidate).is_some_and(|value| value.selected == Some(true))
                })
            })
            .or_else(|| {
                if reverse {
                    items.last().copied()
                } else {
                    items.first().copied()
                }
            })
    };
    Some((controller, target))
}

/// Finds an enabled option and its editable combobox controller from a hit
/// descendant. Pointer activation can therefore preserve text focus while
/// updating the same virtual option used by keyboard navigation.
pub(crate) fn editable_combobox_option_for_descendant(
    ir: &CoreIR,
    descendant: WidgetId,
) -> Option<(WidgetId, WidgetId)> {
    let mut current = Some(descendant);
    while let Some(candidate) = current {
        if semantics(ir, candidate)
            .is_some_and(|value| value.role == Role::Option && value.focusable && !value.disabled)
        {
            let (controller, popup) = popup_controller_for_descendant(ir, candidate)?;
            let (owned_popup, items) = editable_combobox_popup(ir, controller)?;
            if popup == owned_popup && items.contains(&candidate) {
                return Some((controller, candidate));
            }
            return None;
        }
        current = ir.nodes.get(&candidate).and_then(|node| node.parent);
    }
    None
}

/// Finds the activation that opens a collapsed Menu/ListBox controlled by the
/// focused node or one of its semantic ancestors.
pub(crate) fn collapsed_popup_open_action(
    ir: &CoreIR,
    focused: WidgetId,
) -> Option<(WidgetId, ActionEntry)> {
    let mut controller = Some(focused);
    while let Some(controller_id) = controller {
        if let Some(semantics) = semantics(ir, controller_id) {
            let controls_supported_popup = semantics
                .has_popup
                .and_then(popup_role)
                .is_some_and(|_| !semantics.controls.is_empty());
            if semantics.focusable
                && !semantics.disabled
                && semantics.expanded == Some(false)
                && controls_supported_popup
            {
                return semantics
                    .actions
                    .entries
                    .iter()
                    .find(|entry| entry.trigger == ActionTrigger::Default)
                    .cloned()
                    .map(|entry| (controller_id, entry));
            }
        }
        controller = ir.nodes.get(&controller_id).and_then(|node| node.parent);
    }
    None
}

/// Enters an expanded, mounted Menu/ListBox controlled by the focused node.
/// Selected/active items win; otherwise Down enters the first item and Up the
/// last. Disabled descendants are already excluded by `composite_items`.
pub(crate) fn controlled_popup_entry_target(
    ir: &CoreIR,
    focused: WidgetId,
    reverse: bool,
) -> SemanticFocusNavigation {
    let mut controller = Some(focused);
    while let Some(controller_id) = controller {
        if let Some((_popup_id, _item_role, items)) = controlled_popup(ir, controller_id) {
            let controller_semantics = semantics(ir, controller_id);
            let preferred = controller_semantics
                .and_then(|value| value.active_descendant)
                .filter(|candidate| items.contains(candidate))
                .or_else(|| {
                    items.iter().copied().find(|candidate| {
                        semantics(ir, *candidate).is_some_and(|value| value.selected == Some(true))
                    })
                });
            let target = preferred.or_else(|| {
                if reverse {
                    items.last().copied()
                } else {
                    items.first().copied()
                }
            });
            return SemanticFocusNavigation::Handled(target);
        }
        controller = ir.nodes.get(&controller_id).and_then(|node| node.parent);
    }
    SemanticFocusNavigation::NotApplicable
}

fn collect_semantic_nodes_in_tree_order(
    ir: &CoreIR,
    node_id: WidgetId,
    result: &mut Vec<WidgetId>,
) {
    let Some(node) = ir.nodes.get(&node_id) else {
        return;
    };
    if matches!(
        node.op,
        Op::Structural(StructuralOp::InteractionInert { .. })
    ) {
        return;
    }
    if matches!(node.op, Op::Semantics(_)) {
        result.push(node_id);
    }
    for child in &node.children {
        collect_semantic_nodes_in_tree_order(ir, *child, result);
    }
}

/// Returns the focusable controller for the nearest controlled popup that
/// contains `descendant`, plus that popup's semantic node.
pub(crate) fn popup_controller_for_descendant(
    ir: &CoreIR,
    descendant: WidgetId,
) -> Option<(WidgetId, WidgetId)> {
    let mut semantic_nodes = Vec::new();
    if let Some(root) = ir.root {
        collect_semantic_nodes_in_tree_order(ir, root, &mut semantic_nodes);
    }

    let mut ancestor = Some(descendant);
    while let Some(popup_id) = ancestor {
        let is_popup = semantics(ir, popup_id)
            .is_some_and(|value| matches!(value.role, Role::Menu | Role::ListBox));
        if is_popup {
            let controller = semantic_nodes.iter().rev().copied().find(|candidate| {
                semantics(ir, *candidate).is_some_and(|value| {
                    value.focusable
                        && !value.disabled
                        && value.expanded == Some(true)
                        && value.controls.contains(&popup_id)
                        && value.has_popup.and_then(popup_role).is_some_and(|role| {
                            semantics(ir, popup_id).is_some_and(|s| s.role == role)
                        })
                })
            });
            if let Some(controller) = controller {
                return Some((controller, popup_id));
            }
        }
        ancestor = ir.nodes.get(&popup_id).and_then(|node| node.parent);
    }
    None
}

/// Resolves the explicit dismissal contract for a focused popup controller or
/// a focused descendant of its controlled Menu/ListBox.
///
/// The popup root must own Dismiss and an expanded focusable controller must
/// name that root through `controls`; ancestry or role alone is insufficient.
pub(crate) fn controlled_popup_dismissal_for_descendant(
    ir: &CoreIR,
    descendant: WidgetId,
) -> Option<(WidgetId, WidgetId, ActionEntry)> {
    let (controller, popup) = controlled_popup(ir, descendant)
        .map(|(popup, _, _)| (descendant, popup))
        .or_else(|| popup_controller_for_descendant(ir, descendant))?;
    let action = semantics(ir, popup)?
        .actions
        .entries
        .iter()
        .find(|entry| entry.trigger == ActionTrigger::Dismiss)?
        .clone();
    Some((controller, popup, action))
}

pub fn find_neighbor_focus_node(
    ir: &CoreIR,
    layout: &LayoutSnapshot,
    current: WidgetId,
    direction: FocusDirection,
) -> Option<WidgetId> {
    let current_rect = layout.get_node_rect(current)?;
    let focusable_nodes = get_all_focusable_nodes(ir);

    let mut best_candidate = None;
    let mut best_dist = f32::INFINITY;

    let (cx, cy) = (
        current_rect.x() + current_rect.width() / 2.0,
        current_rect.y() + current_rect.height() / 2.0,
    );

    for node_id in focusable_nodes {
        if node_id == current {
            continue;
        }
        let rect = match layout.get_node_rect(node_id) {
            Some(r) => r,
            None => continue,
        };

        let (nx, ny) = (
            rect.x() + rect.width() / 2.0,
            rect.y() + rect.height() / 2.0,
        );

        let is_in_dir = match direction {
            FocusDirection::Up => ny < cy && (nx - cx).abs() < (ny - cy).abs(),
            FocusDirection::Down => ny > cy && (nx - cx).abs() < (ny - cy).abs(),
            FocusDirection::Left => nx < cx && (ny - cy).abs() < (nx - cx).abs(),
            FocusDirection::Right => nx > cx && (ny - cy).abs() < (nx - cx).abs(),
        };

        if is_in_dir {
            let dist = (nx - cx).powi(2) + (ny - cy).powi(2);
            if dist < best_dist {
                best_dist = dist;
                best_candidate = Some(node_id);
            }
        }
    }

    best_candidate
}

#[cfg(test)]
mod canvas_hit_tests {
    use super::{
        canvas_target_hit, focus_barriers_in_tree_order, get_all_focusable_nodes, hit_test,
        is_enabled_focus_node, is_interaction_inert, preferred_focus_node_in_scope,
        topmost_semantics_action,
    };
    use crate::env::ScrollStateMap;
    use fission_ir::{
        ActionEntry, ActionSet, ActionTrigger, CanvasSelectionPolicy, CanvasTarget,
        CanvasTargetKind, CoreIR, Op, Role, Semantics, StructuralOp, WidgetId,
    };
    use fission_layout::{LayoutNodeGeometry, LayoutPoint, LayoutRect, LayoutSize, LayoutSnapshot};

    fn edge(points: Vec<[f32; 2]>, cubic: bool) -> CanvasTarget {
        CanvasTarget {
            canvas_id: 1,
            kind: CanvasTargetKind::Edge {
                edge_id: 2,
                points,
                cubic,
                hit_tolerance: 4.0,
            },
            selection_policy: CanvasSelectionPolicy::Single,
            snap_spacing: None,
            snap_threshold: 0.0,
        }
    }

    #[test]
    fn edge_hit_testing_uses_stroke_geometry_instead_of_its_bounding_box() {
        let straight = edge(vec![[10.0, 10.0], [90.0, 90.0]], false);
        assert!(canvas_target_hit(&straight, LayoutPoint::new(50.0, 52.0)));
        assert!(!canvas_target_hit(&straight, LayoutPoint::new(10.0, 90.0)));

        let polyline = edge(
            vec![[10.0, 10.0], [40.0, 10.0], [40.0, 80.0], [90.0, 80.0]],
            false,
        );
        assert!(canvas_target_hit(&polyline, LayoutPoint::new(75.0, 82.0)));
        assert!(!canvas_target_hit(&polyline, LayoutPoint::new(75.0, 20.0)));

        let cubic = edge(
            vec![[0.0, 50.0], [25.0, 0.0], [75.0, 100.0], [100.0, 50.0]],
            true,
        );
        assert!(canvas_target_hit(&cubic, LayoutPoint::new(50.0, 50.0)));
        assert!(!canvas_target_hit(&cubic, LayoutPoint::new(50.0, 5.0)));
    }

    #[test]
    fn pointer_transparent_groups_pass_hits_to_underlying_siblings() {
        let root = WidgetId::explicit("root");
        let background = WidgetId::explicit("background-button");
        let wrapper = WidgetId::explicit("pointer-transparent");
        let overlay_child = WidgetId::explicit("interactive-overlay-child");
        let mut ir = CoreIR::default();
        ir.add_node(
            background,
            Op::Semantics(Semantics {
                role: Role::Button,
                focusable: true,
                ..Semantics::default()
            }),
            Vec::new(),
        );
        ir.add_node(
            overlay_child,
            Op::Semantics(Semantics {
                role: Role::Button,
                focusable: true,
                ..Semantics::default()
            }),
            Vec::new(),
        );
        ir.add_node(
            wrapper,
            Op::Structural(StructuralOp::PointerTransparent { stable_hash: 1 }),
            vec![overlay_child],
        );
        ir.add_node(
            root,
            Op::Structural(StructuralOp::Group { stable_hash: 2 }),
            vec![background, wrapper],
        );
        ir.set_root(root);
        let mut layout = LayoutSnapshot::new(LayoutSize::new(100.0, 100.0));
        for id in [root, background, wrapper, overlay_child] {
            layout.nodes.insert(
                id,
                LayoutNodeGeometry {
                    rect: LayoutRect::new(0.0, 0.0, 100.0, 100.0),
                    content_size: LayoutSize::new(100.0, 100.0),
                },
            );
        }

        assert_eq!(
            hit_test(
                &ir,
                &layout,
                &ScrollStateMap::default(),
                LayoutPoint::new(50.0, 50.0),
            ),
            Some(background)
        );
    }

    #[test]
    fn interaction_inert_groups_are_visual_only_for_pointer_focus_and_barriers() {
        let root = WidgetId::explicit("root");
        let background = WidgetId::explicit("background-button");
        let wrapper = WidgetId::explicit("interaction-inert");
        let barrier = WidgetId::explicit("exiting-focus-barrier");
        let overlay_child = WidgetId::explicit("exiting-button");
        let mut ir = CoreIR::default();
        ir.add_node(
            background,
            Op::Semantics(Semantics {
                role: Role::Button,
                focusable: true,
                ..Semantics::default()
            }),
            Vec::new(),
        );
        ir.add_node(
            overlay_child,
            Op::Semantics(Semantics {
                role: Role::Button,
                focusable: true,
                ..Semantics::default()
            }),
            Vec::new(),
        );
        ir.add_node(
            barrier,
            Op::Semantics(Semantics {
                is_focus_scope: true,
                is_focus_barrier: true,
                ..Semantics::default()
            }),
            vec![overlay_child],
        );
        ir.add_node(
            wrapper,
            Op::Structural(StructuralOp::InteractionInert { stable_hash: 1 }),
            vec![barrier],
        );
        ir.add_node(
            root,
            Op::Structural(StructuralOp::Group { stable_hash: 2 }),
            vec![background, wrapper],
        );
        ir.set_root(root);
        let mut layout = LayoutSnapshot::new(LayoutSize::new(100.0, 100.0));
        for id in [root, background, wrapper, barrier, overlay_child] {
            layout.nodes.insert(
                id,
                LayoutNodeGeometry {
                    rect: LayoutRect::new(0.0, 0.0, 100.0, 100.0),
                    content_size: LayoutSize::new(100.0, 100.0),
                },
            );
        }

        assert_eq!(
            hit_test(
                &ir,
                &layout,
                &ScrollStateMap::default(),
                LayoutPoint::new(50.0, 50.0),
            ),
            Some(background)
        );
        assert_eq!(get_all_focusable_nodes(&ir), vec![background]);
        assert!(focus_barriers_in_tree_order(&ir).is_empty());
        assert!(!is_enabled_focus_node(&ir, overlay_child));
        assert!(is_interaction_inert(&ir, overlay_child));
    }

    #[test]
    fn programmatic_focus_targets_can_opt_out_of_sequential_traversal() {
        let root = WidgetId::explicit("root");
        let regular = WidgetId::explicit("regular");
        let composite_child = WidgetId::explicit("composite-child");
        let mut ir = CoreIR::default();
        ir.add_node(
            regular,
            Op::Semantics(Semantics {
                role: Role::Button,
                focusable: true,
                ..Semantics::default()
            }),
            Vec::new(),
        );
        ir.add_node(
            composite_child,
            Op::Semantics(Semantics {
                role: Role::Option,
                focusable: true,
                sequential_focusable: false,
                ..Semantics::default()
            }),
            Vec::new(),
        );
        ir.add_node(
            root,
            Op::Structural(StructuralOp::Group { stable_hash: 1 }),
            vec![regular, composite_child],
        );
        ir.set_root(root);

        assert_eq!(get_all_focusable_nodes(&ir), vec![regular]);
        assert!(is_enabled_focus_node(&ir, composite_child));
    }

    #[test]
    fn focus_scope_entry_prefers_sequential_then_programmatic_only_fallback() {
        let scope = WidgetId::explicit("scope");
        let surface = WidgetId::explicit("programmatic-surface");
        let control = WidgetId::explicit("sequential-control");
        let build_ir = |with_control| {
            let mut ir = CoreIR::default();
            ir.add_node(
                surface,
                Op::Semantics(Semantics {
                    role: Role::Dialog,
                    focusable: true,
                    sequential_focusable: false,
                    ..Semantics::default()
                }),
                Vec::new(),
            );
            let mut children = vec![surface];
            if with_control {
                ir.add_node(
                    control,
                    Op::Semantics(Semantics {
                        role: Role::Button,
                        focusable: true,
                        sequential_focusable: true,
                        ..Semantics::default()
                    }),
                    Vec::new(),
                );
                children.push(control);
            }
            ir.add_node(
                scope,
                Op::Semantics(Semantics {
                    is_focus_scope: true,
                    ..Semantics::default()
                }),
                children,
            );
            ir.set_root(scope);
            ir
        };

        assert_eq!(
            preferred_focus_node_in_scope(&build_ir(true), scope),
            Some(control),
            "an earlier programmatic-only surface must not displace a sequential control"
        );
        assert_eq!(
            preferred_focus_node_in_scope(&build_ir(false), scope),
            Some(surface),
            "a scope without sequential controls must retain a programmatic entry target"
        );
    }

    #[test]
    fn topmost_action_ignores_exiting_interaction_inert_overlays() {
        let root = WidgetId::explicit("root");
        let active = WidgetId::explicit("active-overlay");
        let inert = WidgetId::explicit("inert-overlay");
        let inert_action = WidgetId::explicit("inert-action");
        let action = |action_id| ActionSet {
            entries: vec![ActionEntry {
                trigger: ActionTrigger::Dismiss,
                action_id,
                payload_data: Some(Vec::new()),
            }],
        };
        let mut ir = CoreIR::default();
        ir.add_node(
            active,
            Op::Semantics(Semantics {
                actions: action(7),
                ..Semantics::default()
            }),
            Vec::new(),
        );
        ir.add_node(
            inert_action,
            Op::Semantics(Semantics {
                actions: action(9),
                ..Semantics::default()
            }),
            Vec::new(),
        );
        ir.add_node(
            inert,
            Op::Structural(StructuralOp::InteractionInert { stable_hash: 2 }),
            vec![inert_action],
        );
        ir.add_node(
            root,
            Op::Structural(StructuralOp::Group { stable_hash: 1 }),
            vec![active, inert],
        );
        ir.set_root(root);

        let (target, entry) =
            topmost_semantics_action(&ir, ActionTrigger::Dismiss).expect("active dismissal action");
        assert_eq!(target, active);
        assert_eq!(entry.action_id, 7);
    }
}
