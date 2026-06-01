use crate::components::status::ActivityLog;
use crate::components::ui::{
    action_button, body_text, is_compact, metric, muted_text, panel_card, responsive_grid,
    small_button, soft_panel, status_pill, title_text, usable_width,
};
use crate::model::*;
use fission::prelude::*;
use fission::IntoWidget;

#[derive(Clone)]
pub struct VerifyPanel;

impl Widget<FieldInspectorState> for VerifyPanel {
    fn build(
        &self,
        ctx: &mut BuildCtx<FieldInspectorState>,
        view: &View<FieldInspectorState>,
    ) -> impl fission::IntoWidget<FieldInspectorState> {
        fission::core::view::internal_node_widget({
            let order = view.state.selected_order();
            let scan_barcode = with_reducer!(ctx, VerifyWithBarcode, on_verify_with_barcode);
            let scan_nfc = with_reducer!(ctx, VerifyWithNfc, on_verify_with_nfc);
            panel_card(
            view,
            Column {
                gap: Some(16.0),
                children: vec![
                    section_header(view, "Verify the asset", "Use the field label and the embedded service tag to prove the technician is inspecting the right physical unit."),
                    responsive_grid(
                        view,
                        vec![
                            metric(view, "Expected barcode", order.asset.expected_barcode),
                            metric(view, "Expected NFC", order.asset.expected_nfc_uri),
                        ],
                        2,
                    ),
                    Row {
                        gap: Some(12.0),
                        wrap: ir_op::FlexWrap::Wrap,
                        children: vec![
                            action_button("Scan barcode", scan_barcode, ButtonVariant::Primary),
                            action_button("Tap NFC tag", scan_nfc, ButtonVariant::SecondaryColor),
                        ],
                        ..Default::default()
                    }.into_node(),
                    result_line(view, "Barcode result", view.state.scanned_barcode.as_ref().and_then(|r| r.items.first()).map(|i| i.value.clone()), view.state.asset_barcode_matches()),
                    result_line(view, "NFC result", view.state.scanned_nfc.as_ref().and_then(nfc_uri_for_display), view.state.asset_nfc_matches()),
                ],
                ..Default::default()
            }.into_node(),
        )
        })
    }
}

#[derive(Clone)]
pub struct EvidencePanel;

impl Widget<FieldInspectorState> for EvidencePanel {
    fn build(
        &self,
        ctx: &mut BuildCtx<FieldInspectorState>,
        view: &View<FieldInspectorState>,
    ) -> impl fission::IntoWidget<FieldInspectorState> {
        fission::core::view::internal_node_widget({
            let capture = with_reducer!(ctx, CaptureEvidencePhoto, on_capture_evidence_photo);
            let torch = with_reducer!(ctx, ToggleTorch, on_toggle_torch);
            let record = with_reducer!(ctx, RecordVoiceNote, on_record_voice_note);
            let controls = Column {
                gap: Some(12.0),
                children: vec![
                    action_button("Capture photo", capture, ButtonVariant::Primary),
                    action_button(
                        if view.state.torch_on {
                            "Turn torch off"
                        } else {
                            "Turn torch on"
                        },
                        torch,
                        ButtonVariant::SecondaryGray,
                    ),
                    action_button("Record voice note", record, ButtonVariant::SecondaryColor),
                    metric(
                        view,
                        "Camera",
                        view.state
                            .camera_availability
                            .as_ref()
                            .map(|a| format!("{} device(s)", a.devices.len()))
                            .unwrap_or_else(|| "Not checked".into()),
                    ),
                    metric(
                        view,
                        "Microphone",
                        view.state
                            .microphone_availability
                            .as_ref()
                            .map(|a| format!("{} input(s)", a.devices.len()))
                            .unwrap_or_else(|| "Not checked".into()),
                    ),
                ],
                ..Default::default()
            }
            .into_node();
            let capture_layout = if is_compact(view) {
                Column {
                    gap: Some(14.0),
                    children: vec![controls, evidence_photo(view)],
                    ..Default::default()
                }
                .into_node()
            } else {
                Grid {
                    columns: vec![ir_op::GridTrack::Fr(1.1), ir_op::GridTrack::Fr(0.9)],
                    column_gap: Some(14.0),
                    row_gap: Some(14.0),
                    children: vec![
                        GridItem::new(evidence_photo(view)).cell(1, 1).into_node(),
                        GridItem::new(controls).cell(1, 2).into_node(),
                    ],
                    ..Default::default()
                }
                .into_node()
            };
            panel_card(
            view,
            Column {
                gap: Some(16.0),
                children: vec![
                    section_header(view, "Collect evidence", "Capture a still image, use the flashlight when the host supports it, and attach a short voice note without blocking the UI."),
                    capture_layout,
                ],
                ..Default::default()
            }.into_node(),
        )
        })
    }
}

#[derive(Clone)]
pub struct SensorsPanel;

impl Widget<FieldInspectorState> for SensorsPanel {
    fn build(
        &self,
        ctx: &mut BuildCtx<FieldInspectorState>,
        view: &View<FieldInspectorState>,
    ) -> impl fission::IntoWidget<FieldInspectorState> {
        fission::core::view::internal_node_widget({
            let scan = with_reducer!(ctx, ScanSensors, on_scan_sensors);
            let read = with_reducer!(ctx, ReadSensor, on_read_sensor);
            let connect = view.state.bluetooth_devices.first().map(|device| {
                with_reducer!(ctx, ConnectSensor(device.id.clone()), on_connect_sensor)
            });
            let mut actions = vec![action_button(
                "Scan nearby devices",
                scan,
                ButtonVariant::Primary,
            )];
            if let Some(action) = connect {
                actions.push(action_button(
                    "Connect bridge",
                    action,
                    ButtonVariant::SecondaryColor,
                ));
            }
            actions.push(action_button(
                "Read telemetry",
                read,
                ButtonVariant::SecondaryGray,
            ));

            panel_card(
            view,
            Column {
                gap: Some(16.0),
                children: vec![
                    section_header(view, "Read local context", "Nearby Bluetooth and Wi-Fi data belong behind host capabilities because hardware and permissions vary across platforms."),
                    Row { gap: Some(12.0), wrap: ir_op::FlexWrap::Wrap, children: actions, ..Default::default() }.into_node(),
                    responsive_grid(
                        view,
                        vec![
                            metric(view, "Bluetooth devices", view.state.bluetooth_devices.len().to_string()),
                            metric(view, "Wi-Fi networks", view.state.wifi_networks.len().to_string()),
                            metric(view, "Sensor reading", view.state.sensor_reading.clone().unwrap_or_else(|| "Pending".into())),
                        ],
                        3,
                    ),
                    device_list(view),
                ],
                ..Default::default()
            }.into_node(),
        )
        })
    }
}

#[derive(Clone)]
pub struct SecurityPanel;

impl Widget<FieldInspectorState> for SecurityPanel {
    fn build(
        &self,
        ctx: &mut BuildCtx<FieldInspectorState>,
        view: &View<FieldInspectorState>,
    ) -> impl fission::IntoWidget<FieldInspectorState> {
        fission::core::view::internal_node_widget({
            let unlock = with_reducer!(ctx, SecureUnlock, on_secure_unlock);
            let register = with_reducer!(ctx, RegisterPasskey, on_register_passkey);
            let auth = with_reducer!(ctx, AuthenticatePasskey, on_authenticate_passkey);
            panel_card(
            view,
            Column {
                gap: Some(16.0),
                children: vec![
                    section_header(view, "Unlock protected site data", "Biometrics verify the local user. Passkeys produce credential data that a backend would verify before granting account access."),
                    Row {
                        gap: Some(12.0),
                        wrap: ir_op::FlexWrap::Wrap,
                        children: vec![
                            action_button("Biometric unlock", unlock, ButtonVariant::Primary),
                            action_button("Register passkey", register, ButtonVariant::SecondaryColor),
                            action_button("Authenticate passkey", auth, ButtonVariant::SecondaryGray),
                        ],
                        ..Default::default()
                    }.into_node(),
                    responsive_grid(
                        view,
                        vec![
                            metric(view, "Protected notes", if view.state.sensitive_unlocked { "Unlocked" } else { "Locked" }),
                            metric(view, "Account proof", if view.state.passkey_verified { "Passkey verified" } else { "Pending" }),
                        ],
                        2,
                    ),
                    protected_notes(view),
                ],
                ..Default::default()
            }.into_node(),
        )
        })
    }
}

#[derive(Clone)]
pub struct ReviewPanel;

impl Widget<FieldInspectorState> for ReviewPanel {
    fn build(
        &self,
        ctx: &mut BuildCtx<FieldInspectorState>,
        view: &View<FieldInspectorState>,
    ) -> impl fission::IntoWidget<FieldInspectorState> {
        fission::core::view::internal_node_widget({
            let copy = with_reducer!(ctx, CopyReportSummary, on_copy_report_summary);
            let reminder = with_reducer!(ctx, ScheduleReminder, on_schedule_reminder);
            let vol_down = with_reducer!(
                ctx,
                AdjustAlertVolume(VolumeAdjustDirection::Down),
                on_adjust_alert_volume
            );
            let vol_up = with_reducer!(
                ctx,
                AdjustAlertVolume(VolumeAdjustDirection::Up),
                on_adjust_alert_volume
            );
            let submit = with_reducer!(ctx, SubmitReport, on_submit_report);
            let deep_link = ctx.bind(
                DeepLinkReceived {
                    link: DeepLink::new(format!(
                        "field-inspector://work-orders/{}",
                        view.state.selected_order().id
                    ))
                    .source(DeepLinkSource::CustomScheme),
                },
                reduce_with!(on_deep_link_received),
            );
            panel_card(
            view,
            Column {
                gap: Some(16.0),
                children: vec![
                    section_header(view, "Review and submit", "The report gathers host-provided context into a plain summary that can be copied, linked from notifications, or submitted."),
                    soft_panel(view, body_text(view, view.state.report_summary())),
                    Row {
                        gap: Some(10.0),
                        wrap: ir_op::FlexWrap::Wrap,
                        children: vec![
                            action_button("Copy summary", copy, ButtonVariant::SecondaryGray),
                            action_button("Schedule reminder", reminder, ButtonVariant::SecondaryColor),
                            action_button("Open deep link", deep_link, ButtonVariant::Ghost),
                            small_button("Volume -", vol_down, ButtonVariant::Ghost),
                            small_button("Volume +", vol_up, ButtonVariant::Ghost),
                            action_button(if view.state.report_submitted { "Submitted" } else { "Submit report" }, submit, ButtonVariant::Primary),
                        ],
                        ..Default::default()
                    }.into_node(),
                    ActivityLog.build(ctx, view).into_widget().lower_to_node(ctx, view),
                ],
                ..Default::default()
            }.into_node(),
        )
        })
    }
}

fn section_header(
    view: &View<FieldInspectorState>,
    title: &'static str,
    body: &'static str,
) -> Node {
    Column {
        gap: Some(5.0),
        children: vec![title_text(view, title, 22.0), muted_text(view, body)],
        ..Default::default()
    }
    .into_node()
}

fn result_line(
    view: &View<FieldInspectorState>,
    label: &'static str,
    value: Option<String>,
    ok: bool,
) -> Node {
    soft_panel(
        view,
        Row {
            gap: Some(10.0),
            children: vec![
                Column {
                    gap: Some(3.0),
                    flex_grow: 1.0,
                    children: vec![
                        Text::new(label).size(14.0).weight(800).into_node(),
                        muted_text(view, value.unwrap_or_else(|| "Waiting for scan".into())),
                    ],
                    ..Default::default()
                }
                .into_node(),
                status_pill(
                    view,
                    if ok { "Matched" } else { "Pending" },
                    if ok {
                        CapabilityState::Complete
                    } else {
                        CapabilityState::Idle
                    },
                ),
            ],
            ..Default::default()
        }
        .into_node(),
    )
}

fn evidence_photo(view: &View<FieldInspectorState>) -> Node {
    let order = view.state.selected_order();
    let width = usable_width(view, if is_compact(view) { 96.0 } else { 0.0 }).min(520.0);
    let height = if is_compact(view) {
        width * 0.48
    } else {
        320.0
    };
    let image = if let Some(capture) = &view.state.photo_capture {
        Image::memory(capture.bytes.clone())
            .size(width, height)
            .fit(ir_op::ImageFit::Cover)
            .semantic_label("Captured evidence photo")
            .into_node()
    } else {
        Image::network(order.asset.photo_url)
            .size(width, height)
            .fit(ir_op::ImageFit::Cover)
            .semantic_label(order.asset.name)
            .into_node()
    };
    Container::<Node>::lowered(image)
        .bg(view.env.theme.tokens.colors.background)
        .border_radius(18.0)
        .into_node()
}

fn device_list(view: &View<FieldInspectorState>) -> Node {
    let mut children = Vec::new();
    for device in &view.state.bluetooth_devices {
        children.push(soft_panel(
            view,
            Row {
                gap: Some(10.0),
                children: vec![
                    Column {
                        gap: Some(3.0),
                        flex_grow: 1.0,
                        children: vec![
                            Text::new(device.name.clone().unwrap_or_else(|| device.id.clone()))
                                .size(15.0)
                                .weight(800)
                                .into_node(),
                            muted_text(
                                view,
                                format!("RSSI {:?}, paired {}", device.rssi, device.paired),
                            ),
                        ],
                        ..Default::default()
                    }
                    .into_node(),
                    status_pill(view, "Bluetooth", CapabilityState::Ready),
                ],
                ..Default::default()
            }
            .into_node(),
        ));
    }
    for network in &view.state.wifi_networks {
        children.push(soft_panel(
            view,
            Row {
                gap: Some(10.0),
                children: vec![
                    Column {
                        gap: Some(3.0),
                        flex_grow: 1.0,
                        children: vec![
                            Text::new(network.ssid.clone())
                                .size(15.0)
                                .weight(800)
                                .into_node(),
                            muted_text(
                                view,
                                format!("RSSI {:?}, security {:?}", network.rssi, network.security),
                            ),
                        ],
                        ..Default::default()
                    }
                    .into_node(),
                    status_pill(
                        view,
                        if network.connected {
                            "Connected"
                        } else {
                            "Visible"
                        },
                        CapabilityState::Ready,
                    ),
                ],
                ..Default::default()
            }
            .into_node(),
        ));
    }
    if children.is_empty() {
        children.push(body_text(
            view,
            "No nearby device data yet. Run a sensor scan.",
        ));
    }
    Column {
        gap: Some(10.0),
        children,
        ..Default::default()
    }
    .into_node()
}

fn protected_notes(view: &View<FieldInspectorState>) -> Node {
    if view.state.sensitive_unlocked {
        soft_panel(
            view,
            Column {
                gap: Some(6.0),
                children: vec![
                    Text::new("Site access note").size(16.0).weight(900).into_node(),
                    body_text(view, "Door code expires after this shift. Escalate compressor pressure above 14 bar to the site manager before leaving."),
                ],
                ..Default::default()
            }.into_node(),
        )
    } else {
        soft_panel(
            view,
            body_text(
                view,
                "Protected notes stay hidden until the host verifies the technician.",
            ),
        )
    }
}
