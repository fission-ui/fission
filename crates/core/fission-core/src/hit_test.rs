use crate::env::ScrollStateMap;
use fission_ir::{CoreIR, LayoutOp, NodeId, Op, PaintOp};
use fission_layout::{LayoutPoint, LayoutRect, LayoutSnapshot, LayoutUnit};

pub fn hit_test(ir: &CoreIR, layout: &LayoutSnapshot, point: LayoutPoint) -> Option<NodeId> {
    hit_test_internal(ir, layout, None, point)
}

pub fn hit_test_with_scroll(
    ir: &CoreIR,
    layout: &LayoutSnapshot,
    scroll_map: &ScrollStateMap,
    point: LayoutPoint,
) -> Option<NodeId> {
    hit_test_internal(ir, layout, Some(scroll_map), point)
}

fn hit_test_internal(
    ir: &CoreIR,
    layout: &LayoutSnapshot,
    scroll_map: Option<&ScrollStateMap>,
    point: LayoutPoint,
) -> Option<NodeId> {
    let mut last_hit: Option<NodeId> = None;

    if let Some(root) = ir.root {
        hit_test_recursive(root, ir, layout, scroll_map, point, &mut last_hit);
    }

    last_hit
}

fn hit_test_recursive(
    node_id: NodeId,
    ir: &CoreIR,
    layout: &LayoutSnapshot,
    scroll_map: Option<&ScrollStateMap>,
    point: LayoutPoint,
    last_hit: &mut Option<NodeId>,
) {
    let mut current_is_hit = false;
    if let Some(geom) = layout.get_node_geometry(node_id) {
        if geom.rect.contains(point) {
            if let Some(node_ir) = ir.nodes.get(&node_id) {
                match &node_ir.op {
                    Op::Paint(PaintOp::DrawRect { corner_radius, .. }) => {
                        current_is_hit = is_point_in_rounded_rect(point, geom.rect, *corner_radius);
                    }
                    Op::Paint(_) | Op::Layout(LayoutOp::Scroll { .. }) | Op::Layout(LayoutOp::Embed { .. }) => {
                        current_is_hit = true;
                    }
                    _ => {
                        current_is_hit = false;
                    }
                }
            }
        }
    }

    if current_is_hit {
        *last_hit = Some(node_id);
    }

    if let Some(node) = ir.nodes.get(&node_id) {
        let mut child_point = point;

        if let (Some(map), Op::Layout(LayoutOp::Scroll { direction, .. })) = (scroll_map, &node.op)
        {
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

        for child_id in &node.children {
            hit_test_recursive(*child_id, ir, layout, scroll_map, child_point, last_hit);
        }
    }
}

fn is_point_in_rounded_rect(p: LayoutPoint, r: LayoutRect, radius: LayoutUnit) -> bool {
    let local_p_x = p.x - r.x();
    let local_p_y = p.y - r.y();

    let (width, height) = (r.width(), r.height());

    if radius <= 0.0 {
        return true;
    }

    let clamped_radius = radius.min(width / 2.0).min(height / 2.0);

    if local_p_x < clamped_radius && local_p_y < clamped_radius {
        return (local_p_x - clamped_radius).powi(2) + (local_p_y - clamped_radius).powi(2)
            <= clamped_radius.powi(2);
    }
    if local_p_x > width - clamped_radius && local_p_y < clamped_radius {
        return (local_p_x - (width - clamped_radius)).powi(2)
            + (local_p_y - clamped_radius).powi(2)
            <= clamped_radius.powi(2);
    }
    if local_p_x < clamped_radius && local_p_y > height - clamped_radius {
        return (local_p_x - clamped_radius).powi(2)
            + (local_p_y - (height - clamped_radius)).powi(2)
            <= clamped_radius.powi(2);
    }
    if local_p_x > width - clamped_radius && local_p_y > height - clamped_radius {
        return (local_p_x - (width - clamped_radius)).powi(2)
            + (local_p_y - (height - clamped_radius)).powi(2)
            <= clamped_radius.powi(2);
    }

    true
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusDirection {
    Up,
    Down,
    Left,
    Right,
}

pub fn find_focus_search_root(ir: &CoreIR, current_focus: Option<NodeId>) -> Option<NodeId> {
    if let Some(curr) = current_focus {
        let mut cur = Some(curr);
        while let Some(id) = cur {
            if let Some(node) = ir.nodes.get(&id) {
                if let Op::Semantics(s) = &node.op {
                    if s.is_focus_scope && s.is_focus_barrier {
                        return Some(id);
                    }
                }
                cur = node.parent;
            } else {
                break;
            }
        }
    }
    ir.root
}

pub fn find_next_focus_node(
    ir: &CoreIR,
    current_focus: Option<NodeId>,
    reverse: bool,
) -> Option<NodeId> {
    let mut focusable_nodes = Vec::new();
    let search_root = find_focus_search_root(ir, current_focus);

    if let Some(root) = search_root {
        collect_focusable_nodes(root, ir, &mut focusable_nodes);
    }

    if focusable_nodes.is_empty() {
        return None;
    }

    // Sort focusable nodes by explicit focus_index, then stable tree order
    focusable_nodes.sort_by(|a, b| {
        let sem_a = if let Op::Semantics(s) = &ir.nodes.get(a).unwrap().op {
            s
        } else {
            unreachable!()
        };
        let sem_b = if let Op::Semantics(s) = &ir.nodes.get(b).unwrap().op {
            s
        } else {
            unreachable!()
        };

        match (sem_a.focus_index, sem_b.focus_index) {
            (Some(ia), Some(ib)) => ia.cmp(&ib),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        }
    });

    if let Some(curr) = current_focus {
        if let Some(idx) = focusable_nodes.iter().position(|&id| id == curr) {
            if reverse {
                if idx == 0 {
                    return Some(*focusable_nodes.last().unwrap());
                } else {
                    return Some(focusable_nodes[idx - 1]);
                }
            } else {
                if idx == focusable_nodes.len() - 1 {
                    return Some(focusable_nodes[0]);
                } else {
                    return Some(focusable_nodes[idx + 1]);
                }
            }
        }
    }

    // Default to first (or last if reverse)
    if reverse {
        Some(*focusable_nodes.last().unwrap())
    } else {
        Some(focusable_nodes[0])
    }
}

pub fn find_neighbor_focus_node(
    ir: &CoreIR,
    layout: &LayoutSnapshot,
    current_focus: NodeId,
    direction: FocusDirection,
) -> Option<NodeId> {
    let current_rect = layout.get_node_rect(current_focus)?;
    let center = LayoutPoint::new(
        current_rect.x() + current_rect.width() / 2.0,
        current_rect.y() + current_rect.height() / 2.0,
    );

    let mut focusable_nodes = Vec::new();
    let search_root = find_focus_search_root(ir, Some(current_focus));
    if let Some(root) = search_root {
        collect_focusable_nodes(root, ir, &mut focusable_nodes);
    }

    let mut best_node = None;
    let mut best_dist = f32::MAX;

    for id in focusable_nodes {
        if id == current_focus {
            continue;
        }
        if let Some(rect) = layout.get_node_rect(id) {
            let other_center = LayoutPoint::new(
                rect.x() + rect.width() / 2.0,
                rect.y() + rect.height() / 2.0,
            );

            let dx = other_center.x - center.x;
            let dy = other_center.y - center.y;

            let is_in_direction = match direction {
                FocusDirection::Up => dy < 0.0 && dx.abs() <= rect.width(),
                FocusDirection::Down => dy > 0.0 && dx.abs() <= rect.width(),
                FocusDirection::Left => dx < 0.0 && dy.abs() <= rect.height(),
                FocusDirection::Right => dx > 0.0 && dy.abs() <= rect.height(),
            };

            if is_in_direction {
                let dist = dx * dx + dy * dy;
                if dist < best_dist {
                    best_dist = dist;
                    best_node = Some(id);
                }
            }
        }
    }

    best_node
}

fn collect_focusable_nodes(node_id: NodeId, ir: &CoreIR, list: &mut Vec<NodeId>) {
    if let Some(node) = ir.nodes.get(&node_id) {
        if let Op::Semantics(s) = &node.op {
            if s.focusable {
                list.push(node_id);
            }
        }

        for child in &node.children {
            collect_focusable_nodes(*child, ir, list);
        }
    }
}
