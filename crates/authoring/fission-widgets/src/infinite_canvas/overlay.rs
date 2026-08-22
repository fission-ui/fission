use std::collections::BTreeSet;

use fission_core::ui::{Container, Widget, ZStack};
use fission_core::{ActionEnvelope, WidgetId};
use fission_ir::{op::Color, CanvasTarget, CanvasTargetKind};
use fission_layout::{LayoutPoint, LayoutRect};

use super::{
    interaction_region::CanvasInteractionRegion, CanvasNodeId, CanvasSelectionPolicy, CanvasSnap,
    InfiniteCanvasNode,
};

const HANDLE_SIZE: f32 = 10.0;

#[derive(Debug, Clone)]
pub(crate) struct InfiniteCanvasOverlay {
    pub canvas_id: WidgetId,
    pub nodes: Vec<InfiniteCanvasNode>,
    pub selection: BTreeSet<CanvasNodeId>,
    pub marquee: Option<LayoutRect>,
    pub translation_x: f32,
    pub translation_y: f32,
    pub scale: f32,
    pub color: Color,
    pub selection_policy: CanvasSelectionPolicy,
    pub snap: CanvasSnap,
    pub on_node_resize: Option<ActionEnvelope>,
}

impl From<InfiniteCanvasOverlay> for Widget {
    fn from(overlay: InfiniteCanvasOverlay) -> Self {
        let mut children = Vec::new();
        for node in overlay
            .nodes
            .iter()
            .filter(|node| node.resizable && overlay.selection.contains(&node.id))
        {
            for (index, point) in handle_points(node.bounds).into_iter().enumerate() {
                let screen = overlay.to_screen(point);
                let handle_id = WidgetId::derived(
                    node.id.widget_id(overlay.canvas_id).as_u128(),
                    &[0xA11D, index as u32],
                );
                let handle = CanvasInteractionRegion {
                    id: handle_id,
                    identifier: format!("infinite-canvas-resize:{:032x}:{index}", node.id.0),
                    child: Container::new(fission_core::ui::Spacer::default())
                        .bg(overlay.color)
                        .border_radius(HANDLE_SIZE * 0.5)
                        .width(HANDLE_SIZE)
                        .height(HANDLE_SIZE)
                        .into(),
                    target: CanvasTarget {
                        canvas_id: overlay.canvas_id.as_u128(),
                        kind: CanvasTargetKind::ResizeHandle {
                            node_id: node.id.0,
                            handle: index as u8,
                            bounds: [
                                node.bounds.x(),
                                node.bounds.y(),
                                node.bounds.width(),
                                node.bounds.height(),
                            ],
                        },
                        selection_policy: overlay.selection_policy,
                        snap_spacing: overlay.snap.enabled.then_some(overlay.snap.spacing),
                        snap_threshold: overlay.snap.threshold,
                    },
                    on_activate: None,
                    on_drag: overlay.on_node_resize.clone(),
                };
                children.push(
                    Container::new(handle)
                        .positioned(
                            Some(screen.x - HANDLE_SIZE * 0.5),
                            Some(screen.y - HANDLE_SIZE * 0.5),
                            None,
                            None,
                        )
                        .width(HANDLE_SIZE)
                        .height(HANDLE_SIZE)
                        .into(),
                );
            }
        }
        if let Some(marquee) = overlay.marquee {
            children.push(
                Container::new(fission_core::ui::Spacer::default())
                    .positioned(Some(marquee.x()), Some(marquee.y()), None, None)
                    .width(marquee.width().max(0.0))
                    .height(marquee.height().max(0.0))
                    .bg(overlay.color.with_alpha(32))
                    .border(overlay.color, 1.0)
                    .into(),
            );
        }
        ZStack { id: None, children }.into()
    }
}

impl InfiniteCanvasOverlay {
    fn to_screen(&self, point: LayoutPoint) -> LayoutPoint {
        LayoutPoint::new(
            point.x * self.scale + self.translation_x,
            point.y * self.scale + self.translation_y,
        )
    }
}

fn handle_points(rect: LayoutRect) -> [LayoutPoint; 8] {
    let center_x = rect.x() + rect.width() * 0.5;
    let center_y = rect.y() + rect.height() * 0.5;
    [
        LayoutPoint::new(rect.x(), rect.y()),
        LayoutPoint::new(center_x, rect.y()),
        LayoutPoint::new(rect.right(), rect.y()),
        LayoutPoint::new(rect.right(), center_y),
        LayoutPoint::new(rect.right(), rect.bottom()),
        LayoutPoint::new(center_x, rect.bottom()),
        LayoutPoint::new(rect.x(), rect.bottom()),
        LayoutPoint::new(rect.x(), center_y),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resize_handles_cover_corners_and_edge_centres() {
        let points = handle_points(LayoutRect::new(-20.0, 10.0, 40.0, 20.0));
        assert_eq!(points[0], LayoutPoint::new(-20.0, 10.0));
        assert_eq!(points[3], LayoutPoint::new(20.0, 20.0));
        assert_eq!(points[5], LayoutPoint::new(0.0, 30.0));
    }
}
