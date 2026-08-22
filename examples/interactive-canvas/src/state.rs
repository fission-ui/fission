use std::collections::BTreeSet;

use fission::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct CanvasNodeRecord {
    pub id: CanvasNodeId,
    pub title: String,
    pub detail: String,
    pub bounds: LayoutRect,
    pub z_index: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CanvasExampleState {
    pub(crate) nodes: Vec<CanvasNodeRecord>,
    pub(crate) selected_nodes: BTreeSet<CanvasNodeId>,
    pub(crate) selected_edges: BTreeSet<CanvasEdgeId>,
    pub(crate) status: String,
}

impl Default for CanvasExampleState {
    fn default() -> Self {
        Self {
            nodes: vec![
                CanvasNodeRecord {
                    id: CanvasNodeId::explicit("source"),
                    title: "Source".into(),
                    detail: "Capture events".into(),
                    bounds: LayoutRect::new(-220.0, -60.0, 168.0, 88.0),
                    z_index: 0,
                },
                CanvasNodeRecord {
                    id: CanvasNodeId::explicit("transform"),
                    title: "Transform".into(),
                    detail: "Normalize data".into(),
                    bounds: LayoutRect::new(40.0, 70.0, 176.0, 88.0),
                    z_index: 1,
                },
                CanvasNodeRecord {
                    id: CanvasNodeId::explicit("output"),
                    title: "Output".into(),
                    detail: "Publish result".into(),
                    bounds: LayoutRect::new(300.0, -90.0, 168.0, 88.0),
                    z_index: 0,
                },
            ],
            selected_nodes: BTreeSet::new(),
            selected_edges: BTreeSet::new(),
            status: "Drag, resize, select, pan, or zoom".into(),
        }
    }
}

impl GlobalState for CanvasExampleState {}

#[fission_reducer(EditCanvas)]
pub(crate) fn edit_canvas(
    state: &mut CanvasExampleState,
    document: String,
    ctx: &mut ReducerContext<CanvasExampleState>,
) {
    let Some(interaction) = ctx.input.canvas_interaction().cloned() else {
        return;
    };
    state.status = format!("{document}: {:?} {:?}", interaction.kind, interaction.phase);

    match interaction.kind {
        CanvasInteractionKind::SelectNode { node_id }
            if interaction.phase == CanvasInteractionPhase::Activate =>
        {
            let node_id = CanvasNodeId::from_u128(node_id);
            if interaction.selection_policy == CanvasSelectionPolicy::Toggle {
                if !state.selected_nodes.remove(&node_id) {
                    state.selected_nodes.insert(node_id);
                }
            } else {
                state.selected_nodes.clear();
                state.selected_nodes.insert(node_id);
                state.selected_edges.clear();
            }
        }
        CanvasInteractionKind::MoveNode { node_id }
            if interaction.phase == CanvasInteractionPhase::Update =>
        {
            update_node_bounds(state, node_id, interaction.bounds_after);
        }
        CanvasInteractionKind::ResizeNode { node_id, .. }
            if interaction.phase == CanvasInteractionPhase::Update =>
        {
            update_node_bounds(state, node_id, interaction.bounds_after);
        }
        CanvasInteractionKind::SelectEdge { edge_id }
            if interaction.phase == CanvasInteractionPhase::Activate =>
        {
            state.selected_nodes.clear();
            state.selected_edges.clear();
            state
                .selected_edges
                .insert(CanvasEdgeId::from_u128(edge_id));
        }
        CanvasInteractionKind::Marquee
            if matches!(
                interaction.phase,
                CanvasInteractionPhase::Update | CanvasInteractionPhase::End
            ) =>
        {
            if let Some(marquee) = interaction.marquee {
                state.selected_nodes = state
                    .nodes
                    .iter()
                    .filter(|node| rectangles_intersect(node.bounds, marquee))
                    .map(|node| node.id)
                    .collect();
                state.selected_edges.clear();
            }
        }
        _ => {}
    }
}

#[fission_reducer(TrackViewport)]
pub(crate) fn track_viewport(
    state: &mut CanvasExampleState,
    surface: String,
    ctx: &mut ReducerContext<CanvasExampleState>,
) {
    let Some(interaction) = ctx.input.viewport_interaction() else {
        return;
    };
    state.status = format!(
        "{surface}: {:?} · {:.2}× · ({:.0}, {:.0})",
        interaction.phase,
        interaction.transform.scale,
        interaction.transform.translation[0],
        interaction.transform.translation[1]
    );
}

fn update_node_bounds(state: &mut CanvasExampleState, node_id: u128, bounds: Option<LayoutRect>) {
    let Some(bounds) = bounds else { return };
    if let Some(node) = state
        .nodes
        .iter_mut()
        .find(|node| node.id == CanvasNodeId::from_u128(node_id))
    {
        node.bounds = bounds;
    }
}

fn rectangles_intersect(first: LayoutRect, second: LayoutRect) -> bool {
    first.x() < second.right()
        && first.right() > second.x()
        && first.y() < second.bottom()
        && first.bottom() > second.y()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rectangle_intersection_handles_negative_world_coordinates() {
        let node = LayoutRect::new(-100.0, -50.0, 80.0, 40.0);
        assert!(rectangles_intersect(
            node,
            LayoutRect::new(-40.0, -30.0, 60.0, 60.0)
        ));
        assert!(!rectangles_intersect(
            node,
            LayoutRect::new(10.0, 10.0, 20.0, 20.0)
        ));
    }
}
