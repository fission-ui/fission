use std::collections::{BTreeSet, HashMap};

use fission_core::ui::{AnchoredPositioned, Container, Spacer, Widget, ZStack};
use fission_core::{ActionEnvelope, WidgetId};
use fission_ir::{op::Fill, op::Stroke, CanvasTarget, CanvasTargetKind};
use fission_layout::{LayoutPoint, LayoutRect};

use super::geometry::{edge_path, intersects, node_bounds, port_points, resolve_endpoint};
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
        let ports = port_points(&layer.nodes);
        let mut batches: Vec<(Stroke, String)> = Vec::new();
        let mut labels = Vec::new();
        let mut markers = Vec::new();
        let mut interactions = Vec::new();
        let origin = layer.visible_world.origin;
        for edge in &layer.edges {
            let Some(edge_bounds) = resolved_edge_bounds(edge, &bounds, &ports) else {
                continue;
            };
            if !intersects(edge_bounds, layer.visible_world) {
                continue;
            }
            let Some(path) = edge_path(edge, &bounds, &ports, origin) else {
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
            let points = resolved_edge_points(edge, &bounds, &ports);
            if points.len() >= 2 {
                if let Some(marker) = &edge.start_marker {
                    if let Some(path) = distinct_marker_neighbor(&points, true)
                        .and_then(|neighbor| marker_path(marker, points[0], neighbor, origin))
                    {
                        markers.push((
                            marker_fill(marker, &edge.stroke.fill),
                            path,
                            marker_id(layer.canvas_id, edge.id, 0),
                        ));
                    }
                }
                if let Some(marker) = &edge.end_marker {
                    let last = points.len() - 1;
                    if let Some(path) = distinct_marker_neighbor(&points, false)
                        .and_then(|neighbor| marker_path(marker, points[last], neighbor, origin))
                    {
                        markers.push((
                            marker_fill(marker, &edge.stroke.fill),
                            path,
                            marker_id(layer.canvas_id, edge.id, 1),
                        ));
                    }
                }
            }
            if let Some(action) = &layer.on_edge_selection {
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
                                        &edge.route,
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
                if let Some(point) = point_along_route(
                    &points,
                    matches!(&edge.route, super::CanvasEdgeRoute::Cubic { .. }),
                    label.position,
                ) {
                    labels.push(
                        AnchoredPositioned {
                            id: Some(marker_id(layer.canvas_id, edge.id, 2)),
                            x: point.x + label.offset.x,
                            y: point.y + label.offset.y,
                            anchor_x: label.anchor[0],
                            anchor_y: label.anchor[1],
                            child: label.child.clone(),
                        }
                        .into(),
                    );
                }
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
        children.extend(markers.into_iter().map(|(fill, path, id)| {
            Container::new(CanvasVectorLayer {
                id,
                path,
                width: layer.visible_world.width(),
                height: layer.visible_world.height(),
                fill: Some(fill),
                stroke: None,
            })
            .positioned(Some(origin.x), Some(origin.y), None, None)
            .width(layer.visible_world.width())
            .height(layer.visible_world.height())
            .into()
        }));
        children.extend(labels);
        children.extend(interactions);
        ZStack { id: None, children }.into()
    }
}

fn point_along_route(points: &[LayoutPoint], cubic: bool, position: f32) -> Option<LayoutPoint> {
    if points.len() < 2 {
        return None;
    }
    let position = if position.is_finite() {
        position.clamp(0.0, 1.0)
    } else {
        0.5
    };
    if cubic && points.len() >= 4 {
        let inverse = 1.0 - position;
        let weights = [
            inverse * inverse * inverse,
            3.0 * inverse * inverse * position,
            3.0 * inverse * position * position,
            position * position * position,
        ];
        return Some(LayoutPoint::new(
            points
                .iter()
                .zip(weights)
                .map(|(point, weight)| point.x * weight)
                .sum(),
            points
                .iter()
                .zip(weights)
                .map(|(point, weight)| point.y * weight)
                .sum(),
        ));
    }

    let lengths = points
        .windows(2)
        .map(|segment| {
            let dx = segment[1].x - segment[0].x;
            let dy = segment[1].y - segment[0].y;
            (dx * dx + dy * dy).sqrt()
        })
        .collect::<Vec<_>>();
    let total = lengths.iter().copied().sum::<f32>();
    if !total.is_finite() || total <= f32::EPSILON {
        return Some(points[0]);
    }
    let mut remaining = total * position;
    for (segment, length) in points.windows(2).zip(lengths) {
        if remaining <= length || length <= f32::EPSILON {
            let progress = if length <= f32::EPSILON {
                0.0
            } else {
                remaining / length
            };
            return Some(LayoutPoint::new(
                segment[0].x + (segment[1].x - segment[0].x) * progress,
                segment[0].y + (segment[1].y - segment[0].y) * progress,
            ));
        }
        remaining -= length;
    }
    points.last().copied()
}

fn marker_id(canvas_id: WidgetId, edge_id: CanvasEdgeId, side: u32) -> WidgetId {
    WidgetId::derived(
        canvas_id.as_u128(),
        &[
            0xA440,
            side,
            edge_id.0 as u32,
            (edge_id.0 >> 32) as u32,
            (edge_id.0 >> 64) as u32,
            (edge_id.0 >> 96) as u32,
        ],
    )
}

fn marker_fill(marker: &super::CanvasEdgeMarker, fallback: &Fill) -> Fill {
    match marker {
        super::CanvasEdgeMarker::Arrow { fill, .. } => {
            fill.clone().unwrap_or_else(|| fallback.clone())
        }
    }
}

fn distinct_marker_neighbor(points: &[LayoutPoint], start: bool) -> Option<LayoutPoint> {
    let tip = if start {
        *points.first()?
    } else {
        *points.last()?
    };
    let distinct = |point: &&LayoutPoint| {
        let dx = point.x - tip.x;
        let dy = point.y - tip.y;
        dx * dx + dy * dy > f32::EPSILON
    };
    if start {
        points.iter().skip(1).find(distinct).copied()
    } else {
        points.iter().rev().skip(1).find(distinct).copied()
    }
}

fn marker_path(
    marker: &super::CanvasEdgeMarker,
    tip: LayoutPoint,
    previous: LayoutPoint,
    origin: LayoutPoint,
) -> Option<String> {
    let super::CanvasEdgeMarker::Arrow { length, width, .. } = marker;
    if !length.is_finite() || !width.is_finite() || *length <= 0.0 || *width <= 0.0 {
        return None;
    }
    let dx = tip.x - previous.x;
    let dy = tip.y - previous.y;
    let magnitude = (dx * dx + dy * dy).sqrt();
    if !magnitude.is_finite() || magnitude <= f32::EPSILON {
        return None;
    }
    let direction_x = dx / magnitude;
    let direction_y = dy / magnitude;
    let base_x = tip.x - direction_x * *length;
    let base_y = tip.y - direction_y * *length;
    let half_width = *width * 0.5;
    let normal_x = -direction_y * half_width;
    let normal_y = direction_x * half_width;
    Some(format!(
        "M{} {} L{} {} L{} {} Z",
        tip.x - origin.x,
        tip.y - origin.y,
        base_x + normal_x - origin.x,
        base_y + normal_y - origin.y,
        base_x - normal_x - origin.x,
        base_y - normal_y - origin.y,
    ))
}

fn resolved_edge_points(
    edge: &InfiniteCanvasEdge,
    nodes: &HashMap<super::CanvasNodeId, LayoutRect>,
    ports: &HashMap<super::CanvasPortRef, LayoutPoint>,
) -> Vec<LayoutPoint> {
    let (Some(from), Some(to)) = (
        resolve_endpoint(edge.from, nodes, ports),
        resolve_endpoint(edge.to, nodes, ports),
    ) else {
        return Vec::new();
    };
    match &edge.route {
        super::CanvasEdgeRoute::Straight => vec![from, to],
        super::CanvasEdgeRoute::Polyline { points } => {
            let mut resolved = Vec::with_capacity(points.len() + 2);
            resolved.push(from);
            resolved.extend(points.iter().copied());
            resolved.push(to);
            resolved
        }
        super::CanvasEdgeRoute::Cubic {
            first_control,
            second_control,
        } => vec![from, *first_control, *second_control, to],
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
    ports: &HashMap<super::CanvasPortRef, LayoutPoint>,
) -> Option<LayoutRect> {
    let from = resolve_endpoint(edge.from, nodes, ports)?;
    let to = resolve_endpoint(edge.to, nodes, ports)?;
    let mut points = vec![from, to];
    match &edge.route {
        super::CanvasEdgeRoute::Straight => {}
        super::CanvasEdgeRoute::Polyline {
            points: route_points,
        } => points.extend(route_points.iter().copied()),
        super::CanvasEdgeRoute::Cubic {
            first_control,
            second_control,
        } => {
            points.push(*first_control);
            points.push(*second_control);
        }
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
    let marker_padding = [&edge.start_marker, &edge.end_marker]
        .into_iter()
        .flatten()
        .map(|marker| match marker {
            super::CanvasEdgeMarker::Arrow { length, width, .. } => (*length).max(*width * 0.5),
        })
        .fold(0.0_f32, f32::max);
    Some(LayoutRect::new(
        left - marker_padding,
        top - marker_padding,
        (right - left).max(1.0) + marker_padding * 2.0,
        (bottom - top).max(1.0) + marker_padding * 2.0,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrow_marker_follows_the_endpoint_tangent() {
        let marker = super::super::CanvasEdgeMarker::Arrow {
            length: 10.0,
            width: 8.0,
            fill: None,
        };
        assert_eq!(
            marker_path(
                &marker,
                LayoutPoint::new(30.0, 20.0),
                LayoutPoint::new(10.0, 20.0),
                LayoutPoint::new(5.0, 5.0),
            )
            .as_deref(),
            Some("M25 15 L15 19 L15 11 Z")
        );
        assert_eq!(
            distinct_marker_neighbor(
                &[
                    LayoutPoint::new(30.0, 20.0),
                    LayoutPoint::new(30.0, 20.0),
                    LayoutPoint::new(10.0, 20.0),
                ],
                true,
            ),
            Some(LayoutPoint::new(10.0, 20.0))
        );
    }

    #[test]
    fn retained_label_position_follows_polyline_distance() {
        let points = [
            LayoutPoint::new(0.0, 0.0),
            LayoutPoint::new(20.0, 0.0),
            LayoutPoint::new(20.0, 80.0),
        ];
        assert_eq!(
            point_along_route(&points, false, 0.5),
            Some(LayoutPoint::new(20.0, 30.0))
        );
    }
}
