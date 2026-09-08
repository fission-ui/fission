use std::collections::BTreeSet;

use fission_core::ui::{Container, Widget, ZStack};
use fission_core::{ActionEnvelope, WidgetId};
use fission_ir::{op::Color, CanvasNodeMoveTarget, CanvasTarget, CanvasTargetKind};

use super::{
    geometry::ordered_nodes, interaction_region::CanvasInteractionRegion, CanvasNodeId,
    CanvasSelectionPolicy, CanvasSnap, InfiniteCanvasNode,
};

#[derive(Debug, Clone)]
pub(crate) struct InfiniteCanvasNodeLayer {
    pub canvas_id: WidgetId,
    pub nodes: Vec<InfiniteCanvasNode>,
    pub selection: BTreeSet<CanvasNodeId>,
    pub selection_color: Color,
    pub selection_policy: CanvasSelectionPolicy,
    pub snap: CanvasSnap,
    pub on_selection_change: Option<ActionEnvelope>,
    pub on_node_move: Option<ActionEnvelope>,
}

impl From<InfiniteCanvasNodeLayer> for Widget {
    fn from(layer: InfiniteCanvasNodeLayer) -> Self {
        let children = ordered_nodes(&layer.nodes)
            .into_iter()
            .map(|node| node_widget(&layer, node))
            .collect();
        ZStack { id: None, children }.into()
    }
}

fn node_widget(layer: &InfiniteCanvasNodeLayer, node: &InfiniteCanvasNode) -> Widget {
    let selected = layer.selection.contains(&node.id);
    let content = Container::new(node.child.clone())
        .width(node.bounds.width().max(0.0))
        .height(node.bounds.height().max(0.0));
    let content = if selected {
        content.border(layer.selection_color, 2.0)
    } else {
        content
    };
    let node_id = node.id.widget_id(layer.canvas_id);
    let region = CanvasInteractionRegion {
        id: node_id,
        identifier: format!("infinite-canvas-node:{:032x}", node.id.0),
        child: content.into(),
        target: CanvasTarget {
            canvas_id: layer.canvas_id.as_u128(),
            kind: CanvasTargetKind::Node {
                node_id: node.id.0,
                bounds: bounds(node.bounds),
                move_members: move_members(layer, node),
            },
            selection_policy: layer.selection_policy,
            snap_spacing: layer.snap.enabled.then_some(layer.snap.spacing),
            snap_threshold: layer.snap.threshold,
        },
        on_activate: layer.on_selection_change.clone(),
        on_drag: node.movable.then(|| layer.on_node_move.clone()).flatten(),
    };
    Container::new(region)
        .positioned(Some(node.bounds.x()), Some(node.bounds.y()), None, None)
        .width(node.bounds.width().max(0.0))
        .height(node.bounds.height().max(0.0))
        .into()
}

fn move_members(
    layer: &InfiniteCanvasNodeLayer,
    primary: &InfiniteCanvasNode,
) -> Vec<CanvasNodeMoveTarget> {
    if !primary.movable {
        return Vec::new();
    }
    let mut members = vec![CanvasNodeMoveTarget {
        node_id: primary.id.0,
        bounds: bounds(primary.bounds),
    }];
    if layer.selection.contains(&primary.id) {
        members.extend(
            ordered_nodes(&layer.nodes)
                .into_iter()
                .filter(|node| {
                    node.id != primary.id && node.movable && layer.selection.contains(&node.id)
                })
                .map(|node| CanvasNodeMoveTarget {
                    node_id: node.id.0,
                    bounds: bounds(node.bounds),
                }),
        );
    }
    members
}

fn bounds(rect: fission_layout::LayoutRect) -> [f32; 4] {
    [rect.x(), rect.y(), rect.width(), rect.height()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use fission_core::ui::Spacer;
    use fission_layout::LayoutRect;

    #[test]
    fn node_widget_identity_is_scoped_by_canvas() {
        let node = InfiniteCanvasNode::new(
            CanvasNodeId::from_u128(42),
            LayoutRect::new(0.0, 0.0, 20.0, 20.0),
            Spacer::default(),
        );
        let first = node.id.widget_id(WidgetId::explicit("first"));
        let second = node.id.widget_id(WidgetId::explicit("second"));
        assert_ne!(first, second);
        assert_eq!(first, node.id.widget_id(WidgetId::explicit("first")));
    }

    #[test]
    fn selected_move_members_keep_primary_first_and_stable_z_order() {
        let mut first = InfiniteCanvasNode::new(
            CanvasNodeId(1),
            LayoutRect::new(0.0, 0.0, 20.0, 20.0),
            Spacer::default(),
        );
        first.z_index = 2;
        let mut primary = InfiniteCanvasNode::new(
            CanvasNodeId(2),
            LayoutRect::new(30.0, 0.0, 20.0, 20.0),
            Spacer::default(),
        );
        primary.z_index = 1;
        let mut last = InfiniteCanvasNode::new(
            CanvasNodeId(3),
            LayoutRect::new(60.0, 0.0, 20.0, 20.0),
            Spacer::default(),
        );
        last.z_index = 3;
        let layer = InfiniteCanvasNodeLayer {
            canvas_id: WidgetId::explicit("canvas"),
            nodes: vec![first, primary.clone(), last],
            selection: [CanvasNodeId(1), CanvasNodeId(2), CanvasNodeId(3)]
                .into_iter()
                .collect(),
            selection_color: Color::BLACK,
            selection_policy: CanvasSelectionPolicy::Single,
            snap: CanvasSnap::default(),
            on_selection_change: None,
            on_node_move: None,
        };

        assert_eq!(
            move_members(&layer, &primary)
                .into_iter()
                .map(|member| member.node_id)
                .collect::<Vec<_>>(),
            vec![2, 1, 3]
        );
    }
}
