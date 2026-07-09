use super::panel::DividerLine;
use super::*;
use fission::icons::material;

#[derive(Clone, Copy)]
pub(super) enum StatusTone {
    Ok,
    Warning,
    Error,
    Info,
    Muted,
}

impl StatusTone {
    pub(super) fn color(self, palette: PublishPalette) -> Color {
        match self {
            StatusTone::Ok => palette.success,
            StatusTone::Warning => palette.warning,
            StatusTone::Error => palette.error,
            StatusTone::Info => palette.blue,
            StatusTone::Muted => palette.muted,
        }
    }

    pub(super) fn terminal_marker(self) -> &'static str {
        match self {
            StatusTone::Ok => "OK",
            StatusTone::Warning => "!",
            StatusTone::Error => "X",
            StatusTone::Info => "i",
            StatusTone::Muted => "-",
        }
    }

    fn icon(self) -> &'static str {
        match self {
            StatusTone::Ok => material::action::check_circle::regular(),
            StatusTone::Warning => material::alert::warning::round(),
            StatusTone::Error => material::alert::error::regular(),
            StatusTone::Info | StatusTone::Muted => material::action::info::regular(),
        }
    }
}

#[derive(Clone)]
pub(super) struct ToneMarker {
    pub(super) tone: StatusTone,
}

impl From<ToneMarker> for Widget {
    fn from(marker: ToneMarker) -> Widget {
        let (_ctx, view) = fission::build::current::<PublishUiState>();
        let layout = PublishLayout::from_viewport(view.env().viewport_size);
        let palette = PublishPalette::for_mode(view.state().theme_mode);
        let color = marker.tone.color(palette);
        if layout.terminal {
            return Text::new(marker.tone.terminal_marker())
                .size(10.0)
                .color(color)
                .into();
        }
        Icon::svg(marker.tone.icon())
            .size(layout.icon_size)
            .color(color)
            .into()
    }
}

#[derive(Clone)]
pub(super) struct CheckList {
    pub(super) checks: Vec<UiCheck>,
    pub(super) limit: usize,
}

impl From<CheckList> for Widget {
    fn from(list: CheckList) -> Widget {
        let (_ctx, view) = fission::build::current::<PublishUiState>();
        let layout = PublishLayout::from_viewport(view.env().viewport_size);
        let rows = if list.checks.is_empty() {
            widgets![InfoLine {
                tone: StatusTone::Info,
                label: "No checks reported yet".into(),
                detail: "Run refresh or continue to collect provider checks.".into(),
            }]
        } else {
            list.checks
                .into_iter()
                .take(list.limit)
                .map(|check| CheckListRow { check }.into())
                .collect()
        };
        Column {
            gap: Some(if layout.terminal { 0.0 } else { 6.0 }),
            children: rows,
            ..Default::default()
        }
        .into()
    }
}

#[derive(Clone)]
struct CheckListRow {
    check: UiCheck,
}

impl From<CheckListRow> for Widget {
    fn from(row: CheckListRow) -> Widget {
        let (_ctx, view) = fission::build::current::<PublishUiState>();
        let layout = PublishLayout::from_viewport(view.env().viewport_size);
        let palette = PublishPalette::for_mode(view.state().theme_mode);
        let tone = match row.check.status {
            CheckStatus::Passed => StatusTone::Ok,
            CheckStatus::Warning => StatusTone::Warning,
            CheckStatus::Missing | CheckStatus::Failed => StatusTone::Error,
            CheckStatus::Skipped => StatusTone::Muted,
        };
        let status = row.check.status;
        Row {
            gap: Some(if layout.terminal { 1.0 } else { 8.0 }),
            align_items: AlignItems::Start,
            children: widgets![
                ToneMarker { tone },
                Column {
                    gap: Some(if layout.terminal { 0.0 } else { 2.0 }),
                    children: check_text_widgets(row.check, layout, palette),
                    ..Default::default()
                }
                .flex_grow(1.0),
                Text::new(status_label(status))
                    .size(if layout.terminal { 11.0 } else { 12.0 })
                    .color(tone.color(palette)),
            ],
            ..Default::default()
        }
        .into()
    }
}

fn check_text_widgets(
    check: UiCheck,
    layout: PublishLayout,
    palette: PublishPalette,
) -> Vec<Widget> {
    let mut children = widgets![Text::new(check.summary)
        .size(if layout.terminal { 11.0 } else { 12.5 })
        .color(palette.text)];
    if let Some(details) = check.details.filter(|details| !details.trim().is_empty()) {
        children.push(
            Text::new(details)
                .size(if layout.terminal { 10.0 } else { 11.0 })
                .color(palette.muted)
                .into(),
        );
    }
    if matches!(check.status, CheckStatus::Missing | CheckStatus::Failed) {
        if let Some(remediation) = check.remediation.first() {
            children.push(
                Text::new(remediation.clone())
                    .size(if layout.terminal { 10.0 } else { 11.0 })
                    .color(palette.warning)
                    .into(),
            );
        }
    }
    children
}

#[derive(Clone)]
pub(super) struct InfoLine {
    pub(super) tone: StatusTone,
    pub(super) label: String,
    pub(super) detail: String,
}

impl From<InfoLine> for Widget {
    fn from(line: InfoLine) -> Widget {
        let (_ctx, view) = fission::build::current::<PublishUiState>();
        let layout = PublishLayout::from_viewport(view.env().viewport_size);
        let palette = PublishPalette::for_mode(view.state().theme_mode);
        Row {
            gap: Some(if layout.terminal { 1.0 } else { 8.0 }),
            align_items: AlignItems::Start,
            children: widgets![
                ToneMarker { tone: line.tone },
                Column {
                    gap: Some(1.0),
                    children: widgets![
                        Text::new(line.label)
                            .size(if layout.terminal { 11.0 } else { 12.5 })
                            .color(palette.text),
                        Text::new(line.detail)
                            .size(if layout.terminal { 10.0 } else { 11.5 })
                            .color(palette.muted),
                    ],
                    ..Default::default()
                }
                .flex_grow(1.0),
            ],
            ..Default::default()
        }
        .into()
    }
}

#[derive(Clone)]
pub(super) struct GuideList {
    pub(super) items: Vec<String>,
}

#[derive(Clone)]
pub(super) struct ReadinessDigest {
    pub(super) title: String,
    pub(super) checks: Vec<UiCheck>,
    pub(super) empty_detail: String,
}

impl From<ReadinessDigest> for Widget {
    fn from(digest: ReadinessDigest) -> Widget {
        let (_ctx, view) = fission::build::current::<PublishUiState>();
        let layout = PublishLayout::from_viewport(view.env().viewport_size);
        let palette = PublishPalette::for_mode(view.state().theme_mode);
        let counts = CheckCounts::from_checks(&digest.checks);
        let tone = if counts.blocking > 0 {
            StatusTone::Error
        } else if counts.warning > 0 {
            StatusTone::Warning
        } else if counts.passed > 0 {
            StatusTone::Ok
        } else {
            StatusTone::Info
        };
        let summary = if digest.checks.is_empty() {
            digest.empty_detail
        } else {
            format!(
                "{} passed, {} need action, {} warnings, {} skipped",
                counts.passed, counts.blocking, counts.warning, counts.skipped
            )
        };
        Container::new(Row {
            gap: Some(if layout.terminal { 1.0 } else { 9.0 }),
            align_items: AlignItems::Start,
            children: widgets![
                ToneMarker { tone },
                Column {
                    gap: Some(if layout.terminal { 0.0 } else { 3.0 }),
                    children: widgets![
                        Text::new(digest.title)
                            .size(if layout.terminal { 11.0 } else { 12.5 })
                            .color(palette.text),
                        Text::new(summary)
                            .size(if layout.terminal { 10.0 } else { 11.5 })
                            .color(palette.muted),
                    ],
                    ..Default::default()
                }
                .flex_grow(1.0),
            ],
            ..Default::default()
        })
        .padding([
            layout.card_padding * 0.7,
            layout.card_padding * 0.7,
            layout.card_padding * 0.55,
            layout.card_padding * 0.55,
        ])
        .bg(palette.input)
        .border(tone.color(palette), if layout.terminal { 0.0 } else { 1.0 })
        .border_radius(layout.panel_radius * 0.7)
        .into()
    }
}

#[derive(Default)]
struct CheckCounts {
    passed: usize,
    blocking: usize,
    warning: usize,
    skipped: usize,
}

impl CheckCounts {
    fn from_checks(checks: &[UiCheck]) -> Self {
        let mut counts = Self::default();
        for check in checks {
            match check.status {
                CheckStatus::Passed => counts.passed += 1,
                CheckStatus::Warning => counts.warning += 1,
                CheckStatus::Missing | CheckStatus::Failed => counts.blocking += 1,
                CheckStatus::Skipped => counts.skipped += 1,
            }
        }
        counts
    }
}

#[derive(Clone)]
pub(super) struct TaskStatusCard {
    pub(super) kind: PublishTaskKind,
    pub(super) idle_detail: String,
}

impl From<TaskStatusCard> for Widget {
    fn from(card: TaskStatusCard) -> Widget {
        let (_ctx, view) = fission::build::current::<PublishUiState>();
        let layout = PublishLayout::from_viewport(view.env().viewport_size);
        let palette = PublishPalette::for_mode(view.state().theme_mode);
        let matching_task = view
            .state()
            .task
            .as_ref()
            .filter(|task| task.kind == card.kind);
        let (tone, status, lines) = match matching_task {
            Some(task) => {
                let status = task.status();
                let tone = match status {
                    TaskStatus::Running => StatusTone::Info,
                    TaskStatus::Ok => StatusTone::Ok,
                    TaskStatus::Failed => StatusTone::Error,
                };
                (tone, status.label().to_string(), task.output())
            }
            None => (
                StatusTone::Muted,
                "not run".to_string(),
                vec![card.idle_detail],
            ),
        };
        let mut children = widgets![Row {
            gap: Some(if layout.terminal { 1.0 } else { 8.0 }),
            align_items: AlignItems::Center,
            children: widgets![
                ToneMarker { tone },
                Text::new(format!("{}: {}", card.kind.label(), status))
                    .size(if layout.terminal { 11.0 } else { 12.5 })
                    .color(palette.text),
            ],
            ..Default::default()
        }];
        children.extend(lines.into_iter().rev().take(4).rev().map(|line| {
            Text::new(line)
                .size(if layout.terminal { 10.0 } else { 11.0 })
                .color(palette.muted)
                .into()
        }));
        Container::new(Column {
            gap: Some(if layout.terminal { 0.0 } else { 4.0 }),
            children,
            ..Default::default()
        })
        .padding([
            layout.card_padding * 0.7,
            layout.card_padding * 0.7,
            layout.card_padding * 0.55,
            layout.card_padding * 0.55,
        ])
        .bg(palette.input)
        .border(tone.color(palette), if layout.terminal { 0.0 } else { 1.0 })
        .border_radius(layout.panel_radius * 0.7)
        .into()
    }
}

#[derive(Clone)]
pub(super) struct PublishGateCard;

impl From<PublishGateCard> for Widget {
    fn from(_card: PublishGateCard) -> Widget {
        let (_ctx, view) = fission::build::current::<PublishUiState>();
        let state = view.state();
        let package_ready = state.package_checks.iter().all(UiCheck::is_non_blocking);
        let provider_ready = state
            .distribution_checks
            .iter()
            .all(UiCheck::is_non_blocking);
        let confirmation_ready = state.publish_confirmation.trim() == state.app_id;
        KeyValueList {
            rows: vec![
                (
                    "Package checks".into(),
                    ready_label(package_ready),
                    if package_ready {
                        StatusTone::Ok
                    } else {
                        StatusTone::Error
                    },
                ),
                (
                    "Provider checks".into(),
                    ready_label(provider_ready),
                    if provider_ready {
                        StatusTone::Ok
                    } else {
                        StatusTone::Error
                    },
                ),
                (
                    "App id confirmation".into(),
                    ready_label(confirmation_ready),
                    if confirmation_ready {
                        StatusTone::Ok
                    } else {
                        StatusTone::Warning
                    },
                ),
            ],
        }
        .into()
    }
}

fn ready_label(ready: bool) -> String {
    if ready {
        "ready".to_string()
    } else {
        "blocked".to_string()
    }
}

impl From<GuideList> for Widget {
    fn from(list: GuideList) -> Widget {
        let (_ctx, view) = fission::build::current::<PublishUiState>();
        let layout = PublishLayout::from_viewport(view.env().viewport_size);
        let palette = PublishPalette::for_mode(view.state().theme_mode);
        Column {
            gap: Some(if layout.terminal { 0.0 } else { 7.0 }),
            children: list
                .items
                .into_iter()
                .enumerate()
                .map(|(idx, item)| {
                    Row {
                        gap: Some(if layout.terminal { 1.0 } else { 8.0 }),
                        children: widgets![
                            Container::new(
                                Text::new((idx + 1).to_string())
                                    .size(if layout.terminal { 10.0 } else { 11.0 })
                                    .color(palette.accent_text)
                            )
                            .width(if layout.terminal { 2.0 } else { 21.0 })
                            .height(if layout.terminal { 1.0 } else { 21.0 })
                            .bg(palette.accent)
                            .border_radius(999.0),
                            Text::new(item)
                                .size(if layout.terminal { 11.0 } else { 12.0 })
                                .color(palette.text)
                                .flex_grow(1.0),
                        ],
                        ..Default::default()
                    }
                    .into()
                })
                .collect(),
            ..Default::default()
        }
        .into()
    }
}

#[derive(Clone)]
pub(super) struct EnvVarList {
    pub(super) names: Vec<&'static str>,
    pub(super) title: String,
}

impl From<EnvVarList> for Widget {
    fn from(list: EnvVarList) -> Widget {
        let (_ctx, view) = fission::build::current::<PublishUiState>();
        let layout = PublishLayout::from_viewport(view.env().viewport_size);
        let palette = PublishPalette::for_mode(view.state().theme_mode);
        let mut children = widgets![Text::new(list.title)
            .size(if layout.terminal { 11.0 } else { 12.0 })
            .color(palette.blue)];
        children.extend(list.names.into_iter().map(|name| {
            Text::new(name)
                .size(if layout.terminal { 10.0 } else { 11.0 })
                .color(palette.success)
                .into()
        }));
        Container::new(Column {
            gap: Some(if layout.terminal { 0.0 } else { 3.0 }),
            children,
            ..Default::default()
        })
        .padding([
            layout.card_padding * 0.65,
            layout.card_padding * 0.65,
            layout.card_padding * 0.55,
            layout.card_padding * 0.55,
        ])
        .bg(palette.input)
        .border(palette.hairline, if layout.terminal { 0.0 } else { 1.0 })
        .border_radius(layout.panel_radius * 0.7)
        .into()
    }
}

#[derive(Clone)]
pub(super) struct KeyValueList {
    pub(super) rows: Vec<(String, String, StatusTone)>,
}

impl From<KeyValueList> for Widget {
    fn from(list: KeyValueList) -> Widget {
        let (_ctx, view) = fission::build::current::<PublishUiState>();
        let layout = PublishLayout::from_viewport(view.env().viewport_size);
        let palette = PublishPalette::for_mode(view.state().theme_mode);
        Column {
            gap: Some(if layout.terminal { 0.0 } else { 6.0 }),
            children: list
                .rows
                .into_iter()
                .map(|(label, value, tone)| {
                    Row {
                        gap: Some(if layout.terminal { 1.0 } else { 10.0 }),
                        children: widgets![
                            Text::new(label)
                                .size(if layout.terminal { 10.0 } else { 12.0 })
                                .color(palette.muted),
                            Text::new(value)
                                .size(if layout.terminal { 10.0 } else { 12.0 })
                                .color(tone.color(palette))
                                .flex_grow(1.0),
                        ],
                        ..Default::default()
                    }
                    .into()
                })
                .collect(),
            ..Default::default()
        }
        .into()
    }
}

#[derive(Clone)]
pub(super) struct Callout {
    pub(super) tone: StatusTone,
    pub(super) text: String,
}

impl From<Callout> for Widget {
    fn from(callout: Callout) -> Widget {
        let (_ctx, view) = fission::build::current::<PublishUiState>();
        let layout = PublishLayout::from_viewport(view.env().viewport_size);
        let palette = PublishPalette::for_mode(view.state().theme_mode);
        let color = callout.tone.color(palette);
        Container::new(Row {
            gap: Some(if layout.terminal { 1.0 } else { 8.0 }),
            children: widgets![
                ToneMarker { tone: callout.tone },
                Text::new(callout.text)
                    .size(if layout.terminal { 11.0 } else { 12.0 })
                    .color(color)
                    .flex_grow(1.0),
            ],
            ..Default::default()
        })
        .padding([
            layout.card_padding * 0.7,
            layout.card_padding * 0.7,
            layout.card_padding * 0.55,
            layout.card_padding * 0.55,
        ])
        .bg(palette.input)
        .border(color, if layout.terminal { 0.0 } else { 1.0 })
        .border_radius(layout.panel_radius * 0.7)
        .into()
    }
}

#[derive(Clone)]
pub(super) struct PublishTextField {
    pub(super) id: &'static str,
    pub(super) label: String,
    pub(super) value: String,
    pub(super) placeholder: String,
    pub(super) on_change: ActionEnvelope,
    pub(super) secret: bool,
    pub(super) width: f32,
}

impl From<PublishTextField> for Widget {
    fn from(field: PublishTextField) -> Widget {
        let (_ctx, view) = fission::build::current::<PublishUiState>();
        let layout = PublishLayout::from_viewport(view.env().viewport_size);
        let palette = PublishPalette::for_mode(view.state().theme_mode);
        TextInput {
            id: Some(WidgetId::explicit(field.id)),
            label: Some(field.label.into()),
            value: field.value,
            placeholder: Some(field.placeholder.into()),
            on_change: Some(field.on_change),
            width: Some(if layout.terminal {
                layout.column_width - 2.0
            } else {
                field.width
            }),
            height: Some(layout.input_height),
            padding: Some([8.0, 8.0, 4.0, 4.0]),
            font_size: Some(if layout.terminal { 12.0 } else { 13.0 }),
            obscure_text: field.secret,
            background_fill: Some(Fill::Solid(palette.input)),
            border_color: Some(palette.hairline),
            focus_border_color: Some(palette.accent),
            text_color: Some(palette.text),
            label_color: Some(palette.muted),
            placeholder_color: Some(palette.subtle),
            border_radius: Some(layout.control_radius),
            ..Default::default()
        }
        .into()
    }
}

#[derive(Clone)]
pub(super) struct PublishButton {
    pub(super) label: String,
    pub(super) action: Option<ActionEnvelope>,
    pub(super) tone: ButtonTone,
    pub(super) width: f32,
}

#[derive(Clone, Copy)]
pub(super) enum ButtonTone {
    Primary,
    Secondary,
    Quiet,
    Success,
}

impl From<PublishButton> for Widget {
    fn from(button: PublishButton) -> Widget {
        let (_ctx, view) = fission::build::current::<PublishUiState>();
        let layout = PublishLayout::from_viewport(view.env().viewport_size);
        let palette = PublishPalette::for_mode(view.state().theme_mode);
        let fill = match button.tone {
            ButtonTone::Primary => palette.blue,
            ButtonTone::Secondary => palette.panel_soft,
            ButtonTone::Quiet => palette.input,
            ButtonTone::Success => palette.success,
        };
        let text_color = if matches!(button.tone, ButtonTone::Success) {
            palette.accent_text
        } else {
            palette.text
        };
        if layout.terminal {
            let mut text = Text::new(format!("[ {} ]", button.label)).size(11.0).color(
                if button.action.is_some() {
                    text_color
                } else {
                    palette.subtle
                },
            );
            if let Some(action) = button.action {
                text = text.on_tap(action);
            }
            return text.into();
        }
        let width = if layout.terminal {
            (layout.column_width - 2.0).min(button.width)
        } else {
            button.width
        };
        let child = Container::new(
            Text::new(button.label)
                .size(if layout.terminal { 11.0 } else { 12.5 })
                .color(text_color),
        )
        .width(width)
        .height(layout.button_height)
        .padding([9.0, 9.0, 4.0, 4.0])
        .bg(fill)
        .border(palette.hairline, if layout.terminal { 0.0 } else { 1.0 })
        .border_radius(layout.control_radius);
        GestureDetector {
            child: child.into(),
            on_tap: button.action,
            ..Default::default()
        }
        .into()
    }
}

#[derive(Clone)]
pub(super) struct ButtonRow {
    pub(super) buttons: Vec<PublishButton>,
}

impl From<ButtonRow> for Widget {
    fn from(row: ButtonRow) -> Widget {
        let (_ctx, view) = fission::build::current::<PublishUiState>();
        let layout = PublishLayout::from_viewport(view.env().viewport_size);
        if layout.terminal {
            Column {
                gap: Some(1.0),
                children: row.buttons.into_iter().map(Widget::from).collect(),
                ..Default::default()
            }
            .into()
        } else {
            Row {
                gap: Some(8.0),
                children: row.buttons.into_iter().map(Widget::from).collect(),
                ..Default::default()
            }
            .into()
        }
    }
}

#[derive(Clone)]
pub(super) struct RadioList {
    pub(super) items: Vec<(String, bool)>,
}

impl From<RadioList> for Widget {
    fn from(list: RadioList) -> Widget {
        let (_ctx, view) = fission::build::current::<PublishUiState>();
        let layout = PublishLayout::from_viewport(view.env().viewport_size);
        let palette = PublishPalette::for_mode(view.state().theme_mode);
        Column {
            gap: Some(if layout.terminal { 0.0 } else { 5.0 }),
            children: list
                .items
                .into_iter()
                .map(|(label, active)| {
                    let marker = if active { "◉" } else { "○" };
                    Row {
                        gap: Some(if layout.terminal { 1.0 } else { 7.0 }),
                        children: widgets![
                            Text::new(marker).color(if active {
                                palette.blue
                            } else {
                                palette.muted
                            }),
                            Text::new(label)
                                .size(if layout.terminal { 11.0 } else { 12.0 })
                                .color(palette.text),
                        ],
                        ..Default::default()
                    }
                    .into()
                })
                .collect(),
            ..Default::default()
        }
        .into()
    }
}

#[derive(Clone)]
pub(super) struct ProgressTimeline {
    pub(super) rows: Vec<(String, String, StatusTone)>,
}

impl From<ProgressTimeline> for Widget {
    fn from(timeline: ProgressTimeline) -> Widget {
        let (_ctx, view) = fission::build::current::<PublishUiState>();
        let layout = PublishLayout::from_viewport(view.env().viewport_size);
        let palette = PublishPalette::for_mode(view.state().theme_mode);
        Column {
            gap: Some(if layout.terminal { 0.0 } else { 6.0 }),
            children: timeline
                .rows
                .into_iter()
                .map(|(label, time, tone)| {
                    Row {
                        gap: Some(if layout.terminal { 1.0 } else { 8.0 }),
                        children: widgets![
                            ToneMarker { tone },
                            Text::new(label)
                                .size(if layout.terminal { 10.0 } else { 12.0 })
                                .color(palette.text)
                                .flex_grow(1.0),
                            Text::new(time)
                                .size(if layout.terminal { 10.0 } else { 11.0 })
                                .color(palette.muted),
                        ],
                        ..Default::default()
                    }
                    .into()
                })
                .collect(),
            ..Default::default()
        }
        .into()
    }
}

#[derive(Clone)]
pub(super) struct ArtifactCard;

impl From<ArtifactCard> for Widget {
    fn from(_card: ArtifactCard) -> Widget {
        let (_ctx, view) = fission::build::current::<PublishUiState>();
        let layout = PublishLayout::from_viewport(view.env().viewport_size);
        let palette = PublishPalette::for_mode(view.state().theme_mode);
        let artifact = if view.state().artifact_manifest.as_os_str().is_empty() {
            "target/fission/release/artifact-manifest.json".to_string()
        } else {
            view.state().artifact_manifest.display().to_string()
        };
        Container::new(Column {
            gap: Some(if layout.terminal { 0.0 } else { 6.0 }),
            children: widgets![
                Text::new("Artifact")
                    .size(if layout.terminal { 11.0 } else { 12.0 })
                    .color(palette.muted),
                Text::new(artifact)
                    .size(if layout.terminal { 10.0 } else { 12.0 })
                    .color(palette.text),
                Text::new("SHA-256 will be recorded for the package artifact only.")
                    .size(if layout.terminal { 10.0 } else { 11.0 })
                    .color(palette.subtle),
            ],
            ..Default::default()
        })
        .padding([
            layout.card_padding * 0.7,
            layout.card_padding * 0.7,
            layout.card_padding * 0.6,
            layout.card_padding * 0.6,
        ])
        .bg(palette.input)
        .border(palette.hairline, if layout.terminal { 0.0 } else { 1.0 })
        .border_radius(layout.panel_radius * 0.7)
        .into()
    }
}

#[derive(Clone)]
pub(super) struct TaskLogSnippet;

impl From<TaskLogSnippet> for Widget {
    fn from(_snippet: TaskLogSnippet) -> Widget {
        let (_ctx, view) = fission::build::current::<PublishUiState>();
        let palette = PublishPalette::for_mode(view.state().theme_mode);
        let layout = PublishLayout::from_viewport(view.env().viewport_size);
        let mut lines = view
            .state()
            .task_log
            .iter()
            .rev()
            .take(5)
            .cloned()
            .collect::<Vec<_>>();
        if lines.is_empty() {
            lines = vec![
                view.state().status_message.clone(),
                "Build logs and provider responses appear here.".to_string(),
            ];
        }
        lines.reverse();
        Container::new(Column {
            gap: Some(1.0),
            children: lines
                .into_iter()
                .map(|line| {
                    Text::new(line)
                        .size(if layout.terminal { 10.0 } else { 11.0 })
                        .color(palette.muted)
                        .into()
                })
                .collect(),
            ..Default::default()
        })
        .padding([
            layout.card_padding * 0.7,
            layout.card_padding * 0.7,
            layout.card_padding * 0.55,
            layout.card_padding * 0.55,
        ])
        .bg(palette.input)
        .border(palette.hairline, if layout.terminal { 0.0 } else { 1.0 })
        .border_radius(layout.panel_radius * 0.7)
        .into()
    }
}

#[derive(Clone)]
pub(super) struct SplitBand {
    pub(super) left: Vec<Widget>,
    pub(super) right: Vec<Widget>,
}

impl From<SplitBand> for Widget {
    fn from(band: SplitBand) -> Widget {
        let (_ctx, view) = fission::build::current::<PublishUiState>();
        let layout = PublishLayout::from_viewport(view.env().viewport_size);
        if layout.terminal {
            let mut children = band.left;
            children.push(
                DividerLine {
                    color: PublishPalette::for_mode(view.state().theme_mode).hairline,
                }
                .into(),
            );
            children.extend(band.right);
            Column {
                gap: Some(1.0),
                children,
                ..Default::default()
            }
            .into()
        } else {
            Row {
                gap: Some(12.0),
                align_items: AlignItems::Start,
                children: widgets![
                    Column {
                        gap: Some(8.0),
                        children: band.left,
                        ..Default::default()
                    }
                    .flex_grow(1.0),
                    Column {
                        gap: Some(8.0),
                        children: band.right,
                        ..Default::default()
                    }
                    .flex_grow(1.0),
                ],
                ..Default::default()
            }
            .into()
        }
    }
}

pub(super) fn status_label(status: CheckStatus) -> &'static str {
    match status {
        CheckStatus::Passed => "OK",
        CheckStatus::Warning => "Review",
        CheckStatus::Missing => "Missing",
        CheckStatus::Failed => "Error",
        CheckStatus::Skipped => "Skip",
    }
}
