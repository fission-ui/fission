use crate::components::activity_log_row::ActivityLogRow;
use crate::components::ui::{BodyText, PanelCard, TitleScale, TitleText};
use crate::model::FieldInspectorState;
use fission::prelude::*;
fission_component! {
    pub ActivityLog<FieldInspectorState> {}
    |_log, state, cx| {} =>
        PanelCard(
            Column(
                gap: cx.view.env().theme.tokens.spacing.s,
                children: [
                    TitleText("Capability activity".to_string(), TitleScale::Section),
                    Column(gap: cx.view.env().theme.tokens.spacing.s, children: [
                        if state.logs.is_empty() {
                            BodyText("Run the inspection to see capability requests and host results here.".to_string())
                        }
                        for (index, log) in state.logs.iter().cloned().enumerate() {
                            ActivityLogRow(log, key: index)
                        }
                    ]),
                ],
            ),
        )
}
