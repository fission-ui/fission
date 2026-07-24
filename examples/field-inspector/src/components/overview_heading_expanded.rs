use crate::components::overview_heading_summary::OverviewHeadingSummary;
use crate::components::ui::ActionButton;
use crate::model::{on_start_inspection, FieldInspectorState, StartInspection};
use fission::prelude::*;

pub struct OverviewHeadingExpanded;

impl From<OverviewHeadingExpanded> for Widget {
    fn from(_: OverviewHeadingExpanded) -> Self {
        let (ctx, view) = fission::build::current::<FieldInspectorState>();
        let start = with_reducer!(ctx, StartInspection, on_start_inspection);

        Row {
            gap: Some(view.env().theme.tokens.spacing.m),
            align_items: ir_op::AlignItems::Start,
            children: widgets![
                OverviewHeadingSummary,
                ActionButton::new(
                    "field-inspector.action.start-overview",
                    if view.state().started {
                        "Refresh checks"
                    } else {
                        "Start inspection"
                    },
                    start,
                    ButtonVariant::Primary,
                ),
            ],
            ..Default::default()
        }
        .into()
    }
}
