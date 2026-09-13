//! Reducers for choosing an order, working through its checklist and filing the report.

use super::*;

#[fission_reducer(SelectOrder)]
pub fn on_select_order(state: &mut FieldInspectorState, order_id: String) {
    if state.orders.iter().any(|order| order.id == order_id) {
        state.reset_for_order(order_id);
    }
}

#[fission_reducer(SelectPanel)]
pub fn on_select_panel(state: &mut FieldInspectorState, panel: InspectorPanel) {
    state.panel = panel;
}

#[fission_reducer(StartInspection)]
pub fn on_start_inspection(
    state: &mut FieldInspectorState,
    ctx: &mut ReducerContext<FieldInspectorState>,
) {
    state.started = true;
    state.weather = AsyncSnapshot::waiting();
    state.weather_generation = state.weather_generation.saturating_add(1);
    state.log(
        "Inspection started",
        "Capability readiness checks are running",
        CapabilityState::Pending,
    );

    let ok: ActionEnvelope = CapabilitySucceeded.into();
    let err: ActionEnvelope = CapabilityFailed.into();

    ctx.effects
        .notifications()
        .settings()
        .on_ok(ok.clone())
        .on_err(err.clone());
    ctx.effects
        .geolocation()
        .permission()
        .on_ok(ok.clone())
        .on_err(err.clone());
    ctx.effects
        .geolocation()
        .request_permission(GeolocationPermissionRequest {
            precise: true,
            background: false,
        })
        .on_ok(ok.clone())
        .on_err(err.clone());
    ctx.effects
        .geolocation()
        .current_position(GeolocationPositionRequest {
            high_accuracy: true,
            timeout_ms: Some(5_000),
            maximum_age_ms: Some(60_000),
        })
        .on_ok(ok.clone())
        .on_err(err.clone());
    ctx.effects
        .camera()
        .availability()
        .on_ok(ok.clone())
        .on_err(err.clone());
    ctx.effects
        .camera()
        .request_permission(CameraPermissionRequest {
            reason: Some("Capture field evidence and scan asset labels".into()),
        })
        .on_ok(ok.clone())
        .on_err(err.clone());
    ctx.effects
        .microphone()
        .availability()
        .on_ok(ok.clone())
        .on_err(err.clone());
    ctx.effects
        .microphone()
        .request_permission(MicrophonePermissionRequest {
            reason: Some("Attach a short voice note to the field report".into()),
        })
        .on_ok(ok.clone())
        .on_err(err.clone());
    ctx.effects
        .nfc()
        .availability()
        .on_ok(ok.clone())
        .on_err(err.clone());
    ctx.effects
        .biometrics()
        .availability()
        .on_ok(ok.clone())
        .on_err(err.clone());
    ctx.effects
        .passkeys()
        .availability()
        .on_ok(ok.clone())
        .on_err(err.clone());
    ctx.effects
        .bluetooth()
        .availability()
        .on_ok(ok.clone())
        .on_err(err.clone());
    ctx.effects
        .bluetooth()
        .scan_devices(BluetoothScanRequest {
            service_uuids: vec![state.selected_order().asset.sensor_service_uuid.to_string()],
            timeout_ms: Some(2_000),
            include_paired: true,
            allow_duplicates: false,
        })
        .on_ok(ok.clone())
        .on_err(err.clone());
    ctx.effects
        .wifi()
        .availability()
        .on_ok(ok.clone())
        .on_err(err.clone());
    ctx.effects
        .wifi()
        .scan_networks(WifiScanRequest {
            ssid_prefix: None,
            include_hidden: false,
            timeout_ms: Some(2_000),
        })
        .on_ok(ok.clone())
        .on_err(err.clone());
    ctx.effects
        .volume()
        .get_level(VolumeStream::Media)
        .on_ok(ok)
        .on_err(err);
}

#[fission_reducer(CopyReportSummary)]
pub fn on_copy_report_summary(
    state: &mut FieldInspectorState,
    ctx: &mut ReducerContext<FieldInspectorState>,
) {
    state.panel = InspectorPanel::Review;
    let summary = state.report_summary();
    state.copied_summary = Some(summary.clone());
    state.complete_check("report");
    let ok: ActionEnvelope = CapabilitySucceeded.into();
    let err: ActionEnvelope = CapabilityFailed.into();
    ctx.effects
        .clipboard()
        .write_text(ClipboardWriteTextRequest { text: summary })
        .on_ok(ok)
        .on_err(err);
}

#[fission_reducer(ScheduleReminder)]
pub fn on_schedule_reminder(
    state: &mut FieldInspectorState,
    ctx: &mut ReducerContext<FieldInspectorState>,
) {
    state.panel = InspectorPanel::Review;
    let order = state.selected_order();
    let ok: ActionEnvelope = CapabilitySucceeded.into();
    let err: ActionEnvelope = CapabilityFailed.into();
    ctx.effects
        .notifications()
        .schedule(NotificationRequest {
            id: NotificationId::new(format!("{}-reminder", order.id)),
            title: format!("Inspection reminder: {}", order.id),
            body: format!("Finish {} before {}", order.asset.name, order.due),
            subtitle: Some(order.site.into()),
            badge: Some(1),
            sound: NotificationSound::Default,
            deep_link: Some(format!("field-inspector://work-orders/{}", order.id)),
            actions: vec![NotificationActionButton {
                id: "open".into(),
                title: "Open job".into(),
                foreground: true,
                ..Default::default()
            }],
            schedule: NotificationSchedule::AfterMillis(30_000),
        })
        .on_ok(ok.clone())
        .on_err(err.clone());
    ctx.effects
        .notifications()
        .set_badge_count(SetBadgeCountRequest { count: Some(1) })
        .on_ok(ok)
        .on_err(err);
}

#[fission_reducer(AdjustAlertVolume)]
pub fn on_adjust_alert_volume(
    state: &mut FieldInspectorState,
    direction: VolumeAdjustDirection,
    ctx: &mut ReducerContext<FieldInspectorState>,
) {
    state.panel = InspectorPanel::Review;
    let ok: ActionEnvelope = CapabilitySucceeded.into();
    let err: ActionEnvelope = CapabilityFailed.into();
    ctx.effects
        .volume()
        .adjust_level(VolumeAdjustRequest {
            stream: VolumeStream::Media,
            direction,
            step: 8,
        })
        .on_ok(ok)
        .on_err(err);
}

#[fission_reducer(SubmitReport)]
pub fn on_submit_report(
    state: &mut FieldInspectorState,
    ctx: &mut ReducerContext<FieldInspectorState>,
) {
    state.panel = InspectorPanel::Review;
    state.report_submitted = true;
    state.complete_check("report");
    let order = state.selected_order();
    let ok: ActionEnvelope = CapabilitySucceeded.into();
    let err: ActionEnvelope = CapabilityFailed.into();
    ctx.effects
        .notifications()
        .show(NotificationRequest {
            id: NotificationId::new(format!("{}-submitted", order.id)),
            title: "Inspection submitted".into(),
            body: format!("{} is ready for review", order.asset.id),
            subtitle: Some(order.site.into()),
            badge: Some(0),
            sound: NotificationSound::Default,
            deep_link: Some(format!("field-inspector://work-orders/{}/report", order.id)),
            actions: Vec::new(),
            schedule: NotificationSchedule::Immediate,
        })
        .on_ok(ok.clone())
        .on_err(err.clone());
    ctx.effects
        .haptics()
        .notification(HapticNotificationRequest {
            kind: HapticNotificationKind::Success,
        })
        .on_ok(ok)
        .on_err(err);
}

#[fission_reducer(CompleteChecklist)]
pub fn on_complete_checklist(state: &mut FieldInspectorState, id: String) {
    state.complete_check(&id);
}
