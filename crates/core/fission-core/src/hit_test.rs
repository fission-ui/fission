use crate::env::ScrollStateMap;
use crate::ui::custom_render::downcast_render_object;
use fission_diagnostics::prelude as diag;
use fission_ir::{CoreIR, LayoutOp, Op, WidgetId};
use fission_layout::{LayoutPoint, LayoutSnapshot};
use glam::{Mat4, Vec4};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusDirection {
    Up,
    Down,
    Left,
    Right,
}

pub fn hit_test(
    ir: &CoreIR,
    layout: &LayoutSnapshot,
    scroll_map: &ScrollStateMap,
    point: LayoutPoint,
) -> Option<WidgetId> {
    hit_test_internal(ir, layout, Some(scroll_map), point)
}

pub fn hit_test_with_scroll(
    ir: &CoreIR,
    layout: &LayoutSnapshot,
    scroll_map: &ScrollStateMap,
    point: LayoutPoint,
) -> Option<WidgetId> {
    hit_test_internal(ir, layout, Some(scroll_map), point)
}

fn hit_test_internal(
    ir: &CoreIR,
    layout: &LayoutSnapshot,
    scroll_map: Option<&ScrollStateMap>,
    point: LayoutPoint,
) -> Option<WidgetId> {
    let result = ir
        .root
        .and_then(|root| hit_test_recursive(root, ir, layout, scroll_map, point));

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
    point: LayoutPoint,
) -> Option<WidgetId> {
    let node = ir.nodes.get(&node_id)?;
    let geom = layout.get_node_geometry(node_id)?;

    let is_clip_container = matches!(
        node.op,
        Op::Layout(LayoutOp::Clip { .. }) | Op::Layout(LayoutOp::Scroll { .. })
    );

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

    for child_id in node.children.iter().rev() {
        if let Some(hit) = hit_test_recursive(*child_id, ir, layout, scroll_map, child_point) {
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

    let mut current_is_hit = false;
    if geom.rect.contains(point) {
        match &node.op {
            Op::Layout(LayoutOp::Scroll { .. }) | Op::Layout(LayoutOp::Embed { .. }) => {
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

pub fn find_next_focus_node(
    ir: &CoreIR,
    current: Option<WidgetId>,
    reverse: bool,
) -> Option<WidgetId> {
    // Identify current scope if focused node is provided
    let (current_scope_id, current_is_barrier) = if let Some(id) = current {
        let scope = find_parent_scope(id, ir);
        let mut is_barrier = false;
        if let Some(sid) = scope {
            if let Some(node) = ir.nodes.get(&sid) {
                if let Op::Semantics(s) = &node.op {
                    is_barrier = s.is_focus_barrier;
                }
            }
        }
        (scope, is_barrier)
    } else {
        (None, false)
    };

    let nodes_in_scope = if current_is_barrier {
        let scope_id = current_scope_id.unwrap();
        let mut list = Vec::new();
        // Start recursion on CHILDREN of the barrier root to avoid skipping it
        if let Some(node) = ir.nodes.get(&scope_id) {
            for child in &node.children {
                collect_focusable_nodes(*child, ir, &mut list, true, 0);
            }
        }
        sort_focusable_nodes(ir, list)
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

pub fn get_all_focusable_nodes(ir: &CoreIR) -> Vec<WidgetId> {
    let mut list = Vec::new();
    if let Some(root) = ir.root {
        collect_focusable_nodes(root, ir, &mut list, false, 0);
    }
    sort_focusable_nodes(ir, list)
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
    mut order: usize,
) {
    if let Some(node) = ir.nodes.get(&node_id) {
        let mut is_barrier = false;
        if let Op::Semantics(s) = &node.op {
            if s.focusable && !s.disabled {
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
            collect_focusable_nodes(child, ir, list, stop_at_barriers, order);
            order = list.last().map(|(_, o)| *o + 1).unwrap_or(order);
        }
    }
}

fn find_parent_scope(node_id: WidgetId, ir: &CoreIR) -> Option<WidgetId> {
    let mut curr = ir.nodes.get(&node_id)?.parent;
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
