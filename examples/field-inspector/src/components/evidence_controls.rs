use crate::components::ui::{ActionButton, Metric};
use crate::model::{
    on_capture_evidence_photo, on_record_voice_note, on_toggle_torch, CaptureEvidencePhoto,
    FieldInspectorState, RecordVoiceNote, ToggleTorch,
};
use fission::prelude::*;
fission_component! {
    pub EvidenceControls<FieldInspectorState> {}
    |_controls, state, cx| {
        let capture = with_reducer!(cx.build, CaptureEvidencePhoto, on_capture_evidence_photo);
        let torch = with_reducer!(cx.build, ToggleTorch, on_toggle_torch);
        let record = with_reducer!(cx.build, RecordVoiceNote, on_record_voice_note);
    } =>
        Column(
            gap: cx.view.env().theme.tokens.spacing.s,
            children: [
                ActionButton("field-inspector.evidence.capture".to_string(), "Capture photo".to_string(), capture, ButtonVariant::Primary),
                ActionButton(
                    "field-inspector.evidence.torch".to_string(),
                    if state.torch_on { "Turn torch off" } else { "Turn torch on" }.to_string(),
                    torch, ButtonVariant::SecondaryGray,
                ),
                ActionButton("field-inspector.evidence.voice-note".to_string(), "Record voice note".to_string(), record, ButtonVariant::SecondaryColor),
                Metric("Camera".to_string(), state.camera_availability.as_ref().map(|value| format!("{} device(s)", value.devices.len())).unwrap_or_else(|| "Not checked".to_string())),
                Metric("Microphone".to_string(), state.microphone_availability.as_ref().map(|value| format!("{} input(s)", value.devices.len())).unwrap_or_else(|| "Not checked".to_string())),
            ],
        )
}
