use crate::components::ui::{BodyText, SoftPanel};
use crate::model::FieldInspectorState;
use fission::prelude::*;
fission_component! {
    pub ProtectedNotes<FieldInspectorState> {}
    |_notes, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let typography = &tokens.typography;
    } =>
        if state.sensitive_unlocked {
            SoftPanel(
                Column(
                    gap: tokens.spacing.s,
                    children: [
                        Text("Site access note").size(typography.body_large_size).weight(typography.font_weight_bold),
                        BodyText("Door code expires after this shift. Escalate compressor pressure above 14 bar to the site manager before leaving.".to_string()),
                    ],
                ),
            )
        } else {
            SoftPanel(BodyText("Protected notes stay hidden until the host verifies the technician.".to_string()))
        }
}
