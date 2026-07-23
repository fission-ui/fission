use crate::components::ui::{BodyText, StatusPill, TitleScale, TitleText};
use crate::model::{CapabilityState, FieldInspectorState};
use fission::prelude::*;

pub struct OverviewHeadingSummary;

impl From<OverviewHeadingSummary> for Widget {
    fn from(_: OverviewHeadingSummary) -> Self {
        let (_, view) = fission::build::current::<FieldInspectorState>();
        let order = view.state().selected_order();
        let spacing = &view.env().theme.tokens.spacing;

        Column {
            gap: Some(spacing.xs),
            flex_grow: 1.0,
            align_items: ir_op::AlignItems::Start,
            children: widgets![
                StatusPill::new(order.priority, CapabilityState::Warning),
                TitleText::new(order.title, TitleScale::Page),
                BodyText::new(order.summary),
            ],
            ..Default::default()
        }
        .into()
    }
}
