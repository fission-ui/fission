use crate::components::overview::OverviewPanel;
use crate::components::panels::{
    EvidencePanel, ReviewPanel, SecurityPanel, SensorsPanel, VerifyPanel,
};
use crate::components::ui::{
    action_button, color, is_compact, muted_text, page_padding, panel_card, small_button,
    status_pill, title_text,
};
use crate::components::work_orders::WorkOrderRail;
use crate::model::{
    on_select_panel, on_start_inspection, CapabilityState, FieldInspectorState, InspectorPanel,
    SelectPanel, StartInspection,
};
use fission::prelude::*;
use std::sync::Arc;

pub struct FieldInspectorApp;

impl Widget<FieldInspectorState> for FieldInspectorApp {
    fn build(
        &self,
        ctx: &mut BuildCtx<FieldInspectorState>,
        view: &View<FieldInspectorState>,
    ) -> Node {
        let viewport = view.viewport_size();
        let wide = viewport.width >= 1100.0;
        let padding = page_padding(view);
        let content = if wide {
            Row {
                gap: Some(18.0),
                align_items: ir_op::AlignItems::Stretch,
                children: vec![
                    Container::new(WorkOrderRail.build(ctx, view))
                        .width(330.0)
                        .into_node(),
                    Container::new(main_column(ctx, view))
                        .flex_grow(1.0)
                        .into_node(),
                ],
                ..Default::default()
            }
            .into_node()
        } else {
            Column {
                gap: Some(18.0),
                children: vec![main_column(ctx, view), WorkOrderRail.build(ctx, view)],
                ..Default::default()
            }
            .into_node()
        };

        Container::new(
            Scroll {
                child: Some(Box::new(content)),
                direction: FlexDirection::Column,
                show_scrollbar: true,
                flex_grow: 1.0,
                ..Default::default()
            }
            .into_node(),
        )
        .height(viewport.height.max(1.0))
        .padding_all(padding)
        .bg_fill(Fill::LinearGradient {
            start: (0.0, 0.0),
            end: (1.0, 1.0),
            stops: vec![
                (0.0, color(239, 246, 255)),
                (0.45, color(248, 250, 252)),
                (1.0, color(236, 253, 245)),
            ],
        })
        .into_node()
    }
}

fn main_column(ctx: &mut BuildCtx<FieldInspectorState>, view: &View<FieldInspectorState>) -> Node {
    Column {
        gap: Some(18.0),
        children: vec![
            hero(ctx, view),
            panel_tabs(ctx, view),
            active_panel(ctx, view),
        ],
        ..Default::default()
    }
    .into_node()
}

fn hero(ctx: &mut BuildCtx<FieldInspectorState>, view: &View<FieldInspectorState>) -> Node {
    let order = view.state.selected_order();
    let start = with_reducer!(ctx, StartInspection, on_start_inspection);
    let review = with_reducer!(ctx, SelectPanel(InspectorPanel::Review), on_select_panel);
    let (complete, total) = view.state.checklist_progress();
    let summary = Column {
        gap: Some(8.0),
        flex_grow: 1.0,
        children: vec![
            Row {
                gap: Some(8.0),
                wrap: ir_op::FlexWrap::Wrap,
                children: vec![
                    status_pill(view, "Field Inspector", CapabilityState::Ready),
                    status_pill(
                        view,
                        view.state.provider_mode.label(),
                        view.state.provider_mode.state(),
                    ),
                ],
                ..Default::default()
            }
            .into_node(),
            title_text(
                view,
                "Capability-driven field service",
                if is_compact(view) { 25.0 } else { 34.0 },
            ),
            muted_text(
                view,
                format!(
                    "{} - {} - assigned to {}",
                    order.id, order.site, order.assigned_to
                ),
            ),
            muted_text(view, view.state.provider_mode.detail()),
        ],
        ..Default::default()
    }
    .into_node();
    let actions = Column {
        gap: Some(10.0),
        children: {
            let mut children = vec![status_pill(
                view,
                format!("Checklist {complete}/{total}"),
                if complete == total {
                    CapabilityState::Complete
                } else {
                    CapabilityState::Pending
                },
            )];
            if is_compact(view) {
                children.push(action_button(
                    if view.state.started {
                        "Refresh checks"
                    } else {
                        "Start inspection"
                    },
                    start,
                    ButtonVariant::Primary,
                ));
            }
            children.push(action_button(
                "Review report",
                review,
                ButtonVariant::SecondaryColor,
            ));
            children
        },
        align_items: if is_compact(view) {
            ir_op::AlignItems::Start
        } else {
            ir_op::AlignItems::End
        },
        ..Default::default()
    }
    .into_node();
    let child = if is_compact(view) {
        Column {
            gap: Some(16.0),
            children: vec![summary, actions],
            ..Default::default()
        }
        .into_node()
    } else {
        Row {
            gap: Some(18.0),
            align_items: ir_op::AlignItems::Start,
            children: vec![summary, actions],
            ..Default::default()
        }
        .into_node()
    };
    panel_card(view, child)
}

fn panel_tabs(ctx: &mut BuildCtx<FieldInspectorState>, view: &View<FieldInspectorState>) -> Node {
    let panels = [
        InspectorPanel::Overview,
        InspectorPanel::Verify,
        InspectorPanel::Evidence,
        InspectorPanel::Sensors,
        InspectorPanel::Security,
        InspectorPanel::Review,
    ];
    let selected_index = panels
        .iter()
        .position(|panel| *panel == view.state.panel)
        .unwrap_or(0);
    let actions = Arc::new(
        panels
            .iter()
            .map(|panel| with_reducer!(ctx, SelectPanel(*panel), on_select_panel))
            .collect::<Vec<_>>(),
    );
    if is_compact(view) {
        let tab_width = if view.viewport_size().width < 360.0 {
            84.0
        } else {
            96.0
        };
        let children = panels
            .iter()
            .enumerate()
            .map(|(index, panel)| {
                Container::new(small_button(
                    panel.label(),
                    actions[index].clone(),
                    if index == selected_index {
                        ButtonVariant::Filled
                    } else {
                        ButtonVariant::Ghost
                    },
                ))
                .width(tab_width)
                .into_node()
            })
            .collect();
        return panel_card(
            view,
            Row {
                gap: Some(8.0),
                wrap: ir_op::FlexWrap::Wrap,
                children,
                ..Default::default()
            }
            .into_node(),
        );
    }

    panel_card(
        view,
        SegmentedControl {
            options: panels
                .iter()
                .map(|panel| panel.label().to_string())
                .collect(),
            selected_index,
            on_change: Some(Arc::new(move |index| actions[index].clone())),
        }
        .build(ctx, view),
    )
}

fn active_panel(ctx: &mut BuildCtx<FieldInspectorState>, view: &View<FieldInspectorState>) -> Node {
    match view.state.panel {
        InspectorPanel::Overview => OverviewPanel.build(ctx, view),
        InspectorPanel::Verify => VerifyPanel.build(ctx, view),
        InspectorPanel::Evidence => EvidencePanel.build(ctx, view),
        InspectorPanel::Sensors => SensorsPanel.build(ctx, view),
        InspectorPanel::Security => SecurityPanel.build(ctx, view),
        InspectorPanel::Review => ReviewPanel.build(ctx, view),
    }
}
