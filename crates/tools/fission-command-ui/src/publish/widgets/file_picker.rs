use super::panel::DividerLine;
use super::primitives::{ButtonRow, ButtonTone, Callout, PublishButton, StatusTone};
use super::*;

#[derive(Clone)]
pub(super) struct FilePickerPanel {
    pub(super) layout: PublishLayout,
}

impl From<FilePickerPanel> for Widget {
    fn from(panel: FilePickerPanel) -> Widget {
        let (ctx, view) = fission::build::current::<PublishUiState>();
        let palette = PublishPalette::for_mode(view.state().theme_mode);
        let Some(picker) = view.state().file_picker.clone() else {
            return Spacer::default().into();
        };
        let close = with_reducer!(ctx, PublishCloseFilePicker, publish_close_file_picker);
        let panel_width = if panel.layout.terminal {
            panel.layout.column_width
        } else {
            (view.env().viewport_size.width * 0.72).min(980.0)
        };
        let panel_height = if panel.layout.terminal {
            (view.env().viewport_size.height - 2.0).max(36.0)
        } else {
            (view.env().viewport_size.height * 0.76).min(760.0)
        };
        let list_height = (panel_height
            - if view.state().selected_file.is_some() {
                220.0
            } else {
                150.0
            })
        .max(if panel.layout.terminal { 16.0 } else { 280.0 });
        let mut entries = widgets![FileEntryButton {
            label: "../".into(),
            action_index: 0,
            is_dir: true,
            selected: picker.selected_index == 0,
        }];
        entries.extend(picker.entries.iter().enumerate().map(|(idx, entry)| {
            let action_index = idx + 1;
            FileEntryButton {
                label: if entry.is_dir {
                    format!("{}/", entry.label)
                } else {
                    entry.label.clone()
                },
                action_index,
                is_dir: entry.is_dir,
                selected: picker.selected_index == action_index,
            }
            .into()
        }));
        if let Some(error) = &picker.error {
            entries.push(
                Callout {
                    tone: StatusTone::Error,
                    text: format!("Cannot read directory: {error}"),
                }
                .into(),
            );
        } else if picker.entries.is_empty() {
            entries.push(
                Callout {
                    tone: StatusTone::Info,
                    text: "No files visible in this directory.".into(),
                }
                .into(),
            );
        }
        let mut children = widgets![
            Row {
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                children: widgets![
                    Column {
                        gap: Some(if panel.layout.terminal { 0.0 } else { 4.0 }),
                        children: widgets![
                            Text::new(picker.purpose.title()).size(if panel.layout.terminal { 14.0 } else { 22.0 }).color(palette.text),
                            Text::new("Choose, then reference, copy, or move the file into the local release workspace.").size(if panel.layout.terminal { 10.0 } else { 12.5 }).color(palette.muted),
                        ],
                        ..Default::default()
                    },
                    PublishButton { label: "Close".into(), action: Some(close), tone: ButtonTone::Quiet, width: if panel.layout.terminal { 12.0 } else { 80.0 } },
                ],
                ..Default::default()
            },
            DividerLine { color: palette.hairline },
            Text::new(picker.current_dir.display().to_string()).size(if panel.layout.terminal { 10.0 } else { 12.0 }).color(palette.accent),
            Scroll {
                direction: FlexDirection::Column,
                height: Some(list_height),
                child: Some(Column { gap: Some(if panel.layout.terminal { 0.0 } else { 5.0 }), children: entries, ..Default::default() }.into()),
                show_scrollbar: true,
                ..Default::default()
            },
        ];
        if picker.truncated {
            children.push(
                Callout {
                    tone: StatusTone::Warning,
                    text: "Showing the first 200 entries.".into(),
                }
                .into(),
            );
        }
        if let Some(selection) = &view.state().selected_file {
            let copy = with_reducer!(ctx, PublishApplyFile(FileAction::Copy), publish_apply_file);
            let mv = with_reducer!(ctx, PublishApplyFile(FileAction::Move), publish_apply_file);
            let reference = with_reducer!(
                ctx,
                PublishApplyFile(FileAction::Reference),
                publish_apply_file
            );
            children.push(
                DividerLine {
                    color: palette.hairline,
                }
                .into(),
            );
            children.push(
                Text::new(format!("Selected: {}", selection.path.display()))
                    .size(if panel.layout.terminal { 10.0 } else { 12.5 })
                    .color(palette.text)
                    .into(),
            );
            children.push(
                ButtonRow {
                    buttons: vec![
                        PublishButton {
                            label: "Reference path".into(),
                            action: Some(reference),
                            tone: ButtonTone::Quiet,
                            width: 150.0,
                        },
                        PublishButton {
                            label: "Copy to workspace".into(),
                            action: Some(copy),
                            tone: ButtonTone::Success,
                            width: 165.0,
                        },
                        PublishButton {
                            label: "Move to workspace".into(),
                            action: Some(mv),
                            tone: ButtonTone::Secondary,
                            width: 160.0,
                        },
                    ],
                }
                .into(),
            );
        }
        Container::new(Column {
            gap: Some(if panel.layout.terminal { 1.0 } else { 10.0 }),
            children,
            ..Default::default()
        })
        .width(panel_width)
        .height(panel_height)
        .padding([
            panel.layout.card_padding,
            panel.layout.card_padding,
            panel.layout.card_padding,
            panel.layout.card_padding,
        ])
        .bg(palette.panel)
        .border(
            palette.accent,
            if panel.layout.terminal { 0.0 } else { 1.0 },
        )
        .border_radius(panel.layout.panel_radius)
        .into()
    }
}

#[derive(Clone)]
pub(super) struct InlineFilePicker {
    pub(super) purpose: FilePurpose,
    pub(super) height: f32,
}

#[derive(Clone)]
pub(super) struct SelectedFileActions {
    pub(super) purpose: FilePurpose,
}

impl From<SelectedFileActions> for Widget {
    fn from(actions: SelectedFileActions) -> Widget {
        let (ctx, view) = fission::build::current::<PublishUiState>();
        let layout = PublishLayout::from_viewport(view.env().viewport_size);
        let palette = PublishPalette::for_mode(view.state().theme_mode);
        let Some(selection) = view
            .state()
            .selected_file
            .as_ref()
            .filter(|selection| selection.purpose == actions.purpose)
            .cloned()
        else {
            return Spacer::default().into();
        };
        let copy = with_reducer!(ctx, PublishApplyFile(FileAction::Copy), publish_apply_file);
        let mv = with_reducer!(ctx, PublishApplyFile(FileAction::Move), publish_apply_file);
        let reference = with_reducer!(
            ctx,
            PublishApplyFile(FileAction::Reference),
            publish_apply_file
        );
        Container::new(Column {
            gap: Some(if layout.terminal { 1.0 } else { 8.0 }),
            children: widgets![
                Text::new(format!("Selected: {}", selection.path.display()))
                    .size(if layout.terminal { 10.0 } else { 12.0 })
                    .color(palette.text),
                Text::new("Choose how Fission should use this sensitive file.")
                    .size(if layout.terminal { 10.0 } else { 11.5 })
                    .color(palette.muted),
                ButtonRow {
                    buttons: vec![
                        PublishButton {
                            label: "Reference path".into(),
                            action: Some(reference),
                            tone: ButtonTone::Quiet,
                            width: 135.0,
                        },
                        PublishButton {
                            label: "Copy to ~/.fission".into(),
                            action: Some(copy),
                            tone: ButtonTone::Success,
                            width: 165.0,
                        },
                        PublishButton {
                            label: "Move to ~/.fission".into(),
                            action: Some(mv),
                            tone: ButtonTone::Secondary,
                            width: 165.0,
                        },
                    ],
                },
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
        .border(palette.accent, if layout.terminal { 0.0 } else { 1.0 })
        .border_radius(layout.panel_radius * 0.7)
        .into()
    }
}

impl From<InlineFilePicker> for Widget {
    fn from(component: InlineFilePicker) -> Widget {
        let (ctx, view) = fission::build::current::<PublishUiState>();
        let layout = PublishLayout::from_viewport(view.env().viewport_size);
        let palette = PublishPalette::for_mode(view.state().theme_mode);
        if view.state().native_file_dialog {
            return Spacer::default().into();
        }
        let Some(picker) = view
            .state()
            .file_picker
            .as_ref()
            .filter(|picker| picker.purpose == component.purpose)
            .cloned()
        else {
            return Spacer::default().into();
        };
        let mut entries = widgets![FileEntryButton {
            label: "../".into(),
            action_index: 0,
            is_dir: true,
            selected: picker.selected_index == 0,
        }];
        entries.extend(
            picker
                .entries
                .iter()
                .take(80)
                .enumerate()
                .map(|(idx, entry)| {
                    let action_index = idx + 1;
                    FileEntryButton {
                        label: if entry.is_dir {
                            format!("{}/", entry.label)
                        } else {
                            entry.label.clone()
                        },
                        action_index,
                        is_dir: entry.is_dir,
                        selected: picker.selected_index == action_index,
                    }
                    .into()
                }),
        );
        if let Some(error) = &picker.error {
            entries.push(
                Callout {
                    tone: StatusTone::Error,
                    text: format!("Cannot read directory: {error}"),
                }
                .into(),
            );
        }
        let mut children = widgets![
            Text::new(format!("Select file  {}", picker.current_dir.display()))
                .size(if layout.terminal { 10.0 } else { 12.0 })
                .color(palette.muted),
            Scroll {
                direction: FlexDirection::Column,
                height: Some(
                    (component.height
                        - if view.state().selected_file.is_some() {
                            96.0
                        } else {
                            36.0
                        })
                    .max(120.0)
                ),
                child: Some(
                    Column {
                        gap: Some(if layout.terminal { 0.0 } else { 4.0 }),
                        children: entries,
                        ..Default::default()
                    }
                    .into()
                ),
                show_scrollbar: true,
                ..Default::default()
            },
        ];
        if let Some(selection) = &view.state().selected_file {
            let copy = with_reducer!(ctx, PublishApplyFile(FileAction::Copy), publish_apply_file);
            let mv = with_reducer!(ctx, PublishApplyFile(FileAction::Move), publish_apply_file);
            let reference = with_reducer!(
                ctx,
                PublishApplyFile(FileAction::Reference),
                publish_apply_file
            );
            children.push(
                Text::new(format!("Selected: {}", selection.path.display()))
                    .size(if layout.terminal { 10.0 } else { 11.5 })
                    .color(palette.text)
                    .into(),
            );
            children.push(
                ButtonRow {
                    buttons: vec![
                        PublishButton {
                            label: "Reference".into(),
                            action: Some(reference),
                            tone: ButtonTone::Quiet,
                            width: 110.0,
                        },
                        PublishButton {
                            label: "Copy to ~/.fission".into(),
                            action: Some(copy),
                            tone: ButtonTone::Success,
                            width: 160.0,
                        },
                        PublishButton {
                            label: "Move".into(),
                            action: Some(mv),
                            tone: ButtonTone::Secondary,
                            width: 92.0,
                        },
                    ],
                }
                .into(),
            );
        }
        Container::new(Column {
            gap: Some(if layout.terminal { 1.0 } else { 6.0 }),
            children,
            ..Default::default()
        })
        .height(component.height)
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
struct FileEntryButton {
    label: String,
    action_index: usize,
    is_dir: bool,
    selected: bool,
}

impl From<FileEntryButton> for Widget {
    fn from(entry: FileEntryButton) -> Widget {
        let (ctx, view) = fission::build::current::<PublishUiState>();
        let layout = PublishLayout::from_viewport(view.env().viewport_size);
        let palette = PublishPalette::for_mode(view.state().theme_mode);
        let action = with_reducer!(
            ctx,
            PublishPickFileEntry(entry.action_index),
            publish_pick_file_entry
        );
        if layout.terminal {
            return Text::new(if entry.selected {
                format!("* {}", entry.label)
            } else if entry.is_dir {
                format!("> {}", entry.label)
            } else {
                format!("  {}", entry.label)
            })
            .size(10.0)
            .color(if entry.selected {
                palette.warning
            } else if entry.is_dir {
                palette.accent
            } else {
                palette.text
            })
            .on_tap(action)
            .into();
        }
        let entry_width = if layout.terminal {
            (layout.column_width - 2.0).max(40.0)
        } else {
            (view.env().viewport_size.width
                - (layout.root_padding * 4.0)
                - (layout.card_padding * 4.0))
                .max(400.0)
        };
        let child = Container::new(
            Text::new(entry.label)
                .size(if layout.terminal { 10.0 } else { 12.0 })
                .color(palette.text),
        )
        .width(entry_width)
        .height(if layout.terminal { 2.0 } else { 30.0 })
        .padding([8.0, 8.0, 3.0, 3.0])
        .bg(if entry.selected {
            palette.background_alt
        } else if entry.is_dir {
            palette.input
        } else {
            palette.panel_soft
        })
        .border(
            if entry.selected {
                palette.accent
            } else {
                palette.hairline
            },
            if layout.terminal { 0.0 } else { 1.0 },
        )
        .border_radius(if layout.terminal { 1.0 } else { 6.0 });
        GestureDetector {
            child: child.into(),
            on_tap: Some(action),
            ..Default::default()
        }
        .into()
    }
}
