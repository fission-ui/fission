use super::file_picker::{InlineFilePicker, SelectedFileActions};
use super::panel::{NumberedPanel, PanelTone};
use super::primitives::*;
use super::*;

#[derive(Clone)]
pub(super) struct PublishBoardCanvas {
    pub(super) layout: PublishLayout,
}

impl From<PublishBoardCanvas> for Widget {
    fn from(canvas: PublishBoardCanvas) -> Widget {
        let (_ctx, view) = fission::build::current::<PublishUiState>();
        let palette = PublishPalette::for_mode(view.state().theme_mode);
        let content: Widget = match view.state().board {
            PublishBoard::Android => AndroidBoard {
                layout: canvas.layout,
            }
            .into(),
            PublishBoard::Ios => IosBoard {
                layout: canvas.layout,
            }
            .into(),
            PublishBoard::Windows => WindowsBoard {
                layout: canvas.layout,
            }
            .into(),
            PublishBoard::S3 => S3Board {
                layout: canvas.layout,
            }
            .into(),
        };
        if canvas.layout.terminal {
            return Container::new(content)
                .padding([0.0, 0.0, 0.0, 0.0])
                .bg(palette.background)
                .into();
        }
        Scroll {
            direction: FlexDirection::Column,
            height: Some(canvas.layout.body_height),
            child: Some(
                Container::new(content)
                    .padding([
                        0.0,
                        0.0,
                        0.0,
                        if canvas.layout.terminal { 0.0 } else { 4.0 },
                    ])
                    .bg(palette.background)
                    .into(),
            ),
            show_scrollbar: false,
            ..Default::default()
        }
        .into()
    }
}

#[derive(Clone)]
struct BoardRows {
    rows: Vec<Vec<NumberedPanel>>,
}

impl From<BoardRows> for Widget {
    fn from(board: BoardRows) -> Widget {
        let (_ctx, view) = fission::build::current::<PublishUiState>();
        let layout = PublishLayout::from_viewport(view.env().viewport_size);
        let panels = board.rows.into_iter().flatten().collect::<Vec<_>>();
        let total = panels.len().max(1);
        let step = view.state().current_step.clamp(1, total);
        let labels = panels
            .iter()
            .map(|panel| panel.title.clone())
            .collect::<Vec<_>>();
        let mut current = panels
            .get(step - 1)
            .cloned()
            .unwrap_or_else(|| NumberedPanel {
                number: 1,
                title: "Publish step".into(),
                subtitle: "No step is available.".into(),
                width: layout.column_width,
                height: None,
                children: Vec::new(),
                tone: PanelTone::Normal,
            });
        current.width = layout.wizard_width(view.env().viewport_size.width);
        current.height = None;
        Column {
            gap: Some(layout.gap),
            align_items: AlignItems::Center,
            children: widgets![
                WizardStepper {
                    step,
                    total,
                    labels
                },
                Row {
                    justify_content: JustifyContent::Center,
                    children: widgets![current],
                    ..Default::default()
                },
                WizardNavigation { step, total },
            ],
            ..Default::default()
        }
        .into()
    }
}

#[derive(Clone)]
struct WizardStepper {
    step: usize,
    total: usize,
    labels: Vec<String>,
}

impl From<WizardStepper> for Widget {
    fn from(stepper: WizardStepper) -> Widget {
        let (_ctx, view) = fission::build::current::<PublishUiState>();
        let layout = PublishLayout::from_viewport(view.env().viewport_size);
        let palette = PublishPalette::for_mode(view.state().theme_mode);
        let title = stepper
            .labels
            .get(stepper.step - 1)
            .cloned()
            .unwrap_or_default();
        if layout.terminal {
            Text::new(format!(
                "Step {} of {} - {}",
                stepper.step, stepper.total, title
            ))
            .color(palette.accent)
            .into()
        } else {
            Container::new(Row {
                gap: Some(layout.gap),
                align_items: AlignItems::Center,
                children: widgets![
                    Container::new(
                        Text::new(format!("{} / {}", stepper.step, stepper.total))
                            .size(13.0)
                            .color(palette.accent_text),
                    )
                    .width(layout.button_height * 1.55)
                    .height(layout.button_height * 0.9)
                    .padding([10.0, 10.0, 7.0, 7.0])
                    .bg(palette.accent)
                    .border_radius(999.0),
                    Column {
                        gap: Some(layout.gap * 0.18),
                        children: widgets![
                            Text::new(title).size(17.0).color(palette.text),
                            Text::new("Use Continue and Back to move through the publish wizard one screen at a time.")
                                .size(12.0)
                                .color(palette.muted),
                        ],
                        ..Default::default()
                    }
                    .flex_grow(1.0),
                    Row {
                        gap: Some(layout.gap * 0.5),
                        children: (1..=stepper.total)
                            .map(|index| ProgressDot {
                                active: index == stepper.step,
                                completed: index < stepper.step,
                            }
                            .into())
                            .collect(),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            })
            .width(layout.wizard_width(view.env().viewport_size.width))
            .padding([
                layout.card_padding * 0.85,
                layout.card_padding * 0.85,
                layout.card_padding * 0.65,
                layout.card_padding * 0.65,
            ])
            .bg(palette.background_alt)
            .border(palette.hairline, 1.0)
            .border_radius(layout.panel_radius * 0.85)
            .into()
        }
    }
}

#[derive(Clone)]
struct ProgressDot {
    active: bool,
    completed: bool,
}

impl From<ProgressDot> for Widget {
    fn from(dot: ProgressDot) -> Widget {
        let (_ctx, view) = fission::build::current::<PublishUiState>();
        let palette = PublishPalette::for_mode(view.state().theme_mode);
        let bg = if dot.active {
            palette.accent
        } else if dot.completed {
            palette.blue
        } else {
            palette.hairline
        };
        Container::new(Spacer::default())
            .width(if dot.active { 30.0 } else { 10.0 })
            .height(10.0)
            .bg(bg)
            .border_radius(999.0)
            .into()
    }
}

#[derive(Clone)]
struct StepSummary {
    step: usize,
    total: usize,
}

impl From<StepSummary> for Widget {
    fn from(summary: StepSummary) -> Widget {
        let (_ctx, view) = fission::build::current::<PublishUiState>();
        let layout = PublishLayout::from_viewport(view.env().viewport_size);
        let palette = PublishPalette::for_mode(view.state().theme_mode);
        Text::new(format!("Step {} of {}", summary.step, summary.total))
            .size(if layout.terminal { 11.0 } else { 12.0 })
            .color(palette.muted)
            .into()
    }
}

#[derive(Clone)]
struct WizardNavigation {
    step: usize,
    total: usize,
}

impl From<WizardNavigation> for Widget {
    fn from(nav: WizardNavigation) -> Widget {
        let (ctx, _view) = fission::build::current::<PublishUiState>();
        let back = with_reducer!(ctx, PublishPreviousStep, publish_previous_step);
        let next = with_reducer!(ctx, PublishNextStep, publish_next_step);
        Row {
            gap: Some(10.0),
            align_items: AlignItems::Center,
            children: widgets![
                Container::new(StepSummary {
                    step: nav.step,
                    total: nav.total
                })
                .flex_grow(1.0),
                PublishButton {
                    label: "Back".into(),
                    action: if nav.step > 1 { Some(back) } else { None },
                    tone: ButtonTone::Quiet,
                    width: 108.0,
                },
                PublishButton {
                    label: if nav.step == nav.total {
                        "Finish".into()
                    } else {
                        "Continue".into()
                    },
                    action: if nav.step < nav.total {
                        Some(next)
                    } else {
                        None
                    },
                    tone: ButtonTone::Primary,
                    width: 132.0,
                },
            ],
            ..Default::default()
        }
        .into()
    }
}

#[derive(Clone)]
struct AndroidBoard {
    layout: PublishLayout,
}

impl From<AndroidBoard> for Widget {
    fn from(board: AndroidBoard) -> Widget {
        let (ctx, view) = fission::build::current::<PublishUiState>();
        let m = metrics(board.layout);
        let set_play = with_reducer!(
            ctx,
            PublishSetPlayJson(String::new()),
            publish_set_play_json
        );
        let set_jks = with_reducer!(
            ctx,
            PublishSetAndroidJks(String::new()),
            publish_set_android_jks
        );
        let set_alias = with_reducer!(
            ctx,
            PublishSetAndroidAlias(String::new()),
            publish_set_android_alias
        );
        let set_pass = with_reducer!(
            ctx,
            PublishSetAndroidPassword(String::new()),
            publish_set_android_password
        );
        let set_track = with_reducer!(ctx, PublishSetTrack(String::new()), publish_set_track);
        let set_locales = with_reducer!(ctx, PublishSetLocales(String::new()), publish_set_locales);
        let pick_play = with_reducer!(
            ctx,
            PublishOpenFilePicker(FilePurpose::PlayServiceJson),
            publish_open_file_picker
        );
        let pick_jks = with_reducer!(
            ctx,
            PublishOpenFilePicker(FilePurpose::AndroidKeystore),
            publish_open_file_picker
        );
        let save = with_reducer!(ctx, PublishSaveCredentials, publish_save_credentials);
        let generate = with_reducer!(
            ctx,
            PublishStartTask(PublishTaskKind::GenerateAndroidKey),
            publish_start_task
        );
        let package = with_reducer!(
            ctx,
            PublishStartTask(PublishTaskKind::Package),
            publish_start_task
        );
        let dry = with_reducer!(
            ctx,
            PublishStartTask(PublishTaskKind::DryRun),
            publish_start_task
        );
        let publish = with_reducer!(
            ctx,
            PublishStartTask(PublishTaskKind::Publish),
            publish_start_task
        );
        let confirm = with_reducer!(
            ctx,
            PublishSetConfirmation(String::new()),
            publish_set_confirmation
        );
        BoardRows { rows: vec![
            vec![
                NumberedPanel { number: 1, title: "Android-specific preflight".into(), subtitle: "We'll check your Android toolchain and environment.".into(), width: m.col, height: m.top_h, tone: tone_for_checks(&view.state().package_checks), children: widgets![
                    CheckList { checks: view.state().package_checks.clone(), limit: 9 },
                    Callout { tone: StatusTone::Info, text: "Install command hints and target setup are shown before build.".into() },
                    Callout { tone: StatusTone::Warning, text: "Missing items must be resolved before publishing.".into() },
                ]},
                NumberedPanel { number: 2, title: "Play Console service-account guidance".into(), subtitle: "Create and grant the service account access to your app.".into(), width: m.col, height: m.top_h, tone: PanelTone::Normal, children: widgets![
                    GuideList { items: vec![
                        "Open Play Console -> Setup -> API access".into(),
                        "Link a Google Cloud project or create a new one".into(),
                        "Create a service account in the linked Cloud project".into(),
                        "Grant app access under Users and permissions".into(),
                        "Enable Android Publisher API".into(),
                        "Download a JSON key and keep it outside git".into(),
                    ]},
                    EnvVarList { title: "Accepted credential sources".into(), names: vec!["GOOGLE_APPLICATION_CREDENTIALS", "PLAY_STORE_SERVICE_ACCOUNT_JSON_BASE64"] },
                    Callout { tone: StatusTone::Info, text: "Secrets are read from environment variables or ~/.fission/<app>/release.env, never fission.toml.".into() },
                ]},
                NumberedPanel { number: 3, title: "Select service account JSON".into(), subtitle: "Pick your Play Console service account JSON key.".into(), width: m.col, height: m.top_h, tone: if view.state().play_json_path.is_empty() { PanelTone::Warning } else { PanelTone::Normal }, children: widgets![
                    KeyValueList { rows: vec![
                        ("Selected".into(), empty_label(&view.state().play_json_path, "No file selected"), if view.state().play_json_path.is_empty() { StatusTone::Warning } else { StatusTone::Info }),
                        ("Workspace".into(), workspace_label(view.state()), StatusTone::Info),
                    ]},
                    ButtonRow { buttons: vec![PublishButton { label: "Choose file...".into(), action: Some(pick_play), tone: ButtonTone::Primary, width: 130.0 }]},
                    InlineFilePicker { purpose: FilePurpose::PlayServiceJson, height: if board.layout.terminal { 22.0 } else { 300.0 } },
                    SelectedFileActions { purpose: FilePurpose::PlayServiceJson },
                    PublishTextField { id: "publish_play_json", label: "Service JSON path".into(), value: view.state().play_json_path.clone(), placeholder: "~/.fission/app/play-service-account.json".into(), on_change: set_play, secret: false, width: m.field_w },
                    Callout { tone: StatusTone::Info, text: if view.state().native_file_dialog { "Choose file opens the native OS file dialog; then reference, copy, or move the selected file into ~/.fission/<app>/.".into() } else { "Use the terminal browser; then reference, copy, or move the selected file into ~/.fission/<app>/.".into() } },
                ]},
                NumberedPanel { number: 4, title: "Upload key (keystore) setup".into(), subtitle: "Play App Signing requires an upload key.".into(), width: m.col, height: m.top_h, tone: if view.state().android_jks_path.is_empty() { PanelTone::Warning } else { PanelTone::Normal }, children: widgets![
                    PublishTextField { id: "publish_android_jks", label: "Existing JKS path".into(), value: view.state().android_jks_path.clone(), placeholder: "~/.fission/app/upload-key.jks".into(), on_change: set_jks, secret: false, width: m.field_w },
                    ButtonRow { buttons: vec![PublishButton { label: "Choose JKS...".into(), action: Some(pick_jks), tone: ButtonTone::Quiet, width: 120.0 }]},
                    InlineFilePicker { purpose: FilePurpose::AndroidKeystore, height: if board.layout.terminal { 14.0 } else { 210.0 } },
                    SelectedFileActions { purpose: FilePurpose::AndroidKeystore },
                    SplitBand { left: widgets![
                        PublishTextField { id: "publish_android_alias", label: "Alias".into(), value: view.state().android_alias.clone(), placeholder: "upload".into(), on_change: set_alias, secret: false, width: 170.0 },
                        PublishTextField { id: "publish_android_password", label: "Store/key password".into(), value: view.state().android_password.clone(), placeholder: "stored in release.env".into(), on_change: set_pass, secret: true, width: 190.0 },
                    ], right: widgets![
                        EnvVarList { title: "Env vars at publish time".into(), names: vec!["ANDROID_KEYSTORE", "ANDROID_KEYSTORE_ALIAS", "ANDROID_KEYSTORE_PASSWORD", "ANDROID_KEY_PASSWORD"] },
                    ]},
                    ButtonRow { buttons: vec![
                        PublishButton { label: "Generate upload key".into(), action: Some(generate), tone: ButtonTone::Success, width: 170.0 },
                        PublishButton { label: "Save".into(), action: Some(save), tone: ButtonTone::Primary, width: 80.0 },
                    ]},
                ]},
            ],
            vec![
                NumberedPanel { number: 5, title: "Release options".into(), subtitle: "Configure what you are releasing and where.".into(), width: m.span2, height: m.bottom_h, tone: PanelTone::Normal, children: widgets![
                    SplitBand { left: widgets![
                        PublishTextField { id: "publish_track", label: "Track".into(), value: view.state().track.clone(), placeholder: "internal".into(), on_change: set_track, secret: false, width: 180.0 },
                        RadioList { items: vec![
                            ("Internal (fast feedback)".into(), view.state().track == "internal"),
                            ("Closed (beta)".into(), view.state().track == "closed"),
                            ("Open (beta)".into(), view.state().track == "open"),
                            ("Production".into(), view.state().track == "production"),
                        ]},
                    ], right: widgets![
                        PublishTextField { id: "publish_locales", label: "Locales to include".into(), value: view.state().locales_input.clone(), placeholder: "pl-PL, en-US".into(), on_change: set_locales, secret: false, width: 240.0 },
                        Text::new("Real Play Store readiness checks").size(12.0).color(PublishPalette::for_mode(view.state().theme_mode).muted),
                        CheckList { checks: view.state().distribution_checks.clone(), limit: 6 },
                    ]},
                    Callout { tone: StatusTone::Warning, text: "The final upload remains locked until package and provider checks pass.".into() },
                ]},
                NumberedPanel { number: 6, title: "Build, package & verify".into(), subtitle: "Build release artifacts and verify signatures.".into(), width: m.col, height: m.bottom_h, tone: PanelTone::Normal, children: widgets![
                    TaskStatusCard { kind: PublishTaskKind::Package, idle_detail: "Not built yet. Press Build artifact to run fission package with the selected target and format.".into() },
                    ReadinessDigest { title: "Package readiness".into(), checks: view.state().package_checks.clone(), empty_detail: "Package checks have not produced a result yet.".into() },
                    ArtifactCard,
                    ButtonRow { buttons: vec![PublishButton { label: "Build artifact".into(), action: Some(package), tone: ButtonTone::Primary, width: 130.0 }]},
                ]},
                NumberedPanel { number: 7, title: "Dry-run & publish".into(), subtitle: "Review changes, then publish to Play Store.".into(), width: m.col, height: m.bottom_h, tone: if view.state().is_ready_to_publish() { PanelTone::Success } else { PanelTone::Warning }, children: widgets![
                    KeyValueList { rows: vec![
                        ("Package".into(), view.state().app_id.clone(), StatusTone::Info),
                        ("Track".into(), view.state().track.clone(), StatusTone::Info),
                        ("Release status".into(), "draft".into(), StatusTone::Info),
                        ("Credentials".into(), if view.state().play_json_path.is_empty() { "missing".into() } else { "configured locally".into() }, if view.state().play_json_path.is_empty() { StatusTone::Warning } else { StatusTone::Info }),
                    ]},
                    PublishGateCard,
                    TaskStatusCard { kind: PublishTaskKind::DryRun, idle_detail: "Dry-run has not been executed in this session.".into() },
                    TaskStatusCard { kind: PublishTaskKind::Publish, idle_detail: "Publish is locked until package/provider checks pass and the app id is typed exactly.".into() },
                    ButtonRow { buttons: vec![
                        PublishButton { label: "Run dry run".into(), action: Some(dry), tone: ButtonTone::Secondary, width: 120.0 },
                        PublishButton { label: if view.state().is_ready_to_publish() { "Publish internal draft".into() } else { "Publish locked".into() }, action: if view.state().is_ready_to_publish() { Some(publish) } else { None }, tone: if view.state().is_ready_to_publish() { ButtonTone::Success } else { ButtonTone::Quiet }, width: 170.0 },
                    ]},
                    PublishTextField { id: "publish_confirmation", label: format!("Type app id to unlock: {}", view.state().app_id), value: view.state().publish_confirmation.clone(), placeholder: view.state().app_id.clone(), on_change: confirm, secret: false, width: m.field_w },
                    Callout { tone: StatusTone::Info, text: "After upload, keep testers and release verification inside Play Console.".into() },
                ]},
            ],
        ]}.into()
    }
}

#[derive(Clone)]
struct IosBoard {
    layout: PublishLayout,
}

impl From<IosBoard> for Widget {
    fn from(board: IosBoard) -> Widget {
        let (ctx, view) = fission::build::current::<PublishUiState>();
        let m = metrics(board.layout);
        let third = (m.full - board.layout.gap * 2.0) / 3.0;
        let key_path = with_reducer!(
            ctx,
            PublishSetAppStoreKeyPath(String::new()),
            publish_set_app_store_key_path
        );
        let key_id = with_reducer!(
            ctx,
            PublishSetAppStoreKeyId(String::new()),
            publish_set_app_store_key_id
        );
        let issuer = with_reducer!(
            ctx,
            PublishSetAppStoreIssuerId(String::new()),
            publish_set_app_store_issuer_id
        );
        let browse = with_reducer!(
            ctx,
            PublishOpenFilePicker(FilePurpose::AppStoreKey),
            publish_open_file_picker
        );
        let save = with_reducer!(ctx, PublishSaveCredentials, publish_save_credentials);
        let package = with_reducer!(
            ctx,
            PublishStartTask(PublishTaskKind::Package),
            publish_start_task
        );
        let dry = with_reducer!(
            ctx,
            PublishStartTask(PublishTaskKind::DryRun),
            publish_start_task
        );
        let publish = with_reducer!(
            ctx,
            PublishStartTask(PublishTaskKind::Publish),
            publish_start_task
        );
        let confirm = with_reducer!(
            ctx,
            PublishSetConfirmation(String::new()),
            publish_set_confirmation
        );
        BoardRows { rows: vec![
            vec![
                NumberedPanel { number: 1, title: "Preflight: iOS Environment".into(), subtitle: "Check host, Xcode, toolchain, and signing setup.".into(), width: third, height: m.top_h, tone: tone_for_checks(&view.state().package_checks), children: widgets![CheckList { checks: view.state().package_checks.clone(), limit: 10 }, Callout { tone: StatusTone::Info, text: "All checks are local and can be re-run before upload.".into() }]},
                NumberedPanel { number: 2, title: "Apple Identity & Bundle Setup".into(), subtitle: "Select team and app identity.".into(), width: third, height: m.top_h, tone: PanelTone::Normal, children: widgets![
                    KeyValueList { rows: vec![("Team".into(), "Apple Developer team".into(), StatusTone::Info), ("Bundle ID".into(), view.state().app_id.clone(), StatusTone::Info), ("Display name".into(), view.state().app_name.clone(), StatusTone::Info), ("Version".into(), "from fission.toml".into(), StatusTone::Muted)]},
                    Callout { tone: StatusTone::Warning, text: "If the bundle is not in App Store Connect, create it there before upload.".into() },
                ]},
                NumberedPanel { number: 3, title: "App Store Connect API Key".into(), subtitle: "Use an API key for secure, non-interactive uploads.".into(), width: third, height: m.top_h, tone: if view.state().app_store_key_path.is_empty() { PanelTone::Warning } else { PanelTone::Normal }, children: widgets![
                    GuideList { items: vec!["Open Users and Access -> Integrations -> App Store Connect API".into(), "Create an API key with App Manager role".into(), "Download the .p8 key once".into(), "Capture Key ID and Issuer ID".into(), "Save the key outside the repository".into()]},
                    EnvVarList { title: "Accepted environment variables".into(), names: vec!["APP_STORE_CONNECT_API_KEY_PATH", "APP_STORE_CONNECT_API_KEY_BASE64", "APP_STORE_CONNECT_KEY_ID", "APP_STORE_CONNECT_ISSUER_ID"] },
                ]},
            ],
            vec![
                NumberedPanel { number: 4, title: "Select API Key File (.p8)".into(), subtitle: "Choose your App Store Connect API key.".into(), width: third, height: m.mid_h, tone: if view.state().app_store_key_path.is_empty() { PanelTone::Warning } else { PanelTone::Normal }, children: widgets![
                    PublishTextField { id: "publish_app_store_key", label: ".p8 key path".into(), value: view.state().app_store_key_path.clone(), placeholder: "~/.fission/app/ios/AuthKey_XXXX.p8".into(), on_change: key_path, secret: false, width: m.field_w },
                    ButtonRow { buttons: vec![PublishButton { label: "Select key file".into(), action: Some(browse), tone: ButtonTone::Primary, width: 130.0 }]},
                    InlineFilePicker { purpose: FilePurpose::AppStoreKey, height: if board.layout.terminal { 14.0 } else { 220.0 } },
                    SelectedFileActions { purpose: FilePurpose::AppStoreKey },
                    PublishTextField { id: "publish_app_store_key_id", label: "Key ID".into(), value: view.state().app_store_key_id.clone(), placeholder: "ABC123DEFG".into(), on_change: key_id, secret: false, width: m.field_w },
                    PublishTextField { id: "publish_app_store_issuer", label: "Issuer ID".into(), value: view.state().app_store_issuer_id.clone(), placeholder: "UUID".into(), on_change: issuer, secret: false, width: m.field_w },
                    ButtonRow { buttons: vec![PublishButton { label: "Save".into(), action: Some(save), tone: ButtonTone::Success, width: 90.0 }]},
                ]},
                NumberedPanel { number: 5, title: "Signing & Provisioning".into(), subtitle: "Validate signing identity and provisioning profile.".into(), width: third, height: m.mid_h, tone: PanelTone::Warning, children: widgets![
                    ReadinessDigest { title: "Signing/provider readiness".into(), checks: view.state().distribution_checks.clone(), empty_detail: "Provider checks will appear after the snapshot loads.".into() },
                    CheckList { checks: view.state().distribution_checks.clone(), limit: 8 },
                    Callout { tone: StatusTone::Warning, text: "Certificate, profile, and bundle identity mismatches are blocking provider checks.".into() },
                ]},
                NumberedPanel { number: 6, title: "Build Archive & Export IPA".into(), subtitle: "Build, archive, and export a signed IPA.".into(), width: third, height: m.mid_h, tone: PanelTone::Normal, children: widgets![
                    TaskStatusCard { kind: PublishTaskKind::Package, idle_detail: "Not built yet. Press Build IPA to run the package pipeline.".into() },
                    ReadinessDigest { title: "Package readiness".into(), checks: view.state().package_checks.clone(), empty_detail: "Package checks have not produced a result yet.".into() },
                    ArtifactCard,
                    ButtonRow { buttons: vec![PublishButton { label: "Build IPA".into(), action: Some(package), tone: ButtonTone::Primary, width: 120.0 }]},
                ]},
            ],
            vec![
                NumberedPanel { number: 7, title: "Destination, Compliance & Upload".into(), subtitle: "Choose where to publish and complete preflight.".into(), width: m.full, height: m.bottom_h, tone: if view.state().is_ready_to_publish() { PanelTone::Success } else { PanelTone::Warning }, children: widgets![
                    SplitBand { left: widgets![RadioList { items: vec![("TestFlight (beta testing)".into(), view.state().track == "testflight"), ("App Store (for review)".into(), view.state().track == "production")]}, PublishTextField { id: "publish_confirmation_ios", label: format!("Type app id to unlock: {}", view.state().app_id), value: view.state().publish_confirmation.clone(), placeholder: view.state().app_id.clone(), on_change: confirm, secret: false, width: m.field_w }, PublishGateCard], right: widgets![TaskStatusCard { kind: PublishTaskKind::DryRun, idle_detail: "Dry-run has not been executed in this session.".into() }, TaskStatusCard { kind: PublishTaskKind::Publish, idle_detail: "Upload is locked until checks pass and the app id is typed exactly.".into() }, ButtonRow { buttons: vec![PublishButton { label: "Run dry run".into(), action: Some(dry), tone: ButtonTone::Secondary, width: 130.0 }, PublishButton { label: if view.state().is_ready_to_publish() { "Upload to TestFlight".into() } else { "Upload locked".into() }, action: if view.state().is_ready_to_publish() { Some(publish) } else { None }, tone: if view.state().is_ready_to_publish() { ButtonTone::Success } else { ButtonTone::Quiet }, width: 165.0 }]}]},
                ]},
            ],
        ]}.into()
    }
}

#[derive(Clone)]
struct WindowsBoard {
    layout: PublishLayout,
}

impl From<WindowsBoard> for Widget {
    fn from(board: WindowsBoard) -> Widget {
        let (ctx, view) = fission::build::current::<PublishUiState>();
        let m = metrics(board.layout);
        let third = (m.full - board.layout.gap * 2.0) / 3.0;
        let pfx = with_reducer!(
            ctx,
            PublishSetWindowsPfx(String::new()),
            publish_set_windows_pfx
        );
        let password = with_reducer!(
            ctx,
            PublishSetWindowsPassword(String::new()),
            publish_set_windows_password
        );
        let tenant = with_reducer!(
            ctx,
            PublishSetAzureTenant(String::new()),
            publish_set_azure_tenant
        );
        let client = with_reducer!(
            ctx,
            PublishSetAzureClient(String::new()),
            publish_set_azure_client
        );
        let secret = with_reducer!(
            ctx,
            PublishSetMicrosoftSecret(String::new()),
            publish_set_microsoft_secret
        );
        let browse = with_reducer!(
            ctx,
            PublishOpenFilePicker(FilePurpose::WindowsCertificate),
            publish_open_file_picker
        );
        let save = with_reducer!(ctx, PublishSaveCredentials, publish_save_credentials);
        let package = with_reducer!(
            ctx,
            PublishStartTask(PublishTaskKind::Package),
            publish_start_task
        );
        let dry = with_reducer!(
            ctx,
            PublishStartTask(PublishTaskKind::DryRun),
            publish_start_task
        );
        let publish = with_reducer!(
            ctx,
            PublishStartTask(PublishTaskKind::Publish),
            publish_start_task
        );
        let confirm = with_reducer!(
            ctx,
            PublishSetConfirmation(String::new()),
            publish_set_confirmation
        );
        BoardRows { rows: vec![
            vec![
                NumberedPanel { number: 1, title: "Windows preflight".into(), subtitle: "Verify your environment before building and submitting.".into(), width: third, height: m.top_h, tone: tone_for_checks(&view.state().package_checks), children: widgets![CheckList { checks: view.state().package_checks.clone(), limit: 9 }, Callout { tone: StatusTone::Info, text: "Issues must be resolved before continuing to submission.".into() }]},
                NumberedPanel { number: 2, title: "Package identity setup".into(), subtitle: "Match local package identity with Microsoft Store.".into(), width: third, height: m.top_h, tone: PanelTone::Normal, children: widgets![KeyValueList { rows: vec![("Package/Identity/Name".into(), view.state().app_id.clone(), StatusTone::Info), ("Publisher CN".into(), "from certificate".into(), StatusTone::Info), ("Display name".into(), view.state().app_name.clone(), StatusTone::Info), ("Version".into(), "from package".into(), StatusTone::Info)]}, Callout { tone: StatusTone::Warning, text: "Any mismatch must be updated in Partner Center first.".into() }]},
                NumberedPanel { number: 3, title: "Certificate setup".into(), subtitle: "Choose how you will sign your MSIX package.".into(), width: third, height: m.top_h, tone: if view.state().windows_pfx_path.is_empty() { PanelTone::Warning } else { PanelTone::Normal }, children: widgets![
                    RadioList { items: vec![("Use certificate by thumbprint".into(), view.state().windows_pfx_path.is_empty()), ("Select a PFX certificate file".into(), !view.state().windows_pfx_path.is_empty()), ("Generate test certificate".into(), false)]},
                    PublishTextField { id: "publish_windows_pfx", label: "PFX file".into(), value: view.state().windows_pfx_path.clone(), placeholder: "~/.fission/app/windows/signing.pfx".into(), on_change: pfx, secret: false, width: m.field_w },
                    PublishTextField { id: "publish_windows_password", label: "PFX password".into(), value: view.state().windows_password.clone(), placeholder: "stored locally".into(), on_change: password, secret: true, width: m.field_w },
                    ButtonRow { buttons: vec![PublishButton { label: "Select file...".into(), action: Some(browse), tone: ButtonTone::Primary, width: 120.0 }]},
                    InlineFilePicker { purpose: FilePurpose::WindowsCertificate, height: if board.layout.terminal { 14.0 } else { 220.0 } },
                    SelectedFileActions { purpose: FilePurpose::WindowsCertificate },
                    EnvVarList { title: "Supported environment variables".into(), names: vec!["WINDOWS_CERTIFICATE", "WINDOWS_CERTIFICATE_BASE64", "WINDOWS_CERTIFICATE_PASSWORD", "WINDOWS_CERTIFICATE_THUMBPRINT"] },
                ]},
            ],
            vec![
                NumberedPanel { number: 4, title: "Partner Center credentials".into(), subtitle: "Authenticate to Microsoft Partner Center.".into(), width: third, height: m.bottom_h, tone: PanelTone::Warning, children: widgets![GuideList { items: vec!["Go to Entra ID and register a confidential app".into(), "Grant Microsoft Store / Partner Center API permissions".into(), "Create a client secret".into(), "Capture tenant, client id, and secret".into()]}, PublishTextField { id: "publish_azure_tenant", label: "Tenant ID".into(), value: view.state().azure_tenant_id.clone(), placeholder: "tenant id".into(), on_change: tenant, secret: false, width: m.field_w }, PublishTextField { id: "publish_azure_client", label: "Client ID".into(), value: view.state().azure_client_id.clone(), placeholder: "client id".into(), on_change: client, secret: false, width: m.field_w }, PublishTextField { id: "publish_ms_secret", label: "Client secret".into(), value: view.state().microsoft_secret.clone(), placeholder: "secret".into(), on_change: secret, secret: true, width: m.field_w }, ButtonRow { buttons: vec![PublishButton { label: "Save credentials".into(), action: Some(save), tone: ButtonTone::Success, width: 145.0 }]}]},
                NumberedPanel { number: 5, title: "Release options".into(), subtitle: "Configure Store submission.".into(), width: third, height: m.bottom_h, tone: PanelTone::Normal, children: widgets![RadioList { items: vec![("Flight (Private)".into(), view.state().track == "private"), ("Production".into(), view.state().track == "production")]}, KeyValueList { rows: vec![("Submission type".into(), "Create draft submission".into(), StatusTone::Info), ("Locales".into(), empty_label(&view.state().locales_input, "en-US"), StatusTone::Info), ("Minimum OS".into(), "from package manifest".into(), StatusTone::Info)]}, ReadinessDigest { title: "Store readiness".into(), checks: view.state().distribution_checks.clone(), empty_detail: "Provider checks will appear after the snapshot loads.".into() }]},
                NumberedPanel { number: 6, title: "Build, sign, and validate".into(), subtitle: "Build and verify your MSIX package.".into(), width: third, height: m.bottom_h, tone: PanelTone::Normal, children: widgets![TaskStatusCard { kind: PublishTaskKind::Package, idle_detail: "Not built yet. Press Build MSIX to run the package pipeline.".into() }, ReadinessDigest { title: "Package readiness".into(), checks: view.state().package_checks.clone(), empty_detail: "Package checks have not produced a result yet.".into() }, ArtifactCard, ButtonRow { buttons: vec![PublishButton { label: "Build MSIX".into(), action: Some(package), tone: ButtonTone::Primary, width: 120.0 }]}]},
                NumberedPanel { number: 7, title: "Dry-run and submit".into(), subtitle: "Validate against Partner Center and submit.".into(), width: third, height: m.bottom_h, tone: if view.state().is_ready_to_publish() { PanelTone::Success } else { PanelTone::Warning }, children: widgets![PublishGateCard, PublishTextField { id: "publish_confirmation_windows", label: format!("Type app id to unlock: {}", view.state().app_id), value: view.state().publish_confirmation.clone(), placeholder: view.state().app_id.clone(), on_change: confirm, secret: false, width: m.field_w }, TaskStatusCard { kind: PublishTaskKind::DryRun, idle_detail: "Dry-run has not been executed in this session.".into() }, TaskStatusCard { kind: PublishTaskKind::Publish, idle_detail: "Submit is locked until checks pass and the app id is typed exactly.".into() }, ButtonRow { buttons: vec![PublishButton { label: "Run dry run".into(), action: Some(dry), tone: ButtonTone::Secondary, width: 120.0 }, PublishButton { label: if view.state().is_ready_to_publish() { "Upload package".into() } else { "Upload locked".into() }, action: if view.state().is_ready_to_publish() { Some(publish) } else { None }, tone: if view.state().is_ready_to_publish() { ButtonTone::Success } else { ButtonTone::Quiet }, width: 140.0 }]}]},
            ],
        ]}.into()
    }
}

#[derive(Clone)]
struct S3Board {
    layout: PublishLayout,
}

impl From<S3Board> for Widget {
    fn from(board: S3Board) -> Widget {
        let (ctx, view) = fission::build::current::<PublishUiState>();
        let m = metrics(board.layout);
        let profile = with_reducer!(
            ctx,
            PublishSetAwsProfile(String::new()),
            publish_set_aws_profile
        );
        let region = with_reducer!(
            ctx,
            PublishSetAwsRegion(String::new()),
            publish_set_aws_region
        );
        let endpoint = with_reducer!(
            ctx,
            PublishSetAwsEndpoint(String::new()),
            publish_set_aws_endpoint
        );
        let access = with_reducer!(
            ctx,
            PublishSetAwsAccessKey(String::new()),
            publish_set_aws_access_key
        );
        let secret = with_reducer!(
            ctx,
            PublishSetAwsSecretKey(String::new()),
            publish_set_aws_secret_key
        );
        let save = with_reducer!(ctx, PublishSaveCredentials, publish_save_credentials);
        let package = with_reducer!(
            ctx,
            PublishStartTask(PublishTaskKind::Package),
            publish_start_task
        );
        let dry = with_reducer!(
            ctx,
            PublishStartTask(PublishTaskKind::DryRun),
            publish_start_task
        );
        let publish = with_reducer!(
            ctx,
            PublishStartTask(PublishTaskKind::Publish),
            publish_start_task
        );
        let confirm = with_reducer!(
            ctx,
            PublishSetConfirmation(String::new()),
            publish_set_confirmation
        );
        BoardRows { rows: vec![
            vec![
                NumberedPanel { number: 1, title: "S3 Preflight".into(), subtitle: "Ensure the environment is ready to publish.".into(), width: m.col, height: m.top_h, tone: tone_for_checks(&view.state().distribution_checks), children: widgets![CheckList { checks: view.state().distribution_checks.clone(), limit: 8 }, Callout { tone: StatusTone::Info, text: "One item needing review blocks final upload until resolved.".into() }]},
                NumberedPanel { number: 2, title: "Credential Mode".into(), subtitle: "Choose how Fission authenticates to S3.".into(), width: m.col, height: m.top_h, tone: PanelTone::Normal, children: widgets![RadioList { items: vec![("AWS profile (recommended)".into(), !view.state().aws_profile.is_empty()), ("Environment access keys".into(), !view.state().aws_access_key_id.is_empty()), ("Web identity (OIDC)".into(), false), ("Custom S3-compatible endpoint".into(), !view.state().aws_endpoint.is_empty())]}, PublishTextField { id: "publish_aws_profile", label: "AWS profile".into(), value: view.state().aws_profile.clone(), placeholder: "default".into(), on_change: profile, secret: false, width: m.field_w }, EnvVarList { title: "Environment variables".into(), names: vec!["AWS_PROFILE", "AWS_ACCESS_KEY_ID", "AWS_SECRET_ACCESS_KEY", "AWS_WEB_IDENTITY_TOKEN_FILE", "AWS_REGION", "AWS_ENDPOINT_URL_S3"] }]},
                NumberedPanel { number: 3, title: "Target & Artifact".into(), subtitle: "What are you publishing?".into(), width: m.col, height: m.top_h, tone: PanelTone::Normal, children: widgets![RadioList { items: vec![("Static site (HTML/CSS/JS assets)".into(), view.state().target == Target::Site), ("Web bundle (WASM/JS)".into(), view.state().target == Target::Web), ("Linux run file".into(), false), ("Android AAB".into(), false), ("Arbitrary artifact manifest".into(), false)]}, ArtifactCard, ButtonRow { buttons: vec![PublishButton { label: "Build artifact".into(), action: Some(package.clone()), tone: ButtonTone::Primary, width: 135.0 }]}]},
                NumberedPanel { number: 4, title: "Bucket & Path Setup".into(), subtitle: "Where should artifacts be published?".into(), width: m.col, height: m.top_h, tone: PanelTone::Normal, children: widgets![PublishTextField { id: "publish_aws_region", label: "Region".into(), value: view.state().aws_region.clone(), placeholder: "eu-west-2".into(), on_change: region, secret: false, width: m.field_w }, PublishTextField { id: "publish_aws_endpoint", label: "Endpoint optional".into(), value: view.state().aws_endpoint.clone(), placeholder: "s3.eu-west-2.amazonaws.com".into(), on_change: endpoint, secret: false, width: m.field_w }, KeyValueList { rows: vec![("Prefix".into(), format!("releases/{}/", view.state().track), StatusTone::Info), ("Encryption".into(), "from publish manifest/provider".into(), StatusTone::Info)]}, ReadinessDigest { title: "S3 target readiness".into(), checks: view.state().distribution_checks.clone(), empty_detail: "Provider checks will appear after the snapshot loads.".into() }]},
            ],
            vec![
                NumberedPanel { number: 5, title: "Publication Options".into(), subtitle: "Configure how objects are published.".into(), width: m.col, height: m.bottom_h, tone: PanelTone::Warning, children: widgets![KeyValueList { rows: vec![("Public read access".into(), "depends on publish manifest".into(), StatusTone::Info), ("Cache-Control".into(), "from publish manifest/defaults".into(), StatusTone::Info), ("Pre-compression".into(), "from package artifact".into(), StatusTone::Info), ("Release receipt".into(), "written after successful publish".into(), StatusTone::Info)]}, PublishTextField { id: "publish_aws_access", label: "Access key".into(), value: view.state().aws_access_key_id.clone(), placeholder: "optional".into(), on_change: access, secret: false, width: m.field_w }, PublishTextField { id: "publish_aws_secret", label: "Secret key".into(), value: view.state().aws_secret_access_key.clone(), placeholder: "optional".into(), on_change: secret, secret: true, width: m.field_w }, ButtonRow { buttons: vec![PublishButton { label: "Save credentials".into(), action: Some(save), tone: ButtonTone::Success, width: 145.0 }]}, Callout { tone: StatusTone::Warning, text: "Deleting stale objects is destructive and requires explicit publish manifest support.".into() }]},
                NumberedPanel { number: 6, title: "Build and dry-run object plan".into(), subtitle: "Build artifacts, then ask the provider to produce a dry-run plan.".into(), width: m.span2, height: m.bottom_h, tone: PanelTone::Normal, children: widgets![TaskStatusCard { kind: PublishTaskKind::Package, idle_detail: "Not built yet. Press Build artifact from target setup or run it here first.".into() }, TaskStatusCard { kind: PublishTaskKind::DryRun, idle_detail: "Dry-run has not been executed in this session; no object counts are fabricated.".into() }, ArtifactCard, ButtonRow { buttons: vec![PublishButton { label: "Build artifact".into(), action: Some(package), tone: ButtonTone::Primary, width: 135.0 }, PublishButton { label: "Run dry run".into(), action: Some(dry), tone: ButtonTone::Secondary, width: 120.0 }]}]},
                NumberedPanel { number: 7, title: "Upload & Receipt".into(), subtitle: "Publishing to S3 produces a local receipt.".into(), width: m.col, height: m.bottom_h, tone: if view.state().is_ready_to_publish() { PanelTone::Success } else { PanelTone::Warning }, children: widgets![PublishGateCard, PublishTextField { id: "publish_confirmation_s3", label: format!("Type app id to unlock: {}", view.state().app_id), value: view.state().publish_confirmation.clone(), placeholder: view.state().app_id.clone(), on_change: confirm, secret: false, width: m.field_w }, TaskStatusCard { kind: PublishTaskKind::Publish, idle_detail: "Publish is locked until checks pass and the app id is typed exactly.".into() }, KeyValueList { rows: vec![("Bucket".into(), "from publish manifest".into(), StatusTone::Info), ("Prefix".into(), format!("releases/{}/", view.state().track), StatusTone::Info)]}, ButtonRow { buttons: vec![PublishButton { label: if view.state().is_ready_to_publish() { "Publish".into() } else { "Publish locked".into() }, action: if view.state().is_ready_to_publish() { Some(publish) } else { None }, tone: if view.state().is_ready_to_publish() { ButtonTone::Success } else { ButtonTone::Quiet }, width: 120.0 }]}]},
            ],
        ]}.into()
    }
}

#[derive(Clone, Copy)]
struct BoardMetrics {
    col: f32,
    span2: f32,
    full: f32,
    top_h: Option<f32>,
    mid_h: Option<f32>,
    bottom_h: Option<f32>,
    field_w: f32,
}

fn metrics(layout: PublishLayout) -> BoardMetrics {
    let col = layout.column_width;
    let span2 = col * 2.0 + layout.gap;
    let full = col * 4.0 + layout.gap * 3.0;
    if layout.terminal || layout.compact {
        BoardMetrics {
            col: layout.column_width,
            span2: layout.column_width,
            full: layout.column_width,
            top_h: None,
            mid_h: None,
            bottom_h: None,
            field_w: layout.column_width - 2.0,
        }
    } else {
        BoardMetrics {
            col,
            span2,
            full,
            top_h: Some((layout.body_height * 0.52).max(330.0)),
            mid_h: Some((layout.body_height * 0.45).max(300.0)),
            bottom_h: Some((layout.body_height * 0.42).max(280.0)),
            field_w: (col - 32.0).max(220.0),
        }
    }
}

fn tone_for_checks(checks: &[UiCheck]) -> PanelTone {
    if checks
        .iter()
        .any(|check| check.status == CheckStatus::Failed || check.status == CheckStatus::Missing)
    {
        PanelTone::Danger
    } else if checks
        .iter()
        .any(|check| check.status == CheckStatus::Warning)
    {
        PanelTone::Warning
    } else if checks
        .iter()
        .any(|check| check.status == CheckStatus::Passed)
    {
        PanelTone::Success
    } else {
        PanelTone::Normal
    }
}

fn empty_label(value: &str, empty: &str) -> String {
    if value.trim().is_empty() {
        empty.to_string()
    } else {
        value.to_string()
    }
}

fn workspace_label(state: &PublishUiState) -> String {
    if state.workspace.as_os_str().is_empty() {
        "~/.fission/<app-name>/".to_string()
    } else {
        state.workspace.display().to_string()
    }
}
