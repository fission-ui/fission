use crate::model::{
    InboxState, LabelDropped, SetAutoAdvanceEnabled, SetDensity, SetDensitySelectOpen,
    SetDragInProgress, SetInboxType, SetInboxTypeSelectOpen, SetLocale, SetOfflineEnabled,
    SetQuickTipOpen, SetSettingsOpen, SetSignature, SetSignatureEditing, SetSmartComposeEnabled,
    SetTheme, SetThemeSelectOpen, SetZoomLevel,
};
use fission_core::op::{Color, GridTrack};
use fission_core::ui::widgets::{Clip, GestureDetector, Transform};
use fission_core::ui::{
    Button, ButtonVariant, Container, Grid, GridItem, Node, Positioned, Scroll, Text, TextContent,
    ZStack,
};
use fission_core::{
    ActionEnvelope, ActionId, BuildCtx, FlexDirection, Handler, View, Widget, WidgetNodeId,
};
use fission_i18n::Locale;
use fission_icons::material;
use fission_widgets::{
    Badge, Card, Divider, DragTarget, Draggable, Editable, FormControl, HStack, Icon, Modal,
    ModalAction, NumberInput, SegmentedControl, Select, SelectItem, Slider, Switch, Tag, VStack,
    Wrap,
};
use serde_json;
use std::sync::Arc;

pub struct SettingsModal;

fn theme_preview(
    ctx: &mut BuildCtx<InboxState>,
    view: &View<InboxState>,
    theme_id: ActionId,
    label: &str,
    bg: Color,
    accent: Color,
    is_active: bool,
) -> Node {
    let t = |key: &str| {
        view.env
            .i18n
            .get(&view.env.locale, key)
            .map(|s| s.to_string())
            .unwrap_or_else(|| key.to_string())
    };
    let angle = 0.18_f32;
    let (sin, cos) = angle.sin_cos();
    let rotate = [
        cos, -sin, 0.0, 0.0, sin, cos, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ];

    let badge = if is_active {
        Some(
            Badge {
                text: t("settings.theme.active"),
                ..Default::default()
            }
            .build(ctx, view),
        )
    } else {
        None
    };

    GestureDetector {
        on_tap: Some(ActionEnvelope {
            id: theme_id,
            payload: serde_json::to_vec(&SetTheme(label.to_string())).unwrap(),
        }),
        child: Box::new(
            Clip {
                id: None,
                path: Some("inset(0px round 12px)".into()),
                child: Box::new(
                    Container::new(
                        ZStack {
                            children: vec![
                                Container::new(
                                    fission_core::ui::widgets::Spacer::default().into_node(),
                                )
                                .size(160.0, 96.0)
                                .bg(bg)
                                .into_node(),
                                Positioned {
                                    top: Some(8.0),
                                    right: Some(8.0),
                                    child: badge.map(Box::new),
                                    ..Default::default()
                                }
                                .into_node(),
                                Positioned {
                                    left: Some(10.0),
                                    bottom: Some(10.0),
                                    child: Some(Box::new(
                                        Transform::new(
                                            Icon::svg(material::action::check_circle::regular())
                                                .size(18.0)
                                                .color(accent)
                                                .into_node(),
                                            rotate,
                                        )
                                        .into_node(),
                                    )),
                                    ..Default::default()
                                }
                                .into_node(),
                            ],
                            ..Default::default()
                        }
                        .into_node(),
                    )
                    .size(160.0, 96.0)
                    .into_node(),
                ),
            }
            .into_node(),
        ),
        ..Default::default()
    }
    .into_node()
}

impl Widget<InboxState> for SettingsModal {
    fn build(&self, ctx: &mut BuildCtx<InboxState>, view: &View<InboxState>) -> Node {
        let tokens = &view.env.theme.tokens;
        let viewport = view.viewport_size();
        let modal_width = (viewport.width.max(0.0) - 48.0).clamp(360.0, 640.0);
        let body_height = (viewport.height.max(0.0) - 180.0).clamp(320.0, 560.0);
        let t = |key: &str| {
            view.env
                .i18n
                .get(&view.env.locale, key)
                .map(|s| s.to_string())
                .unwrap_or_else(|| key.to_string())
        };
        let locale_id = ctx
            .bind(
                SetLocale(Locale("".into())),
                (|s: &mut InboxState, a: SetLocale, _| s.locale = a.0)
                    as Handler<InboxState, SetLocale>,
            )
            .id;
        let theme_id = ctx
            .bind(
                SetTheme("".into()),
                (|s: &mut InboxState, a: SetTheme, _| {
                    s.theme_mode = a.0;
                    s.show_theme_select = false;
                }) as Handler<InboxState, SetTheme>,
            )
            .id;
        let density_id = ctx
            .bind(
                SetDensity("".into()),
                (|s: &mut InboxState, a: SetDensity, _| {
                    s.density_mode = a.0;
                    s.show_density_select = false;
                }) as Handler<InboxState, SetDensity>,
            )
            .id;
        let inbox_type_id = ctx
            .bind(
                SetInboxType("".into()),
                (|s: &mut InboxState, a: SetInboxType, _| {
                    s.inbox_type = a.0;
                    s.show_inbox_type_select = false;
                }) as Handler<InboxState, SetInboxType>,
            )
            .id;
        let zoom_id = ctx
            .bind(
                SetZoomLevel(1.0),
                (|s: &mut InboxState, a: SetZoomLevel, _| s.zoom_level = a.0)
                    as Handler<InboxState, SetZoomLevel>,
            )
            .id;
        let signature_id = ctx
            .bind(
                SetSignature("".into()),
                (|s: &mut InboxState, a: SetSignature, _| s.signature = a.0)
                    as Handler<InboxState, SetSignature>,
            )
            .id;
        let signature_edit_id = ctx
            .bind(
                SetSignatureEditing(false),
                (|s: &mut InboxState, a: SetSignatureEditing, _| s.signature_editing = a.0)
                    as Handler<InboxState, SetSignatureEditing>,
            )
            .id;
        let smart_compose_id = ctx
            .bind(
                SetSmartComposeEnabled(false),
                (|s: &mut InboxState, a: SetSmartComposeEnabled, _| s.smart_compose_enabled = a.0)
                    as Handler<InboxState, SetSmartComposeEnabled>,
            )
            .id;
        let offline_id = ctx
            .bind(
                SetOfflineEnabled(false),
                (|s: &mut InboxState, a: SetOfflineEnabled, _| s.offline_enabled = a.0)
                    as Handler<InboxState, SetOfflineEnabled>,
            )
            .id;
        let auto_advance_id = ctx
            .bind(
                SetAutoAdvanceEnabled(false),
                (|s: &mut InboxState, a: SetAutoAdvanceEnabled, _| s.auto_advance_enabled = a.0)
                    as Handler<InboxState, SetAutoAdvanceEnabled>,
            )
            .id;
        let label_drop_id = ctx
            .bind(
                LabelDropped("".into()),
                (|s: &mut InboxState, a: LabelDropped, ctx| {
                    let mut label = None;
                    if let Some(payload) = ctx.input.as_internal_drop() {
                        if let Ok(parsed) = String::from_utf8(payload.to_vec()) {
                            label = Some(parsed);
                        }
                    }
                    s.last_drag_label = label.or_else(|| Some(a.0));
                    s.drag_in_progress = false;
                }) as Handler<InboxState, LabelDropped>,
            )
            .id;
        let inbox_type_open_id = ctx
            .bind(
                SetInboxTypeSelectOpen(false),
                (|s: &mut InboxState, a: SetInboxTypeSelectOpen, _| {
                    s.show_inbox_type_select = a.0;
                    if a.0 {
                        s.show_theme_select = false;
                        s.show_density_select = false;
                    }
                }) as Handler<InboxState, SetInboxTypeSelectOpen>,
            )
            .id;
        let theme_open_id = ctx
            .bind(
                SetThemeSelectOpen(false),
                (|s: &mut InboxState, a: SetThemeSelectOpen, _| {
                    s.show_theme_select = a.0;
                    if a.0 {
                        s.show_inbox_type_select = false;
                        s.show_density_select = false;
                    }
                }) as Handler<InboxState, SetThemeSelectOpen>,
            )
            .id;
        let density_open_id = ctx
            .bind(
                SetDensitySelectOpen(false),
                (|s: &mut InboxState, a: SetDensitySelectOpen, _| {
                    s.show_density_select = a.0;
                    if a.0 {
                        s.show_inbox_type_select = false;
                        s.show_theme_select = false;
                    }
                }) as Handler<InboxState, SetDensitySelectOpen>,
            )
            .id;
        let drag_active_id = ctx
            .bind(
                SetDragInProgress(false),
                (|s: &mut InboxState, a: SetDragInProgress, _| s.drag_in_progress = a.0)
                    as Handler<InboxState, SetDragInProgress>,
            )
            .id;
        let tip_id = ctx
            .bind(
                SetQuickTipOpen(true),
                (|s: &mut InboxState, a: SetQuickTipOpen, _| s.show_quick_tip = a.0)
                    as Handler<InboxState, SetQuickTipOpen>,
            )
            .id;

        let theme_items = vec![
            SelectItem {
                label: t("settings.theme.light"),
                icon: None,
                on_select: ActionEnvelope {
                    id: theme_id,
                    payload: serde_json::to_vec(&SetTheme("Light".into())).unwrap(),
                },
            },
            SelectItem {
                label: t("settings.theme.dark"),
                icon: None,
                on_select: ActionEnvelope {
                    id: theme_id,
                    payload: serde_json::to_vec(&SetTheme("Dark".into())).unwrap(),
                },
            },
            SelectItem {
                label: t("settings.theme.system"),
                icon: None,
                on_select: ActionEnvelope {
                    id: theme_id,
                    payload: serde_json::to_vec(&SetTheme("System".into())).unwrap(),
                },
            },
        ];

        let density_items = vec![
            SelectItem {
                label: t("settings.density.comfortable"),
                icon: None,
                on_select: ActionEnvelope {
                    id: density_id,
                    payload: serde_json::to_vec(&SetDensity("Comfortable".into())).unwrap(),
                },
            },
            SelectItem {
                label: t("settings.density.compact"),
                icon: None,
                on_select: ActionEnvelope {
                    id: density_id,
                    payload: serde_json::to_vec(&SetDensity("Compact".into())).unwrap(),
                },
            },
            SelectItem {
                label: t("settings.density.cozy"),
                icon: None,
                on_select: ActionEnvelope {
                    id: density_id,
                    payload: serde_json::to_vec(&SetDensity("Cozy".into())).unwrap(),
                },
            },
        ];

        let draggable_labels = ["Work", "Personal", "Travel", "Receipts", "Updates"]
            .iter()
            .map(|label| {
                Draggable {
                    payload: label.as_bytes().to_vec(),
                    on_drag_start: Some(ActionEnvelope {
                        id: drag_active_id,
                        payload: serde_json::to_vec(&SetDragInProgress(true)).unwrap(),
                    }),
                    on_drag_end: Some(ActionEnvelope {
                        id: drag_active_id,
                        payload: serde_json::to_vec(&SetDragInProgress(false)).unwrap(),
                    }),
                    child: Box::new(
                        Tag {
                            label: (*label).into(),
                            on_close: None,
                        }
                        .build(ctx, view),
                    ),
                }
                .build(ctx, view)
            })
            .collect::<Vec<_>>();

        let drop_target = DragTarget {
            on_drop: Some(ActionEnvelope {
                id: label_drop_id,
                payload: serde_json::to_vec(&LabelDropped("Pinned".into())).unwrap(),
            }),
            child: Box::new(
                Container::new(
                    Text::new(TextContent::Key("settings.labels.drop_target".into()))
                        .size(12.0)
                        .into_node(),
                )
                .padding_all(8.0)
                .bg(if view.state.drag_in_progress {
                    tokens.colors.primary.with_alpha(20)
                } else {
                    tokens.colors.background.with_alpha(0)
                })
                .border(tokens.colors.border, 1.0)
                .border_radius(8.0)
                .into_node(),
            ),
        }
        .build(ctx, view);

        let pinned_badge = if let Some(label) = &view.state.last_drag_label {
            Badge {
                text: format!("Pinned: {}", label),
                ..Default::default()
            }
            .build(ctx, view)
        } else {
            Text::new(TextContent::Key("settings.labels.helper".into()))
                .size(12.0)
                .color(tokens.colors.text_secondary)
                .into_node()
        };

        let signature_editor = Editable {
            id: Some(WidgetNodeId::explicit("settings_signature_editor")),
            value: view.state.signature.clone(),
            placeholder: "Add a signature".into(),
            is_editing: view.state.signature_editing,
            on_change: Some(ActionEnvelope {
                id: signature_id,
                payload: serde_json::to_vec(&SetSignature(view.state.signature.clone())).unwrap(),
            }),
            on_submit: Some(ActionEnvelope {
                id: signature_edit_id,
                payload: serde_json::to_vec(&SetSignatureEditing(false)).unwrap(),
            }),
            on_edit: Some(ActionEnvelope {
                id: signature_edit_id,
                payload: serde_json::to_vec(&SetSignatureEditing(true)).unwrap(),
            }),
            on_cancel: Some(ActionEnvelope {
                id: signature_edit_id,
                payload: serde_json::to_vec(&SetSignatureEditing(false)).unwrap(),
            }),
        }
        .build(ctx, view);

        let theme_display = match view.state.theme_mode.as_str() {
            "Dark" => t("settings.theme.dark"),
            "System" => t("settings.theme.system"),
            _ => t("settings.theme.light"),
        };
        let density_display = match view.state.density_mode.as_str() {
            "Compact" => t("settings.density.compact"),
            "Cozy" => t("settings.density.cozy"),
            _ => t("settings.density.comfortable"),
        };
        let inbox_type_display = match view.state.inbox_type.as_str() {
            "Priority Inbox" => t("settings.inbox_type.priority"),
            _ => t("settings.inbox_type.default"),
        };

        Modal {
            id: WidgetNodeId::explicit("settings_modal"),
            title: t("settings.title"),
            is_open: true,
            on_dismiss: Some(ctx.bind(
                SetSettingsOpen(false),
                (|s, a, _| s.show_settings = a.0) as Handler<InboxState, SetSettingsOpen>,
            )),
            width: Some(modal_width),
            content: Box::new(
                Scroll {
                    direction: FlexDirection::Column,
                    width: None,
                    height: Some(body_height),
                    show_scrollbar: true,
                    child: Some(Box::new(
                        VStack {
                            spacing: Some(16.0),
                            children: vec![
                                Text::new(TextContent::Key("settings.general".into()))
                                    .size(14.0)
                                    .into_node(),
                                SegmentedControl {
                                    options: vec![t("settings.lang_en"), t("settings.lang_es")],
                                    selected_index: if view.state.locale.0 == "es-ES" {
                                        1
                                    } else {
                                        0
                                    },
                                    on_change: Some(Arc::new(move |idx| {
                                        let loc = if idx == 1 { "es-ES" } else { "en-US" };
                                        ActionEnvelope {
                                            id: locale_id,
                                            payload: serde_json::to_vec(&SetLocale(Locale(
                                                loc.into(),
                                            )))
                                            .unwrap(),
                                        }
                                    })),
                                }
                                .build(ctx, view),
                                FormControl {
                                    id: None,
                                    label: Some(t("settings.inbox_type.label")),
                                    required: false,
                                    error: None,
                                    helper: Some(t("settings.inbox_type.helper")),
                                    child: Box::new(
                                        Select {
                                            id: WidgetNodeId::explicit("inbox_type_select"),
                                            selected_label: Some(inbox_type_display),
                                            placeholder: t("settings.inbox_type.placeholder"),
                                            is_open: view.state.show_inbox_type_select,
                                            on_toggle: Some(ActionEnvelope {
                                                id: inbox_type_open_id,
                                                payload: serde_json::to_vec(
                                                    &SetInboxTypeSelectOpen(
                                                        !view.state.show_inbox_type_select,
                                                    ),
                                                )
                                                .unwrap(),
                                            }),
                                            items: vec![
                                                SelectItem {
                                                    label: t("settings.inbox_type.default"),
                                                    icon: None,
                                                    on_select: ActionEnvelope {
                                                        id: inbox_type_id,
                                                        payload: serde_json::to_vec(&SetInboxType(
                                                            "Default".into(),
                                                        ))
                                                        .unwrap(),
                                                    },
                                                },
                                                SelectItem {
                                                    label: t("settings.inbox_type.priority"),
                                                    icon: None,
                                                    on_select: ActionEnvelope {
                                                        id: inbox_type_id,
                                                        payload: serde_json::to_vec(&SetInboxType(
                                                            "Priority Inbox".into(),
                                                        ))
                                                        .unwrap(),
                                                    },
                                                },
                                            ],
                                            ..Default::default()
                                        }
                                        .build(ctx, view),
                                    ),
                                }
                                .build(ctx, view),
                                Divider {
                                    orientation: fission_widgets::divider::Orientation::Horizontal,
                                }
                                .build(ctx, view),
                                Text::new(TextContent::Key("settings.appearance".into()))
                                    .size(14.0)
                                    .into_node(),
                                FormControl {
                                    id: None,
                                    label: Some(t("settings.theme.label")),
                                    required: false,
                                    error: None,
                                    helper: None,
                                    child: Box::new(
                                        Select {
                                            id: WidgetNodeId::explicit("theme_select"),
                                            selected_label: Some(theme_display),
                                            placeholder: t("settings.theme.placeholder"),
                                            is_open: view.state.show_theme_select,
                                            on_toggle: Some(ActionEnvelope {
                                                id: theme_open_id,
                                                payload: serde_json::to_vec(&SetThemeSelectOpen(
                                                    !view.state.show_theme_select,
                                                ))
                                                .unwrap(),
                                            }),
                                            items: theme_items,
                                            ..Default::default()
                                        }
                                        .build(ctx, view),
                                    ),
                                }
                                .build(ctx, view),
                                FormControl {
                                    id: None,
                                    label: Some(t("settings.density.label")),
                                    required: false,
                                    error: None,
                                    helper: Some(t("settings.density.helper")),
                                    child: Box::new(
                                        Select {
                                            id: WidgetNodeId::explicit("density_select"),
                                            selected_label: Some(density_display),
                                            placeholder: t("settings.density.placeholder"),
                                            is_open: view.state.show_density_select,
                                            on_toggle: Some(ActionEnvelope {
                                                id: density_open_id,
                                                payload: serde_json::to_vec(&SetDensitySelectOpen(
                                                    !view.state.show_density_select,
                                                ))
                                                .unwrap(),
                                            }),
                                            items: density_items,
                                            ..Default::default()
                                        }
                                        .build(ctx, view),
                                    ),
                                }
                                .build(ctx, view),
                                FormControl {
                                    id: None,
                                    label: Some(t("settings.zoom.label")),
                                    required: false,
                                    error: None,
                                    helper: Some(t("settings.zoom.helper")),
                                    child: Box::new(
                                        Slider {
                                            id: None,
                                            value: view.state.zoom_level,
                                            min: 0.75,
                                            max: 1.25,
                                            on_change: Some(ActionEnvelope {
                                                id: zoom_id,
                                                payload: serde_json::to_vec(&SetZoomLevel(
                                                    view.state.zoom_level,
                                                ))
                                                .unwrap(),
                                            }),
                                        }
                                        .into_node(),
                                    ),
                                }
                                .build(ctx, view),
                                Grid {
                                    id: None,
                                    columns: vec![GridTrack::Fr(1.0), GridTrack::Fr(1.0)],
                                    rows: vec![GridTrack::Auto],
                                    column_gap: Some(12.0),
                                    row_gap: Some(12.0),
                                    padding: [0.0; 4],
                                    children: vec![
                                        GridItem::new(
                                            Card {
                                                child: Box::new(
                                                    VStack {
                                                        spacing: Some(8.0),
                                                        children: vec![
                                                            Text::new(TextContent::Key(
                                                                "settings.theme.preview_light"
                                                                    .into(),
                                                            ))
                                                            .size(12.0)
                                                            .into_node(),
                                                            theme_preview(
                                                                ctx,
                                                                view,
                                                                theme_id,
                                                                "Light",
                                                                Color {
                                                                    r: 245,
                                                                    g: 245,
                                                                    b: 248,
                                                                    a: 255,
                                                                },
                                                                Color {
                                                                    r: 30,
                                                                    g: 144,
                                                                    b: 255,
                                                                    a: 255,
                                                                },
                                                                view.state.theme_mode == "Light",
                                                            ),
                                                        ],
                                                    }
                                                    .into_node(),
                                                ),
                                                ..Default::default()
                                            }
                                            .build(ctx, view),
                                        )
                                        .into_node(),
                                        GridItem::new(
                                            Card {
                                                child: Box::new(
                                                    VStack {
                                                        spacing: Some(8.0),
                                                        children: vec![
                                                            Text::new(TextContent::Key(
                                                                "settings.theme.preview_dark"
                                                                    .into(),
                                                            ))
                                                            .size(12.0)
                                                            .into_node(),
                                                            theme_preview(
                                                                ctx,
                                                                view,
                                                                theme_id,
                                                                "Dark",
                                                                Color {
                                                                    r: 28,
                                                                    g: 30,
                                                                    b: 34,
                                                                    a: 255,
                                                                },
                                                                Color {
                                                                    r: 255,
                                                                    g: 214,
                                                                    b: 10,
                                                                    a: 255,
                                                                },
                                                                view.state.theme_mode == "Dark",
                                                            ),
                                                        ],
                                                    }
                                                    .into_node(),
                                                ),
                                                ..Default::default()
                                            }
                                            .build(ctx, view),
                                        )
                                        .into_node(),
                                    ],
                                }
                                .into_node(),
                                Divider {
                                    orientation: fission_widgets::divider::Orientation::Horizontal,
                                }
                                .build(ctx, view),
                                Text::new(TextContent::Key("settings.signature.title".into()))
                                    .size(14.0)
                                    .into_node(),
                                FormControl {
                                    id: None,
                                    label: Some("Signature".into()),
                                    required: false,
                                    error: None,
                                    helper: Some("Displayed at the end of new emails".into()),
                                    child: Box::new(signature_editor),
                                }
                                .build(ctx, view),
                                Button {
                                    variant: ButtonVariant::Outline,
                                    child: Some(Box::new(
                                        Text::new(TextContent::Key(
                                            "settings.signature.save".into(),
                                        ))
                                        .into_node(),
                                    )),
                                    on_press: Some(ActionEnvelope {
                                        id: signature_edit_id,
                                        payload: serde_json::to_vec(&SetSignatureEditing(false))
                                            .unwrap(),
                                    }),
                                    ..Default::default()
                                }
                                .into_node(),
                                Divider {
                                    orientation: fission_widgets::divider::Orientation::Horizontal,
                                }
                                .build(ctx, view),
                                Text::new(TextContent::Key("settings.labs.title".into()))
                                    .size(14.0)
                                    .into_node(),
                                Card {
                                    child: Box::new(
                                        VStack {
                                            spacing: Some(12.0),
                                            children: vec![
                                                HStack {
                                                    spacing: Some(8.0),
                                                    children: vec![
                                                        Icon::svg(
                                                            material::action::check_circle::regular(
                                                            ),
                                                        )
                                                        .size(18.0)
                                                        .into_node(),
                                                        Text::new(TextContent::Key(
                                                            "settings.labs.smart_compose".into(),
                                                        ))
                                                        .into_node(),
                                                        fission_core::ui::widgets::Spacer {
                                                            flex_grow: 1.0,
                                                            ..Default::default()
                                                        }
                                                        .into_node(),
                                                        Switch {
                                                            checked: view
                                                                .state
                                                                .smart_compose_enabled,
                                                            on_toggle: Some(ActionEnvelope {
                                                                id: smart_compose_id,
                                                                payload: serde_json::to_vec(
                                                                    &SetSmartComposeEnabled(
                                                                        !view
                                                                            .state
                                                                            .smart_compose_enabled,
                                                                    ),
                                                                )
                                                                .unwrap(),
                                                            }),
                                                            ..Default::default()
                                                        }
                                                        .into_node(),
                                                    ],
                                                }
                                                .into_node(),
                                                HStack {
                                                    spacing: Some(8.0),
                                                    children: vec![
                                                Icon::svg(
                                                    material::action::report_problem::regular(),
                                                )
                                                .size(18.0)
                                                .into_node(),
                                                Text::new(TextContent::Key(
                                                    "settings.labs.offline".into(),
                                                ))
                                                .into_node(),
                                                fission_core::ui::widgets::Spacer {
                                                    flex_grow: 1.0,
                                                    ..Default::default()
                                                }
                                                .into_node(),
                                                Switch {
                                                    checked: view.state.offline_enabled,
                                                    on_toggle: Some(ActionEnvelope {
                                                        id: offline_id,
                                                        payload: serde_json::to_vec(
                                                            &SetOfflineEnabled(
                                                                !view.state.offline_enabled,
                                                            ),
                                                        )
                                                        .unwrap(),
                                                    }),
                                                    ..Default::default()
                                                }
                                                .into_node(),
                                            ],
                                                }
                                                .into_node(),
                                                HStack {
                                                    spacing: Some(8.0),
                                                    children: vec![
                                                Icon::svg(material::action::info::regular())
                                                    .size(18.0)
                                                    .into_node(),
                                                Text::new(TextContent::Key(
                                                    "settings.labs.auto_advance".into(),
                                                ))
                                                .into_node(),
                                                fission_core::ui::widgets::Spacer {
                                                    flex_grow: 1.0,
                                                    ..Default::default()
                                                }
                                                .into_node(),
                                                Switch {
                                                    checked: view.state.auto_advance_enabled,
                                                    on_toggle: Some(ActionEnvelope {
                                                        id: auto_advance_id,
                                                        payload: serde_json::to_vec(
                                                            &SetAutoAdvanceEnabled(
                                                                !view.state.auto_advance_enabled,
                                                            ),
                                                        )
                                                        .unwrap(),
                                                    }),
                                                    ..Default::default()
                                                }
                                                .into_node(),
                                            ],
                                                }
                                                .into_node(),
                                            ],
                                        }
                                        .into_node(),
                                    ),
                                    ..Default::default()
                                }
                                .build(ctx, view),
                                Text::new(TextContent::Key("settings.labels.title".into()))
                                    .size(12.0)
                                    .into_node(),
                                Wrap {
                                    direction: fission_ir::op::FlexDirection::Row,
                                    spacing: Some(6.0),
                                    children: draggable_labels,
                                }
                                .build(ctx, view),
                                drop_target,
                                pinned_badge,
                                HStack {
                                    spacing: Some(6.0),
                                    children: vec![
                                        GestureDetector {
                                            on_tap: Some(ActionEnvelope {
                                                id: tip_id,
                                                payload: serde_json::to_vec(&SetQuickTipOpen(true))
                                                    .unwrap(),
                                            }),
                                            child: Box::new(
                                                HStack {
                                                    spacing: Some(6.0),
                                                    children: vec![
                                                Icon::svg(material::action::info::regular())
                                                    .size(16.0)
                                                    .into_node(),
                                                Text::new(TextContent::Key(
                                                    "settings.tips.show".into(),
                                                ))
                                                .size(12.0)
                                                .into_node(),
                                            ],
                                                }
                                                .into_node(),
                                            ),
                                            ..Default::default()
                                        }
                                        .into_node(),
                                        Badge {
                                            text: "Beta".into(),
                                            ..Default::default()
                                        }
                                        .build(ctx, view),
                                    ],
                                }
                                .into_node(),
                                FormControl {
                                    id: None,
                                    label: Some("Page size".into()),
                                    required: false,
                                    error: None,
                                    helper: Some("Rows per page".into()),
                                    child: Box::new(
                                        NumberInput {
                                            id: None,
                                            value: 50.0,
                                            min: Some(10.0),
                                            max: Some(100.0),
                                            step: 10.0,
                                            on_increment: None,
                                            on_decrement: None,
                                            on_change: None,
                                            ..Default::default()
                                        }
                                        .build(ctx, view),
                                    ),
                                }
                                .build(ctx, view),
                            ],
                        }
                        .into_node(),
                    )),
                    ..Default::default()
                }
                .into_node(),
            ),
            actions: vec![ModalAction {
                label: "Close".into(),
                is_primary: true,
                on_press: Some(ctx.bind(
                    SetSettingsOpen(false),
                    (|s, a, _| s.show_settings = a.0) as Handler<InboxState, SetSettingsOpen>,
                )),
            }],
        }
        .build(ctx, view)
    }
}
