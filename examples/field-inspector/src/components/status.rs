use crate::components::ui::{
    body_text, is_compact, muted_text, panel_card, status_pill, title_text,
};
use crate::model::{CapabilityLine, CapabilityState, FieldInspectorState};
use fission::prelude::*;

pub struct CapabilityOverview;

impl Widget<FieldInspectorState> for CapabilityOverview {
    fn build(
        &self,
        _ctx: &mut BuildCtx<FieldInspectorState>,
        view: &View<FieldInspectorState>,
    ) -> Node {
        let lines = view.state.capability_lines();
        let complete = lines
            .iter()
            .filter(|line| {
                matches!(
                    line.state,
                    CapabilityState::Complete | CapabilityState::Ready
                )
            })
            .count();
        panel_card(
            view,
            Column {
                gap: Some(14.0),
                children: vec![
                    Row {
                        gap: Some(12.0),
                        children: vec![
                            Column {
                                gap: Some(4.0),
                                flex_grow: 1.0,
                                children: vec![
                                    title_text(view, "Capability readiness", 22.0),
                                    muted_text(
                                        view,
                                        format!(
                                            "{} of {} host surfaces have responded",
                                            complete,
                                            lines.len()
                                        ),
                                    ),
                                ],
                                ..Default::default()
                            }
                            .into_node(),
                            status_pill(
                                view,
                                if view.state.started {
                                    "Live"
                                } else {
                                    "Not started"
                                },
                                if view.state.started {
                                    CapabilityState::Ready
                                } else {
                                    CapabilityState::Idle
                                },
                            ),
                        ],
                        ..Default::default()
                    }
                    .into_node(),
                    capability_grid(view, lines),
                ],
                ..Default::default()
            }
            .into_node(),
        )
    }
}

fn capability_grid(view: &View<FieldInspectorState>, lines: Vec<CapabilityLine>) -> Node {
    let columns = if is_compact(view) { 1 } else { 2 };
    let cells = lines
        .into_iter()
        .enumerate()
        .map(|(index, line)| {
            GridItem::new(capability_cell(view, line))
                .cell((index / columns + 1) as i16, (index % columns + 1) as i16)
                .into_node()
        })
        .collect();
    Grid {
        columns: (0..columns).map(|_| ir_op::GridTrack::Fr(1.0)).collect(),
        rows: Vec::new(),
        column_gap: Some(10.0),
        row_gap: Some(10.0),
        children: cells,
        ..Default::default()
    }
    .into_node()
}

fn capability_cell(view: &View<FieldInspectorState>, line: CapabilityLine) -> Node {
    let tokens = &view.env.theme.tokens;
    Container::new(
        Column {
            gap: Some(8.0),
            children: vec![
                Row {
                    gap: Some(8.0),
                    children: vec![
                        Text::new(line.title)
                            .size(14.0)
                            .line_height(20.0)
                            .weight(800)
                            .color(tokens.colors.text_primary)
                            .into_node(),
                        Spacer {
                            flex_grow: 1.0,
                            ..Default::default()
                        }
                        .into_node(),
                        status_pill(view, line.state.label(), line.state),
                    ],
                    ..Default::default()
                }
                .into_node(),
                body_text(view, line.detail),
            ],
            ..Default::default()
        }
        .into_node(),
    )
    .bg(tokens.colors.background.with_alpha(150))
    .border(tokens.colors.border.with_alpha(110), 1.0)
    .border_radius(16.0)
    .padding_all(12.0)
    .min_height(98.0)
    .into_node()
}

pub struct ActivityLog;

impl Widget<FieldInspectorState> for ActivityLog {
    fn build(
        &self,
        _ctx: &mut BuildCtx<FieldInspectorState>,
        view: &View<FieldInspectorState>,
    ) -> Node {
        let rows: Vec<Node> = if view.state.logs.is_empty() {
            vec![body_text(
                view,
                "Run the inspection to see capability requests and host results here.",
            )]
        } else {
            view.state
                .logs
                .iter()
                .map(|log| {
                    Row {
                        gap: Some(10.0),
                        align_items: ir_op::AlignItems::Start,
                        children: vec![
                            status_pill(view, log.state.label(), log.state),
                            Column {
                                gap: Some(3.0),
                                flex_grow: 1.0,
                                children: vec![
                                    Text::new(log.title.clone())
                                        .size(14.0)
                                        .weight(800)
                                        .into_node(),
                                    muted_text(view, log.detail.clone()),
                                ],
                                ..Default::default()
                            }
                            .into_node(),
                        ],
                        ..Default::default()
                    }
                    .into_node()
                })
                .collect()
        };

        panel_card(
            view,
            Column {
                gap: Some(12.0),
                children: vec![
                    title_text(view, "Capability activity", 20.0),
                    Column {
                        gap: Some(10.0),
                        children: rows,
                        ..Default::default()
                    }
                    .into_node(),
                ],
                ..Default::default()
            }
            .into_node(),
        )
    }
}
