use fission_core::ui::{Container, Widget, ZStack};
use fission_core::{ActionEnvelope, WidgetId};
use fission_ir::{CanvasTarget, CanvasTargetKind};

use super::{
    geometry::ordered_nodes, interaction_region::CanvasInteractionRegion, CanvasSelectionPolicy,
    CanvasSnap, InfiniteCanvasNode,
};

#[derive(Debug, Clone)]
pub(crate) struct InfiniteCanvasPortLayer {
    pub canvas_id: WidgetId,
    pub nodes: Vec<InfiniteCanvasNode>,
    pub selection_policy: CanvasSelectionPolicy,
    pub snap: CanvasSnap,
    pub on_connection_drag: Option<ActionEnvelope>,
}

impl From<InfiniteCanvasPortLayer> for Widget {
    fn from(layer: InfiniteCanvasPortLayer) -> Self {
        let mut children = Vec::new();
        for node in ordered_nodes(&layer.nodes) {
            for port in &node.ports {
                let left = node.bounds.x() + port.bounds.x();
                let top = node.bounds.y() + port.bounds.y();
                let content = Container::new(port.child.clone())
                    .width(port.bounds.width().max(0.0))
                    .height(port.bounds.height().max(0.0));
                let child: Widget = if port.enabled {
                    CanvasInteractionRegion {
                        id: port.id.widget_id(layer.canvas_id, node.id),
                        child: content.into(),
                        identifier: format!(
                            "infinite-canvas-port:{:032x}:{:032x}",
                            node.id.0, port.id.0
                        ),
                        target: CanvasTarget {
                            canvas_id: layer.canvas_id.as_u128(),
                            kind: CanvasTargetKind::Port {
                                node_id: node.id.0,
                                port_id: port.id.0,
                            },
                            selection_policy: layer.selection_policy,
                            snap_spacing: layer.snap.enabled.then_some(layer.snap.spacing),
                            snap_threshold: layer.snap.threshold,
                        },
                        on_activate: None,
                        on_drag: layer.on_connection_drag.clone(),
                    }
                    .into()
                } else {
                    content.into()
                };
                children.push(
                    Container::new(child)
                        .positioned(Some(left), Some(top), None, None)
                        .width(port.bounds.width().max(0.0))
                        .height(port.bounds.height().max(0.0))
                        .into(),
                );
            }
        }
        ZStack { id: None, children }.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infinite_canvas::{
        CanvasNodeAnchor, CanvasNodeId, CanvasPortId, InfiniteCanvasPort,
    };
    use fission_core::ui::Spacer;
    use fission_layout::LayoutRect;

    #[test]
    fn port_widget_identity_is_scoped_by_canvas_and_node() {
        let port = InfiniteCanvasPort::new(
            CanvasPortId::from_u128(9),
            LayoutRect::new(0.0, 0.0, 10.0, 10.0),
            CanvasNodeAnchor::Center,
            Spacer::default(),
        );
        let canvas = WidgetId::explicit("canvas");
        assert_ne!(
            port.id.widget_id(canvas, CanvasNodeId::from_u128(1)),
            port.id.widget_id(canvas, CanvasNodeId::from_u128(2))
        );
    }
}
