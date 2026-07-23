use crate::components::ui::{ActionButton, Metric};
use crate::model::{
    on_capture_evidence_photo, on_record_voice_note, on_toggle_torch, CaptureEvidencePhoto,
    FieldInspectorState, RecordVoiceNote, ToggleTorch,
};
use fission::prelude::*;

pub struct EvidenceControls;

impl From<EvidenceControls> for Widget {
    fn from(_: EvidenceControls) -> Self {
        let (ctx, view) = fission::build::current::<FieldInspectorState>();
        let capture = with_reducer!(ctx, CaptureEvidencePhoto, on_capture_evidence_photo);
        let torch = with_reducer!(ctx, ToggleTorch, on_toggle_torch);
        let record = with_reducer!(ctx, RecordVoiceNote, on_record_voice_note);
        let spacing = &view.env().theme.tokens.spacing;

        Column {
            gap: Some(spacing.s),
            children: widgets![
                ActionButton::new(
                    "field-inspector.evidence.capture",
                    "Capture photo",
                    capture,
                    ButtonVariant::Primary,
                ),
                ActionButton::new(
                    "field-inspector.evidence.torch",
                    if view.state().torch_on {
                        "Turn torch off"
                    } else {
                        "Turn torch on"
                    },
                    torch,
                    ButtonVariant::SecondaryGray,
                ),
                ActionButton::new(
                    "field-inspector.evidence.voice-note",
                    "Record voice note",
                    record,
                    ButtonVariant::SecondaryColor,
                ),
                Metric::new(
                    "Camera",
                    view.state()
                        .camera_availability
                        .as_ref()
                        .map(|availability| format!("{} device(s)", availability.devices.len()))
                        .unwrap_or_else(|| "Not checked".to_string()),
                ),
                Metric::new(
                    "Microphone",
                    view.state()
                        .microphone_availability
                        .as_ref()
                        .map(|availability| format!("{} input(s)", availability.devices.len()))
                        .unwrap_or_else(|| "Not checked".to_string()),
                ),
            ],
            ..Default::default()
        }
        .into()
    }
}
