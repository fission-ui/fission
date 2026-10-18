use crate::components::overview_heading_summary::OverviewHeadingSummary;
use crate::components::ui::ActionButton;
use crate::model::{on_start_inspection, FieldInspectorState, StartInspection};
use fission::prelude::*;
fission_component! {
    pub OverviewHeadingExpanded<FieldInspectorState> {}
    |_heading, state, cx| {
        let start = with_reducer!(cx.build, StartInspection, on_start_inspection);
    } =>
        Row(
            gap: cx.view.env().theme.tokens.spacing.m,
            align_items: ir_op::AlignItems::Start,
            children: [
                OverviewHeadingSummary(),
                ActionButton(
                    "field-inspector.action.start-overview".to_string(),
                    if state.started { "Refresh checks" } else { "Start inspection" }.to_string(),
                    start,
                    ButtonVariant::Primary,
                ),
            ],
        )
}
