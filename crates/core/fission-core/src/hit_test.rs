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
            current_is_hit = true;

            if let Some(node_ir) = ir.nodes.get(&node_id) {
                match &node_ir.op {
                    Op::Paint(PaintOp::DrawRect { corner_radius, .. }) => {
                        current_is_hit = is_point_in_rounded_rect(point, geom.rect, *corner_radius);
                    }
                    _ => {}
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

pub fn find_next_focus_node(
    ir: &CoreIR,
    current_focus: Option<NodeId>,
    reverse: bool,
) -> Option<NodeId> {
    let mut focusable_nodes = Vec::new();
    if let Some(root) = ir.root {
        collect_focusable_nodes(root, ir, &mut focusable_nodes);
    }

    if focusable_nodes.is_empty() {
        return None;
    }

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
