use crate::components::activity_log_row::ActivityLogRow;
use crate::components::ui::{BodyText, PanelCard, TitleScale, TitleText};
use crate::model::FieldInspectorState;
use fission::prelude::*;

pub struct ActivityLog;

impl From<ActivityLog> for Widget {
    fn from(_component: ActivityLog) -> Self {
        let (_ctx, view) = fission::build::current::<FieldInspectorState>();
        let rows: Vec<Widget> = if view.state().logs.is_empty() {
            widgets![BodyText::new(
                "Run the inspection to see capability requests and host results here."
            )]
        } else {
            view.state()
                .logs
                .iter()
                .cloned()
                .map(|log| ActivityLogRow { log }.into())
                .collect()
        };

        PanelCard::new(Column {
            gap: Some(view.env().theme.tokens.spacing.s),
            children: widgets![
                TitleText::new("Capability activity", TitleScale::Section),
                Column {
                    gap: Some(view.env().theme.tokens.spacing.s),
                    children: rows,
                    ..Default::default()
                },
            ],
            ..Default::default()
        })
        .into()
    }
}
