//! Reducers that verify the asset and capture photo and voice evidence.

use super::*;

#[fission_reducer(VerifyWithBarcode)]
pub fn on_verify_with_barcode(
    state: &mut FieldInspectorState,
    ctx: &mut ReducerContext<FieldInspectorState>,
) {
    state.panel = InspectorPanel::Verify;
    state.log(
        "Barcode scan",
        "Opening scanner for the selected asset",
        CapabilityState::Pending,
    );
    let ok: ActionEnvelope = CapabilitySucceeded.into();
    let err: ActionEnvelope = CapabilityFailed.into();
    ctx.effects
        .camera()
        .request_permission(CameraPermissionRequest {
            reason: Some("Scan the selected asset label".into()),
        })
        .on_ok(ok.clone())
        .on_err(err.clone());
    ctx.effects
        .barcode_scanner()
        .scan(BarcodeScanRequest {
            formats: vec![BarcodeFormat::QrCode, BarcodeFormat::Code128],
            prompt: Some(format!("Scan {}", state.selected_order().asset.id)),
            camera_id: None,
            timeout_ms: Some(10_000),
            allow_multiple: false,
        })
        .on_ok(ok.clone())
        .on_err(err.clone());
    ctx.effects.haptics().selection().on_ok(ok).on_err(err);
}

#[fission_reducer(VerifyWithNfc)]
pub fn on_verify_with_nfc(
    state: &mut FieldInspectorState,
    ctx: &mut ReducerContext<FieldInspectorState>,
) {
    state.panel = InspectorPanel::Verify;
    state.log(
        "NFC scan",
        "Waiting for the asset service tag",
        CapabilityState::Pending,
    );
    let ok: ActionEnvelope = CapabilitySucceeded.into();
    let err: ActionEnvelope = CapabilityFailed.into();
    ctx.effects
        .nfc()
        .scan_tag(NfcScanRequest {
            technologies: vec![NfcTechnology::Ndef],
            message: Some(format!("Tap {}", state.selected_order().asset.id)),
            timeout_ms: Some(10_000),
            read_multiple_records: false,
        })
        .on_ok(ok.clone())
        .on_err(err.clone());
    ctx.effects
        .haptics()
        .impact(HapticImpactRequest {
            style: HapticImpactStyle::Medium,
        })
        .on_ok(ok)
        .on_err(err);
}

#[fission_reducer(CaptureEvidencePhoto)]
pub fn on_capture_evidence_photo(
    state: &mut FieldInspectorState,
    ctx: &mut ReducerContext<FieldInspectorState>,
) {
    state.panel = InspectorPanel::Evidence;
    state.log(
        "Photo capture",
        "Capturing a still image for the report",
        CapabilityState::Pending,
    );
    let ok: ActionEnvelope = CapabilitySucceeded.into();
    let err: ActionEnvelope = CapabilityFailed.into();
    ctx.effects
        .camera()
        .request_permission(CameraPermissionRequest {
            reason: Some("Capture a still image for the field report".into()),
        })
        .on_ok(ok.clone())
        .on_err(err.clone());
    ctx.effects
        .camera()
        .capture_photo(CameraCaptureRequest {
            camera_id: None,
            facing: CameraFacing::Back,
            resolution: Some(CameraResolution {
                width: 1600,
                height: 1200,
            }),
            format: CameraImageFormat::Jpeg,
            flash: if state.torch_on {
                CameraFlashMode::On
            } else {
                CameraFlashMode::Auto
            },
            quality: Some(86),
        })
        .on_ok(ok)
        .on_err(err);
}

#[fission_reducer(ToggleTorch)]
pub fn on_toggle_torch(
    state: &mut FieldInspectorState,
    ctx: &mut ReducerContext<FieldInspectorState>,
) {
    state.panel = InspectorPanel::Evidence;
    state.torch_on = !state.torch_on;
    let ok: ActionEnvelope = CapabilitySucceeded.into();
    let err: ActionEnvelope = CapabilityFailed.into();
    ctx.effects
        .camera()
        .set_flashlight(CameraFlashlightRequest {
            camera_id: None,
            enabled: state.torch_on,
            intensity: Some(if state.torch_on { 80 } else { 0 }),
        })
        .on_ok(ok)
        .on_err(err);
}

#[fission_reducer(RecordVoiceNote)]
pub fn on_record_voice_note(
    state: &mut FieldInspectorState,
    ctx: &mut ReducerContext<FieldInspectorState>,
) {
    state.panel = InspectorPanel::Evidence;
    state.log(
        "Voice note",
        "Recording a bounded one-second note",
        CapabilityState::Pending,
    );
    let ok: ActionEnvelope = CapabilitySucceeded.into();
    let err: ActionEnvelope = CapabilityFailed.into();
    ctx.effects
        .microphone()
        .capture_audio(MicrophoneCaptureRequest {
            device_id: None,
            duration_ms: 1_000,
            sample_rate_hz: Some(48_000),
            channels: Some(1),
            sample_format: AudioSampleFormat::F32,
        })
        .on_ok(ok)
        .on_err(err);
}

#[fission_reducer(PhotoPreviewLoaded)]
pub fn on_photo_preview_loaded(
    state: &mut FieldInspectorState,
    ctx: &mut ReducerContext<FieldInspectorState>,
) {
    if let Some(preview) = ctx.input.job_ok(STREAM_BYTES_JOB) {
        state.photo_preview = Some(preview.bytes);
    }
}

#[fission_reducer(PhotoPreviewFailed)]
pub fn on_photo_preview_failed(
    state: &mut FieldInspectorState,
    ctx: &mut ReducerContext<FieldInspectorState>,
) {
    let message = ctx
        .input
        .job_err(STREAM_BYTES_JOB)
        .map(|error| error.message)
        .or_else(|| {
            ctx.input
                .job_error_message(STREAM_BYTES_JOB)
                .map(str::to_string)
        })
        .unwrap_or_else(|| "Photo preview stream could not be read".into());
    state.log("Photo preview", message, CapabilityState::Warning);
}
