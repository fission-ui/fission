use super::panel::{DividerLine, NumberedPanel, PanelTone};
use super::primitives::{
    ButtonRow, ButtonTone, Callout, KeyValueList, PublishButton, PublishTextField, SplitBand,
    StatusTone,
};
use super::*;

#[derive(Clone)]
pub(super) struct FissionTomlEditorPanel {
    pub(super) layout: PublishLayout,
}

impl From<FissionTomlEditorPanel> for Widget {
    fn from(panel: FissionTomlEditorPanel) -> Widget {
        let (_ctx, view) = fission::build::current::<PublishUiState>();
        let palette = PublishPalette::for_mode(view.state().theme_mode);
        let Some(editor) = view.state().config_editor.clone() else {
            return Spacer::default().into();
        };
        let width = panel.layout.wizard_width(view.env().viewport_size.width);
        let child = NumberedPanel {
            number: view.state().current_step,
            title: "Configure fission.toml".into(),
            subtitle: "Edit app, target, package, distribution, and release-content fields in one place.".into(),
            width,
            height: None,
            tone: PanelTone::Normal,
            children: widgets![
                Callout {
                    tone: StatusTone::Info,
                    text: "Use this for non-secret project metadata. Credentials and machine-local secret file paths stay in release.env, environment variables, or CI secrets.".into(),
                },
                ConfigEditorBody { editor, layout: panel.layout },
            ],
        };
        Scroll {
            direction: FlexDirection::Column,
            height: Some(panel.layout.body_height),
            child: Some(
                Container::new(child)
                    .padding([0.0, 0.0, 0.0, if panel.layout.terminal { 0.0 } else { 4.0 }])
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
struct ConfigEditorBody {
    editor: FissionTomlEditorState,
    layout: PublishLayout,
}

impl From<ConfigEditorBody> for Widget {
    fn from(body: ConfigEditorBody) -> Widget {
        let (ctx, view) = fission::build::current::<PublishUiState>();
        let selected = field_specs()
            .iter()
            .find(|spec| spec.path == body.editor.field_path)
            .copied();
        let metadata_rows = selected
            .map(|spec| {
                vec![
                    (
                        "Group".to_string(),
                        spec.group.to_string(),
                        StatusTone::Info,
                    ),
                    (
                        "Expected value".to_string(),
                        value_kind_label(spec.kind).to_string(),
                        StatusTone::Info,
                    ),
                    (
                        "Example".to_string(),
                        spec.placeholder.to_string(),
                        StatusTone::Muted,
                    ),
                ]
            })
            .unwrap_or_else(|| {
                vec![
                    ("Group".to_string(), "Custom".to_string(), StatusTone::Info),
                    (
                        "Expected value".to_string(),
                        "TOML literal or plain string".to_string(),
                        StatusTone::Info,
                    ),
                    (
                        "Example".to_string(),
                        "true, 3, [\"en-US\"], or https://example.com".to_string(),
                        StatusTone::Muted,
                    ),
                ]
            });
        let field_path = with_reducer!(
            ctx.clone(),
            PublishSetConfigFieldPath,
            publish_set_config_field_path
        );
        let field_value = with_reducer!(
            ctx.clone(),
            PublishSetConfigFieldValue,
            publish_set_config_field_value
        );
        let apply = with_reducer!(
            ctx.clone(),
            PublishApplyConfigField,
            publish_apply_config_field
        );
        let close = with_reducer!(ctx, PublishCloseConfigEditor, publish_close_config_editor);
        let field_width = if body.layout.terminal {
            body.layout.column_width
        } else {
            (body.layout.wizard_width(view.env().viewport_size.width) * 0.44).min(520.0)
        };
        SplitBand {
            left: widgets![
                PublishTextField {
                    id: "publish_config_field_path",
                    label: "fission.toml field".into(),
                    value: body.editor.field_path.clone(),
                    placeholder: "app.homepage".into(),
                    on_change: field_path,
                    secret: false,
                    width: field_width,
                },
                PublishTextField {
                    id: "publish_config_field_value",
                    label: "Value".into(),
                    value: body.editor.value.clone(),
                    placeholder: selected
                        .map(|spec| spec.placeholder.to_string())
                        .unwrap_or_else(|| "TOML literal or plain text".to_string()),
                    on_change: field_value,
                    secret: false,
                    width: field_width,
                },
                KeyValueList { rows: metadata_rows },
                Callout {
                    tone: StatusTone::Warning,
                    text: "Apply writes fission.toml and refreshes readiness. It refuses obvious secrets and secret file paths.".into(),
                },
                ButtonRow {
                    buttons: vec![
                        PublishButton {
                            label: "Apply to fission.toml".into(),
                            action: Some(apply),
                            tone: ButtonTone::Success,
                            width: 170.0,
                        },
                        PublishButton {
                            label: "Close".into(),
                            action: Some(close),
                            tone: ButtonTone::Quiet,
                            width: 100.0,
                        },
                    ],
                },
                Callout {
                    tone: status_tone(&body.editor.status_message),
                    text: body.editor.status_message.clone(),
                },
            ],
            right: widgets![ConfigFieldPresetList {
                selected_path: body.editor.field_path.clone(),
                height: if body.layout.terminal {
                    20.0
                } else {
                    (body.layout.body_height - 160.0).max(360.0)
                },
            }],
        }
        .into()
    }
}

#[derive(Clone)]
struct ConfigFieldPresetList {
    selected_path: String,
    height: f32,
}

impl From<ConfigFieldPresetList> for Widget {
    fn from(list: ConfigFieldPresetList) -> Widget {
        let (_ctx, view) = fission::build::current::<PublishUiState>();
        let layout = PublishLayout::from_viewport(view.env().viewport_size);
        let palette = PublishPalette::for_mode(view.state().theme_mode);
        let mut rows = widgets![
            Text::new("Known fields")
                .size(if layout.terminal { 12.0 } else { 14.0 })
                .color(palette.text),
            Text::new("Choose a common field or type a custom dotted path.")
                .size(if layout.terminal { 10.0 } else { 11.5 })
                .color(palette.muted),
            DividerLine {
                color: palette.hairline,
            },
        ];
        rows.extend(field_specs().iter().map(|spec| {
            ConfigFieldPresetRow {
                spec: *spec,
                selected: spec.path == list.selected_path,
            }
            .into()
        }));
        Scroll {
            direction: FlexDirection::Column,
            height: Some(list.height),
            child: Some(
                Column {
                    gap: Some(if layout.terminal { 0.0 } else { 7.0 }),
                    children: rows,
                    ..Default::default()
                }
                .into(),
            ),
            show_scrollbar: true,
            ..Default::default()
        }
        .into()
    }
}

#[derive(Clone)]
struct ConfigFieldPresetRow {
    spec: ConfigFieldSpec,
    selected: bool,
}

impl From<ConfigFieldPresetRow> for Widget {
    fn from(row: ConfigFieldPresetRow) -> Widget {
        let (ctx, view) = fission::build::current::<PublishUiState>();
        let layout = PublishLayout::from_viewport(view.env().viewport_size);
        let palette = PublishPalette::for_mode(view.state().theme_mode);
        let select = with_reducer!(
            ctx,
            PublishSelectConfigField(row.spec.path.to_string()),
            publish_select_config_field
        );
        Container::new(Row {
            gap: Some(if layout.terminal { 1.0 } else { 8.0 }),
            align_items: AlignItems::Center,
            children: widgets![
                Column {
                    gap: Some(if layout.terminal { 0.0 } else { 2.0 }),
                    children: widgets![
                        Text::new(row.spec.label)
                            .size(if layout.terminal { 11.0 } else { 12.5 })
                            .color(if row.selected {
                                palette.accent
                            } else {
                                palette.text
                            }),
                        Text::new(row.spec.path)
                            .size(if layout.terminal { 9.0 } else { 10.5 })
                            .color(palette.muted),
                        Text::new(format!(
                            "{} - {}",
                            row.spec.group,
                            value_kind_label(row.spec.kind)
                        ))
                        .size(if layout.terminal { 9.0 } else { 10.5 })
                        .color(palette.subtle),
                    ],
                    ..Default::default()
                }
                .flex_grow(1.0),
                PublishButton {
                    label: if row.selected {
                        "Selected".into()
                    } else {
                        "Use".into()
                    },
                    action: Some(select),
                    tone: if row.selected {
                        ButtonTone::Success
                    } else {
                        ButtonTone::Secondary
                    },
                    width: if layout.terminal { 16.0 } else { 92.0 },
                },
            ],
            ..Default::default()
        })
        .padding([
            layout.card_padding * 0.55,
            layout.card_padding * 0.55,
            layout.card_padding * 0.45,
            layout.card_padding * 0.45,
        ])
        .bg(if row.selected {
            palette.panel_soft
        } else {
            palette.input
        })
        .border(
            if row.selected {
                palette.accent
            } else {
                palette.hairline
            },
            if layout.terminal { 0.0 } else { 1.0 },
        )
        .border_radius(layout.panel_radius * 0.65)
        .into()
    }
}

fn value_kind_label(kind: ConfigValueKind) -> &'static str {
    match kind {
        ConfigValueKind::String => "plain text",
        ConfigValueKind::Integer => "integer",
        ConfigValueKind::Bool => "true or false",
        ConfigValueKind::StringList => "comma-separated list",
        ConfigValueKind::TomlLiteral => "TOML literal",
    }
}

fn status_tone(message: &str) -> StatusTone {
    if message.starts_with("Updated ") {
        StatusTone::Ok
    } else if message.starts_with("Failed ") {
        StatusTone::Error
    } else {
        StatusTone::Info
    }
}
