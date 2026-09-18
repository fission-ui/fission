use crate::canvas_node_card::CanvasNodeCard;
use crate::state::CanvasExampleState;
use fission::op::{Fill, LineCap, LineJoin, Stroke};
use fission::prelude::*;

/// Ports are small circles centred on the middle of a node's left and right edges.
const PORT_SIZE: f32 = 12.0;
const PORT_BORDER_WIDTH: f32 = 2.0;
const EDGE_WIDTH: f32 = 2.0;
const ARROW_LENGTH: f32 = 10.0;
const ARROW_WIDTH: f32 = 8.0;
/// Node moves snap to the same 16-point grid the canvas draws.
const GRID_SPACING: f32 = 16.0;
const GRID_LINE_WIDTH: f32 = 1.0;
const SNAP_THRESHOLD: f32 = 5.0;
/// The starting pan and zoom frame the three example nodes.
const INITIAL_PAN: (f32, f32) = (250.0, 210.0);
const INITIAL_SCALE: f32 = 0.9;
const MIN_SCALE: f32 = 0.3;
const MAX_SCALE: f32 = 3.5;

pub(crate) struct CanvasPanel {
    pub instance: &'static str,
    pub height: f32,
    pub edit_action: ActionEnvelope,
    pub camera_action: ActionEnvelope,
}

impl From<CanvasPanel> for Widget {
    fn from(panel: CanvasPanel) -> Self {
        let (_, view) = fission::build::current::<CanvasExampleState>();
        let tokens = &view.env().theme.tokens;
        Column {
            flex_grow: 1.0,
            gap: Some(tokens.spacing.s),
            children: widgets![
                Text::new("InfiniteCanvas")
                    .size(tokens.typography.heading2_size)
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.text_primary),
                Text::new("Declarative nodes, ports, routed edges, group moves, and snapping")
                    .size(tokens.typography.font_size_sm)
                    .color(tokens.colors.text_secondary),
                Container::new(NodeGraphCanvas {
                    instance: panel.instance,
                    edit_action: panel.edit_action,
                    camera_action: panel.camera_action,
                })
                .height(panel.height)
                .flex_grow(1.0)
                .bg(tokens.colors.surface)
                .border(tokens.colors.border, 1.0)
                .border_radius(tokens.radii.xl),
            ],
            ..Default::default()
        }
        .into()
    }
}

/// The source → transform → output graph, with selection, moves, resizes and
/// new connections reported through `edit_action`, and pan and zoom through
/// `camera_action`.
struct NodeGraphCanvas {
    instance: &'static str,
    edit_action: ActionEnvelope,
    camera_action: ActionEnvelope,
}

impl From<NodeGraphCanvas> for Widget {
    fn from(canvas: NodeGraphCanvas) -> Self {
        let (_, view) = fission::build::current::<CanvasExampleState>();
        let state = view.state();
        let tokens = &view.env().theme.tokens;
        let source = CanvasNodeId::explicit("source");
        let transform = CanvasNodeId::explicit("transform");
        let output = CanvasNodeId::explicit("output");
        let input_port = CanvasPortId::explicit("input");
        let output_port = CanvasPortId::explicit("output");
        let half_port = PORT_SIZE / 2.0;

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
                let port_top = node.bounds.height() * 0.5 - half_port;
                if node.id != source {
                    canvas_node.ports.push(InfiniteCanvasPort::new(
                        input_port,
                        LayoutRect::new(-half_port, port_top, PORT_SIZE, PORT_SIZE),
                        CanvasNodeAnchor::Center,
                        Container::new(Spacer::default())
                            .bg(tokens.colors.surface)
                            .border(tokens.colors.primary, PORT_BORDER_WIDTH)
                            .border_radius(half_port),
                    ));
                }
                if node.id != output {
                    canvas_node.ports.push(InfiniteCanvasPort::new(
                        output_port,
                        LayoutRect::new(
                            node.bounds.width() - half_port,
                            port_top,
                            PORT_SIZE,
                            PORT_SIZE,
                        ),
                        CanvasNodeAnchor::Center,
                        Container::new(Spacer::default())
                            .bg(tokens.colors.primary)
                            .border_radius(half_port),
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
                width: EDGE_WIDTH,
                dash_array: None,
                dash_offset: 0.0,
                trim: None,
                line_cap: LineCap::Round,
                line_join: LineJoin::Round,
            },
            start_marker: None,
            end_marker: Some(CanvasEdgeMarker::Arrow {
                length: ARROW_LENGTH,
                width: ARROW_WIDTH,
                fill: None,
            }),
            label: Some(CanvasEdgeLabel::new(
                Text::new(label)
                    .size(tokens.typography.font_size_sm)
                    .color(tokens.colors.text_secondary),
            )),
        };

        InfiniteCanvas {
            id: Some(WidgetId::explicit(&format!(
                "infinite-canvas.{}",
                canvas.instance
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
                spacing: GRID_SPACING,
                threshold: SNAP_THRESHOLD,
            },
            grid: Some(CanvasGrid {
                major_every: 4,
                major_color: Some(tokens.colors.text_secondary),
                ..CanvasGrid::lines(GRID_SPACING, tokens.colors.border, GRID_LINE_WIDTH)
            }),
            initial_transform: ViewportTransform::new(INITIAL_PAN.0, INITIAL_PAN.1, INITIAL_SCALE),
            min_scale: MIN_SCALE,
            max_scale: MAX_SCALE,
            actions: InfiniteCanvasActions {
                on_selection_change: Some(canvas.edit_action.clone()),
                on_node_move: Some(canvas.edit_action.clone()),
                on_node_resize: Some(canvas.edit_action.clone()),
                on_edge_selection: Some(canvas.edit_action.clone()),
                on_connection_drag: Some(canvas.edit_action),
                on_interaction_start: Some(canvas.camera_action.clone()),
                on_interaction_update: Some(canvas.camera_action.clone()),
                on_interaction_end: Some(canvas.camera_action),
            },
            ..Default::default()
        }
        .into()
    }
}
