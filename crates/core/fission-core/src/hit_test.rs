use fission_ir::{CoreIR, NodeId, Op, PaintOp};
use fission_layout::{LayoutSnapshot, LayoutPoint, LayoutRect, LayoutUnit};

pub fn hit_test(
    ir: &CoreIR,
    layout: &LayoutSnapshot,
    point: LayoutPoint,
) -> Option<NodeId> {
    let mut last_hit: Option<NodeId> = None;
    
    if let Some(root) = ir.root {
        hit_test_recursive(root, ir, layout, point, &mut last_hit);
    }
    
    last_hit
}

fn hit_test_recursive(
    node_id: NodeId,
    ir: &CoreIR,
    layout: &LayoutSnapshot,
    point: LayoutPoint,
    last_hit: &mut Option<NodeId>,
) {
    let mut current_is_hit = false;
    if let Some(geom) = layout.get_node_geometry(node_id) {
        if geom.rect.contains(point) {
            current_is_hit = true; // Bounding box hit

            // Refine hit check based on the actual Op
            if let Some(node_ir) = ir.nodes.get(&node_id) {
                match &node_ir.op {
                    Op::Paint(PaintOp::DrawRect { corner_radius, .. }) => {
                        // Perform rounded rect hit test
                        current_is_hit = is_point_in_rounded_rect(point, geom.rect, *corner_radius);
                    }
                    // For other ops or layout ops, keep bounding box hit for now.
                    // More complex shapes could have custom hit-testing here.
                    _ => {}
                }
            }
        }
    }

    if current_is_hit {
        *last_hit = Some(node_id);
    }

    // Recurse into children
    if let Some(node) = ir.nodes.get(&node_id) {
        for child_id in &node.children {
            hit_test_recursive(*child_id, ir, layout, point, last_hit);
        }
    }
}

fn is_point_in_rounded_rect(p: LayoutPoint, r: LayoutRect, radius: LayoutUnit) -> bool {
    // Translate point to local coordinates of the rectangle
    let local_p_x = p.x - r.x();
    let local_p_y = p.y - r.y();

    let (width, height) = (r.width(), r.height());

    // Handle zero or negative radius (effectively a normal rect)
    if radius <= 0.0 {
        return true; // Already checked by geom.rect.contains(point)
    }

    // Clamp radius to half of the smallest dimension to prevent self-intersection
    let clamped_radius = radius.min(width / 2.0).min(height / 2.0);

    // Check corners
    // Top-left corner region
    if local_p_x < clamped_radius && local_p_y < clamped_radius {
        return (local_p_x - clamped_radius).powi(2) + (local_p_y - clamped_radius).powi(2) <= clamped_radius.powi(2);
    }
    // Top-right corner region
    if local_p_x > width - clamped_radius && local_p_y < clamped_radius {
        return (local_p_x - (width - clamped_radius)).powi(2) + (local_p_y - clamped_radius).powi(2) <= clamped_radius.powi(2);
    }
    // Bottom-left corner region
    if local_p_x < clamped_radius && local_p_y > height - clamped_radius {
        return (local_p_x - clamped_radius).powi(2) + (local_p_y - (height - clamped_radius)).powi(2) <= clamped_radius.powi(2);
    }
    // Bottom-right corner region
    if local_p_x > width - clamped_radius && local_p_y > height - clamped_radius {
        return (local_p_x - (width - clamped_radius)).powi(2) + (local_p_y - (height - clamped_radius)).powi(2) <= clamped_radius.powi(2);
    }

    // If not in a corner region (or radius is 0), and within bounding box, it's a hit.
    // The outer check `geom.rect.contains(point)` already handles this.
    true
}
