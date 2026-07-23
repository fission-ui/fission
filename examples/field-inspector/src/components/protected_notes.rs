use crate::components::ui::{BodyText, SoftPanel};
use crate::model::FieldInspectorState;
use fission::prelude::*;

pub struct ProtectedNotes;

impl From<ProtectedNotes> for Widget {
    fn from(_: ProtectedNotes) -> Self {
        let (_, view) = fission::build::current::<FieldInspectorState>();
        let tokens = &view.env().theme.tokens;
        let typography = &tokens.typography;

        if view.state().sensitive_unlocked {
            SoftPanel::new(Column {
                gap: Some(tokens.spacing.s),
                children: widgets![
                    Text::new("Site access note")
                        .size(typography.body_large_size)
                        .weight(typography.font_weight_bold),
                    BodyText::new(
                        "Door code expires after this shift. Escalate compressor pressure above 14 bar to the site manager before leaving.",
                    ),
                ],
                ..Default::default()
            })
            .into()
        } else {
            SoftPanel::new(BodyText::new(
                "Protected notes stay hidden until the host verifies the technician.",
            ))
            .into()
        }
    }
}
