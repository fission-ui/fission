use crate::canvas_node_card::CanvasNodeCard;
use crate::state::CanvasExampleState;
use fission::op::{Fill, LineCap, LineJoin, Stroke};
use fission::prelude::*;

pub(crate) struct CanvasPanel {
    pub instance: &'static str,
    pub height: f32,
    pub edit_action: ActionEnvelope,
    pub camera_action: ActionEnvelope,
}

impl From<CanvasPanel> for Widget {
    fn from(panel: CanvasPanel) -> Self {
        let (_, view) = fission::build::current::<CanvasExampleState>();
        let state = view.state();
        let tokens = &view.env().theme.tokens;
        let source = CanvasNodeId::explicit("source");
        let transform = CanvasNodeId::explicit("transform");
        let output = CanvasNodeId::explicit("output");
        let input_port = CanvasPortId::explicit("input");
        let output_port = CanvasPortId::explicit("output");
        let nodes = state
            .nodes
            .iter()
            .map(|node| {
                let mut canvas_node = InfiniteCanvasNode::new(
                    node.id,
                    node.bounds,
                    CanvasNodeCard {
                        title: node.title.clone(),
                        detail: node.detail.clone(),
                    },
                );
                canvas_node.z_index = node.z_index;
                if node.id != source {
                    canvas_node.ports.push(InfiniteCanvasPort::new(
                        input_port,
                        LayoutRect::new(-6.0, node.bounds.height() * 0.5 - 6.0, 12.0, 12.0),
                        CanvasNodeAnchor::Center,
                        Container::new(Spacer::default())
                            .bg(tokens.colors.surface)
                            .border(tokens.colors.primary, 2.0)
                            .border_radius(6.0),
                    ));
                }
                if node.id != output {
                    canvas_node.ports.push(InfiniteCanvasPort::new(
                        output_port,
                        LayoutRect::new(
                            node.bounds.width() - 6.0,
                            node.bounds.height() * 0.5 - 6.0,
                            12.0,
                            12.0,
                        ),
                        CanvasNodeAnchor::Center,
                        Container::new(Spacer::default())
                            .bg(tokens.colors.primary)
                            .border_radius(6.0),
                    ));
                }
                canvas_node
            })
            .collect();
        let edge = |id: &str, from, to, label: &str| InfiniteCanvasEdge {
            id: CanvasEdgeId::explicit(id),
            from: CanvasEdgeEndpoint::Port(CanvasPortRef::new(from, output_port)),
            to: CanvasEdgeEndpoint::Port(CanvasPortRef::new(to, input_port)),
            route: CanvasEdgeRoute::Straight,
            stroke: Stroke {
                fill: Fill::Solid(tokens.colors.primary),
                width: 2.0,
                dash_array: None,
                line_cap: LineCap::Round,
                line_join: LineJoin::Round,
            },
            start_marker: None,
            end_marker: Some(CanvasEdgeMarker::Arrow {
                length: 10.0,
                width: 8.0,
                fill: None,
            }),
            label: Some(CanvasEdgeLabel::new(
                Text::new(label)
                    .size(tokens.typography.font_size_sm)
                    .color(tokens.colors.text_secondary),
            )),
        };
        let canvas: Widget = InfiniteCanvas {
            id: Some(WidgetId::explicit(&format!(
                "infinite-canvas.{}",
                panel.instance
            ))),
            nodes,
            edges: vec![
                edge("source-transform", source, transform, "events"),
                edge("transform-output", transform, output, "records"),
            ],
            selected_nodes: state.selected_nodes.clone(),
            selected_edges: state.selected_edges.clone(),
            selection_policy: CanvasSelectionPolicy::Marquee,
            snap: CanvasSnap {
                enabled: true,
                spacing: 16.0,
                threshold: 5.0,
            },
            grid: Some(CanvasGrid {
                major_every: 4,
                major_color: Some(tokens.colors.text_secondary),
                ..CanvasGrid::lines(16.0, tokens.colors.border, 1.0)
            }),
            initial_transform: ViewportTransform::new(250.0, 210.0, 0.9),
            min_scale: 0.3,
            max_scale: 3.5,
            actions: InfiniteCanvasActions {
                on_selection_change: Some(panel.edit_action.clone()),
                on_node_move: Some(panel.edit_action.clone()),
                on_node_resize: Some(panel.edit_action.clone()),
                on_edge_selection: Some(panel.edit_action.clone()),
                on_connection_drag: Some(panel.edit_action),
                on_interaction_start: Some(panel.camera_action.clone()),
                on_interaction_update: Some(panel.camera_action.clone()),
                on_interaction_end: Some(panel.camera_action),
            },
            ..Default::default()
        }
        .into();

        Column {
            flex_grow: 1.0,
            gap: Some(tokens.spacing.s),
            children: widgets![
                Text::new("InfiniteCanvas")
                    .size(tokens.typography.heading2_size)
                    .weight(700)
                    .color(tokens.colors.text_primary),
                Text::new("Declarative nodes, ports, routed edges, group moves, and snapping")
                    .size(tokens.typography.font_size_sm)
                    .color(tokens.colors.text_secondary),
                Container::new(canvas)
                    .height(panel.height)
                    .flex_grow(1.0)
                    .bg(tokens.colors.surface)
                    .border(tokens.colors.border, 1.0)
                    .border_radius(20.0),
            ],
            ..Default::default()
        }
        .into()
    }
}
