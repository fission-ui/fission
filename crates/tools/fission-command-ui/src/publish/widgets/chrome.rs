use super::primitives::{ButtonTone, Callout, PublishButton, StatusTone, ToneMarker};
use super::*;

#[derive(Clone)]
pub(super) struct PublishHeader {
    pub(super) layout: PublishLayout,
}

impl From<PublishHeader> for Widget {
    fn from(header: PublishHeader) -> Widget {
        let (ctx, view) = fission::build::current::<PublishUiState>();
        let palette = PublishPalette::for_mode(view.state().theme_mode);
        let refresh = with_reducer!(ctx, PublishRefresh, publish_refresh);
        let theme = with_reducer!(ctx, PublishToggleTheme, publish_toggle_theme);
        let title = match view.state().board {
            PublishBoard::Android => "Fission Local Publish -- Android / Play Store",
            PublishBoard::Ios => "Fission Local Publish -- iOS / App Store Connect",
            PublishBoard::Windows => "Fission Local Publish -- Windows / Microsoft Store",
            PublishBoard::S3 => "Fission Local Publish -- S3 Artifact Publishing",
        };
        let subtitle = match view.state().board {
            PublishBoard::Android => "From local preflight to signed AAB and internal-track upload",
            PublishBoard::Ios => {
                "Archive, sign, validate, upload, and move toward TestFlight or App Store review"
            }
            PublishBoard::Windows => {
                "MSIX identity, certificate, Partner Center auth, flight submission"
            }
            PublishBoard::S3 => {
                "Publish static sites or release artifacts with a safe dry-run object plan"
            }
        };
        let board = format!(
            "Step {} of {}",
            view.state().current_step,
            view.state().board.step_count()
        );
        let command = format!(
            "fission publish --app --target {} --provider {}",
            view.state().target.as_str(),
            view.state().provider.as_str()
        );
        let header_body: Widget = if header.layout.terminal || header.layout.compact {
            Column {
                gap: Some(if header.layout.terminal { 0.0 } else { 8.0 }),
                children: widgets![
                    Text::new(board)
                        .size(if header.layout.terminal { 11.0 } else { 13.0 })
                        .color(palette.accent),
                    Text::new(title)
                        .size(if header.layout.terminal { 15.0 } else { 30.0 })
                        .color(palette.text),
                    Text::new(subtitle)
                        .size(if header.layout.terminal { 11.0 } else { 16.0 })
                        .color(palette.muted),
                    Row {
                        gap: Some(if header.layout.terminal { 1.0 } else { 8.0 }),
                        children: widgets![
                            CommandChip {
                                label: command.clone()
                            },
                            PublishButton {
                                label: "Refresh".into(),
                                action: Some(refresh.clone()),
                                tone: ButtonTone::Quiet,
                                width: if header.layout.terminal { 18.0 } else { 86.0 }
                            },
                            PublishButton {
                                label: "Theme".into(),
                                action: Some(theme.clone()),
                                tone: ButtonTone::Quiet,
                                width: if header.layout.terminal { 16.0 } else { 74.0 }
                            },
                        ],
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }
            .into()
        } else {
            Row {
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Start,
                children: widgets![
                    Column {
                        gap: Some(4.0),
                        children: widgets![
                            Text::new(board).size(13.0).color(palette.accent),
                            Text::new(title).size(31.0).color(palette.text),
                            Text::new(subtitle).size(17.0).color(palette.muted),
                        ],
                        ..Default::default()
                    },
                    Column {
                        gap: Some(8.0),
                        children: widgets![
                            LegendRow,
                            Row {
                                gap: Some(8.0),
                                children: widgets![
                                    CommandChip {
                                        label: "Terminal Mode  fission publish".into()
                                    },
                                    CommandChip { label: command },
                                    SecretBadge,
                                ],
                                ..Default::default()
                            },
                            Row {
                                gap: Some(8.0),
                                justify_content: JustifyContent::End,
                                children: widgets![
                                    PublishButton {
                                        label: "Refresh".into(),
                                        action: Some(refresh),
                                        tone: ButtonTone::Quiet,
                                        width: 86.0
                                    },
                                    PublishButton {
                                        label: "Theme".into(),
                                        action: Some(theme),
                                        tone: ButtonTone::Quiet,
                                        width: 74.0
                                    },
                                ],
                                ..Default::default()
                            },
                        ],
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }
            .into()
        };
        Container::new(header_body)
            .height(header.layout.header_height)
            .padding([0.0, 0.0, 0.0, 0.0])
            .bg(palette.background)
            .into()
    }
}

#[derive(Clone)]
struct LegendRow;

impl From<LegendRow> for Widget {
    fn from(_row: LegendRow) -> Widget {
        Row {
            gap: Some(18.0),
            justify_content: JustifyContent::End,
            children: widgets![
                LegendItem {
                    tone: StatusTone::Ok,
                    label: "OK".into()
                },
                LegendItem {
                    tone: StatusTone::Warning,
                    label: "Action required".into()
                },
                LegendItem {
                    tone: StatusTone::Error,
                    label: "Missing / Error".into()
                },
                LegendItem {
                    tone: StatusTone::Info,
                    label: "Info".into()
                },
            ],
            ..Default::default()
        }
        .into()
    }
}

#[derive(Clone)]
struct LegendItem {
    tone: StatusTone,
    label: String,
}

impl From<LegendItem> for Widget {
    fn from(item: LegendItem) -> Widget {
        let (_ctx, view) = fission::build::current::<PublishUiState>();
        let palette = PublishPalette::for_mode(view.state().theme_mode);
        Row {
            gap: Some(6.0),
            align_items: AlignItems::Center,
            children: widgets![
                ToneMarker { tone: item.tone },
                Text::new(item.label).size(12.5).color(palette.text),
            ],
            ..Default::default()
        }
        .into()
    }
}

#[derive(Clone)]
struct CommandChip {
    label: String,
}

impl From<CommandChip> for Widget {
    fn from(chip: CommandChip) -> Widget {
        let (_ctx, view) = fission::build::current::<PublishUiState>();
        let layout = PublishLayout::from_viewport(view.env().viewport_size);
        let palette = PublishPalette::for_mode(view.state().theme_mode);
        Container::new(
            Text::new(chip.label)
                .size(if layout.terminal { 10.0 } else { 12.0 })
                .color(palette.accent),
        )
        .padding([
            if layout.terminal { 1.0 } else { 13.0 },
            if layout.terminal { 1.0 } else { 13.0 },
            if layout.terminal { 0.0 } else { 8.0 },
            if layout.terminal { 0.0 } else { 8.0 },
        ])
        .bg(palette.input)
        .border(palette.hairline, if layout.terminal { 0.0 } else { 1.0 })
        .border_radius(if layout.terminal { 1.0 } else { 8.0 })
        .into()
    }
}

#[derive(Clone)]
struct SecretBadge;

impl From<SecretBadge> for Widget {
    fn from(_badge: SecretBadge) -> Widget {
        Callout {
            tone: StatusTone::Ok,
            text: "Secrets never written to fission.toml".into(),
        }
        .into()
    }
}

#[derive(Clone)]
pub(super) struct PublishFooter {
    pub(super) layout: PublishLayout,
}

impl From<PublishFooter> for Widget {
    fn from(footer: PublishFooter) -> Widget {
        let (_ctx, view) = fission::build::current::<PublishUiState>();
        let palette = PublishPalette::for_mode(view.state().theme_mode);
        let workspace = if view.state().workspace.as_os_str().is_empty() {
            "~/.fission/<app-name>/".to_string()
        } else {
            view.state().workspace.display().to_string()
        };
        let text = format!(
            "Local workspace  {}    |    Secrets never written to fission.toml    |    {}",
            workspace,
            view.state().status_message
        );
        Container::new(
            Text::new(text)
                .size(if footer.layout.terminal { 10.0 } else { 12.0 })
                .color(palette.muted),
        )
        .height(footer.layout.footer_height)
        .padding([
            if footer.layout.terminal { 0.0 } else { 12.0 },
            if footer.layout.terminal { 0.0 } else { 12.0 },
            if footer.layout.terminal { 0.0 } else { 8.0 },
            if footer.layout.terminal { 0.0 } else { 8.0 },
        ])
        .bg(palette.background_alt)
        .border(
            palette.hairline,
            if footer.layout.terminal { 0.0 } else { 1.0 },
        )
        .border_radius(if footer.layout.terminal { 0.0 } else { 8.0 })
        .into()
    }
}
