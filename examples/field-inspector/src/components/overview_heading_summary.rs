use crate::components::ui::{BodyText, StatusPill, TitleScale, TitleText};
use crate::model::{CapabilityState, FieldInspectorState};
use fission::prelude::*;

fission_component! {
    pub OverviewHeadingSummary<FieldInspectorState> {}
    |_summary, state, cx| {
        let order = state.selected_order();
    } =>
        Column(
            gap: cx.view.env().theme.tokens.spacing.xs,
            flex_grow: 1.0,
            align_items: ir_op::AlignItems::Start,
            children: [
                StatusPill(order.priority.to_string(), CapabilityState::Warning),
                TitleText(order.title.to_string(), TitleScale::Page),
                BodyText(order.summary.to_string()),
            ],
        )
}
