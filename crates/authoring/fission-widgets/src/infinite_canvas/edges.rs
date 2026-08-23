use std::collections::{BTreeSet, HashMap};

use fission_core::ui::{Container, Spacer, Text, Widget, ZStack};
use fission_core::{ActionEnvelope, WidgetId};
use fission_ir::{op::Stroke, CanvasTarget, CanvasTargetKind};
use fission_layout::{LayoutPoint, LayoutRect};

use super::geometry::{edge_path, intersects, node_bounds, resolve_endpoint};
use super::{
    interaction_region::CanvasInteractionRegion, CanvasEdgeId, CanvasSelectionPolicy, CanvasSnap,
    CanvasVectorLayer, InfiniteCanvasEdge, InfiniteCanvasNode,
};

#[derive(Debug, Clone)]
pub(crate) struct InfiniteCanvasEdgeLayer {
    pub canvas_id: WidgetId,
    pub edges: Vec<InfiniteCanvasEdge>,
    pub nodes: Vec<InfiniteCanvasNode>,
    pub selected_edges: BTreeSet<CanvasEdgeId>,
    pub visible_world: LayoutRect,
    pub selection_policy: CanvasSelectionPolicy,
    pub snap: CanvasSnap,
    pub on_edge_selection: Option<ActionEnvelope>,
}

impl From<InfiniteCanvasEdgeLayer> for Widget {
    fn from(layer: InfiniteCanvasEdgeLayer) -> Self {
        let bounds = node_bounds(&layer.nodes);
        let mut batches: Vec<(Stroke, String)> = Vec::new();
        let mut labels = Vec::new();
        let mut interactions = Vec::new();
        let origin = layer.visible_world.origin;
        for edge in &layer.edges {
            let Some(edge_bounds) = resolved_edge_bounds(edge, &bounds) else {
                continue;
            };
            if !intersects(edge_bounds, layer.visible_world) {
                continue;
            }
            let Some(path) = edge_path(edge, &bounds, origin) else {
                continue;
            };
            let stroke = selected_stroke(edge, layer.selected_edges.contains(&edge.id));
            if let Some((_, existing)) = batches
                .iter_mut()
                .find(|(candidate, _)| *candidate == stroke)
            {
                existing.push(' ');
                existing.push_str(&path);
            } else {
                batches.push((stroke, path));
            }
            if let Some(action) = &layer.on_edge_selection {
                let points = resolved_edge_points(edge, &bounds);
                if points.len() >= 2 {
                    let interaction_id = WidgetId::derived(
                        layer.canvas_id.as_u128(),
                        &[
                            0xED6E,
                            edge.id.0 as u32,
                            (edge.id.0 >> 32) as u32,
                            (edge.id.0 >> 64) as u32,
                            (edge.id.0 >> 96) as u32,
                        ],
                    );
                    interactions.push(
                        Container::new(CanvasInteractionRegion {
                            id: interaction_id,
                            child: Spacer::default().into(),
                            identifier: format!("infinite-canvas-edge:{:032x}", edge.id.0),
                            target: CanvasTarget {
                                canvas_id: layer.canvas_id.as_u128(),
                                kind: CanvasTargetKind::Edge {
                                    edge_id: edge.id.0,
                                    points: points.iter().map(|point| [point.x, point.y]).collect(),
                                    cubic: matches!(
                                        edge.route,
                                        super::CanvasEdgeRoute::Cubic { .. }
                                    ),
                                    hit_tolerance: (edge.stroke.width * 0.5 + 5.0).max(6.0),
                                },
                                selection_policy: layer.selection_policy,
                                snap_spacing: layer.snap.enabled.then_some(layer.snap.spacing),
                                snap_threshold: layer.snap.threshold,
                            },
                            on_activate: Some(action.clone()),
                            on_drag: None,
                        })
                        .positioned(Some(edge_bounds.x()), Some(edge_bounds.y()), None, None)
                        .width(edge_bounds.width())
                        .height(edge_bounds.height())
                        .into(),
                    );
                }
            }
            if let Some(label) = &edge.label {
                let center = LayoutPoint::new(
                    edge_bounds.x() + edge_bounds.width() * 0.5,
                    edge_bounds.y() + edge_bounds.height() * 0.5,
                );
                labels.push(
                    Container::new(Text::new(label.clone()))
                        .positioned(Some(center.x), Some(center.y), None, None)
                        .into(),
                );
            }
        }

        let mut children = batches
            .into_iter()
            .enumerate()
            .map(|(index, (stroke, path))| {
                Container::new(CanvasVectorLayer {
                    id: WidgetId::derived(layer.canvas_id.as_u128(), &[0xED63, index as u32]),
                    path,
                    width: layer.visible_world.width(),
                    height: layer.visible_world.height(),
                    fill: None,
                    stroke: Some(stroke),
                })
                .positioned(Some(origin.x), Some(origin.y), None, None)
                .width(layer.visible_world.width())
                .height(layer.visible_world.height())
                .into()
            })
            .collect::<Vec<_>>();
        children.extend(labels);
        children.extend(interactions);
        ZStack { id: None, children }.into()
    }
}

fn resolved_edge_points(
    edge: &InfiniteCanvasEdge,
    nodes: &HashMap<super::CanvasNodeId, LayoutRect>,
) -> Vec<LayoutPoint> {
    let (Some(from), Some(to)) = (
        resolve_endpoint(edge.from, nodes),
        resolve_endpoint(edge.to, nodes),
    ) else {
        return Vec::new();
    };
    match edge.route {
        super::CanvasEdgeRoute::Straight => vec![from, to],
        super::CanvasEdgeRoute::Cubic {
            first_control,
            second_control,
        } => vec![from, first_control, second_control, to],
    }
}

fn selected_stroke(edge: &InfiniteCanvasEdge, selected: bool) -> Stroke {
    let mut stroke = edge.stroke.clone();
    if selected {
        stroke.width = (stroke.width * 1.5).max(stroke.width + 1.0);
    }
    stroke
}

fn resolved_edge_bounds(
    edge: &InfiniteCanvasEdge,
    nodes: &HashMap<super::CanvasNodeId, LayoutRect>,
) -> Option<LayoutRect> {
    let from = resolve_endpoint(edge.from, nodes)?;
    let to = resolve_endpoint(edge.to, nodes)?;
    let mut points = vec![from, to];
    if let super::CanvasEdgeRoute::Cubic {
        first_control,
        second_control,
    } = edge.route
    {
        points.push(first_control);
        points.push(second_control);
    }
    let left = points
        .iter()
        .map(|point| point.x)
        .fold(f32::INFINITY, f32::min);
    let top = points
        .iter()
        .map(|point| point.y)
        .fold(f32::INFINITY, f32::min);
    let right = points
        .iter()
        .map(|point| point.x)
        .fold(f32::NEG_INFINITY, f32::max);
    let bottom = points
        .iter()
        .map(|point| point.y)
        .fold(f32::NEG_INFINITY, f32::max);
    Some(LayoutRect::new(
        left,
        top,
        (right - left).max(1.0),
        (bottom - top).max(1.0),
    ))
}
