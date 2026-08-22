use std::collections::HashMap;

use fission_layout::{LayoutPoint, LayoutRect};

use super::{
    CanvasEdgeEndpoint, CanvasEdgeRoute, CanvasNodeAnchor, CanvasNodeId, InfiniteCanvasEdge,
    InfiniteCanvasNode,
};

pub(crate) fn ordered_nodes(nodes: &[InfiniteCanvasNode]) -> Vec<&InfiniteCanvasNode> {
    let mut indexed = nodes.iter().enumerate().collect::<Vec<_>>();
    indexed.sort_by_key(|(order, node)| (node.z_index, *order));
    indexed.into_iter().map(|(_, node)| node).collect()
}

pub(crate) fn node_bounds(nodes: &[InfiniteCanvasNode]) -> HashMap<CanvasNodeId, LayoutRect> {
    nodes.iter().map(|node| (node.id, node.bounds)).collect()
}

pub(crate) fn resolve_endpoint(
    endpoint: CanvasEdgeEndpoint,
    nodes: &HashMap<CanvasNodeId, LayoutRect>,
) -> Option<LayoutPoint> {
    match endpoint {
        CanvasEdgeEndpoint::Point(point) => Some(point),
        CanvasEdgeEndpoint::Node { node, anchor } => nodes
            .get(&node)
            .copied()
            .map(|rect| anchor_point(rect, anchor)),
    }
}

pub(crate) fn edge_path(
    edge: &InfiniteCanvasEdge,
    nodes: &HashMap<CanvasNodeId, LayoutRect>,
    origin: LayoutPoint,
) -> Option<String> {
    let from = offset(resolve_endpoint(edge.from, nodes)?, origin);
    let to = offset(resolve_endpoint(edge.to, nodes)?, origin);
    Some(match edge.route {
        CanvasEdgeRoute::Straight => {
            format!("M{} {} L{} {}", from.x, from.y, to.x, to.y)
        }
        CanvasEdgeRoute::Cubic {
            first_control,
            second_control,
        } => {
            let first = offset(first_control, origin);
            let second = offset(second_control, origin);
            format!(
                "M{} {} C{} {},{} {},{} {}",
                from.x, from.y, first.x, first.y, second.x, second.y, to.x, to.y
            )
        }
    })
}

pub(crate) fn visible_world_rect(
    viewport_width: f32,
    viewport_height: f32,
    translation_x: f32,
    translation_y: f32,
    scale: f32,
    overscan: f32,
) -> LayoutRect {
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    let left = -translation_x / scale - overscan;
    let top = -translation_y / scale - overscan;
    LayoutRect::new(
        left,
        top,
        viewport_width.max(0.0) / scale + overscan * 2.0,
        viewport_height.max(0.0) / scale + overscan * 2.0,
    )
}

pub(crate) fn intersects(first: LayoutRect, second: LayoutRect) -> bool {
    first.right() >= second.x()
        && second.right() >= first.x()
        && first.bottom() >= second.y()
        && second.bottom() >= first.y()
}

fn anchor_point(rect: LayoutRect, anchor: CanvasNodeAnchor) -> LayoutPoint {
    let center = LayoutPoint::new(
        rect.x() + rect.width() * 0.5,
        rect.y() + rect.height() * 0.5,
    );
    match anchor {
        CanvasNodeAnchor::Center => center,
        CanvasNodeAnchor::Top => LayoutPoint::new(center.x, rect.y()),
        CanvasNodeAnchor::Right => LayoutPoint::new(rect.right(), center.y),
        CanvasNodeAnchor::Bottom => LayoutPoint::new(center.x, rect.bottom()),
        CanvasNodeAnchor::Left => LayoutPoint::new(rect.x(), center.y),
    }
}

fn offset(point: LayoutPoint, origin: LayoutPoint) -> LayoutPoint {
    LayoutPoint::new(point.x - origin.x, point.y - origin.y)
}

#[cfg(test)]
mod tests {
    use fission_core::ui::Spacer;

    use super::*;
    use crate::infinite_canvas::{CanvasEdgeId, CanvasNodeId};
    use fission_ir::op::{Color, Fill, LineCap, LineJoin, Stroke};

    fn node(id: u128, z_index: i32) -> InfiniteCanvasNode {
        InfiniteCanvasNode {
            id: CanvasNodeId(id),
            bounds: LayoutRect::new(id as f32, 0.0, 20.0, 10.0),
            z_index,
            child: Spacer::default().into(),
            movable: true,
            resizable: true,
        }
    }

    fn stroke() -> Stroke {
        Stroke {
            fill: Fill::Solid(Color::BLACK),
            width: 1.0,
            dash_array: None,
            line_cap: LineCap::Butt,
            line_join: LineJoin::Miter,
        }
    }

    #[test]
    fn z_order_is_stable_for_equal_indices() {
        let nodes = vec![node(1, 4), node(2, -1), node(3, 4)];
        let ids = ordered_nodes(&nodes)
            .into_iter()
            .map(|node| node.id)
            .collect::<Vec<_>>();
        assert_eq!(ids, vec![CanvasNodeId(2), CanvasNodeId(1), CanvasNodeId(3)]);
    }

    #[test]
    fn node_anchors_and_cubic_paths_resolve_in_world_space() {
        let nodes = vec![
            InfiniteCanvasNode::new(
                CanvasNodeId(1),
                LayoutRect::new(-20.0, 10.0, 40.0, 20.0),
                Spacer::default(),
            ),
            InfiniteCanvasNode::new(
                CanvasNodeId(2),
                LayoutRect::new(100.0, 20.0, 20.0, 40.0),
                Spacer::default(),
            ),
        ];
        let edge = InfiniteCanvasEdge {
            id: CanvasEdgeId(3),
            from: CanvasEdgeEndpoint::Node {
                node: CanvasNodeId(1),
                anchor: CanvasNodeAnchor::Right,
            },
            to: CanvasEdgeEndpoint::Node {
                node: CanvasNodeId(2),
                anchor: CanvasNodeAnchor::Left,
            },
            route: CanvasEdgeRoute::Cubic {
                first_control: LayoutPoint::new(40.0, 20.0),
                second_control: LayoutPoint::new(80.0, 40.0),
            },
            stroke: stroke(),
            label: None,
        };
        assert_eq!(
            edge_path(&edge, &node_bounds(&nodes), LayoutPoint::new(-20.0, 10.0)).as_deref(),
            Some("M40 10 C60 10,100 30,120 30")
        );
    }

    #[test]
    fn visible_world_rect_inverts_the_camera() {
        assert_eq!(
            visible_world_rect(800.0, 600.0, -100.0, 50.0, 2.0, 10.0),
            LayoutRect::new(40.0, -35.0, 420.0, 320.0)
        );
    }
}
