use crate::model::{
    Folder, InboxState, SetCalendarSelected, SetMeetCameraOn, SetMeetMicOn, ShowToast,
};
use chrono::{Datelike, Local};
use fission::core::ui::{Button, ButtonVariant, Container, Row, Switch, Text, TextContent, Widget};
use fission::core::{reduce_with, WidgetId};
use fission::icons::material;
use fission::widgets::{
    Calendar, Card, HStack, Icon, Menu, MenuItem, Skeleton, Spinner, Stepper, VStack,
};
use serde_json;

pub struct RightSidebar;

impl From<RightSidebar> for Widget {
    fn from(_component: RightSidebar) -> Self {
        let (ctx, view) = fission::build::current::<InboxState>();
        let tokens = &view.env().theme.tokens;
        let t = |key: &str| {
            view.env()
                .i18n
                .get(&view.env().locale, key)
                .map(|s| s.to_string())
                .unwrap_or_else(|| key.to_string())
        };
        let today = Local::now().date_naive();
        let compact_sidebar = view.viewport_size().height < 980.0;
        let ultra_compact_sidebar = view.viewport_size().height < 900.0;

        let meet_camera_id = ctx
            .bind(
                SetMeetCameraOn(false),
                reduce_with!((|s: &mut InboxState, a: SetMeetCameraOn, _| s.meet_camera_on = a.0)),
            )
            .id;
        let meet_mic_id = ctx
            .bind(
                SetMeetMicOn(false),
                reduce_with!((|s: &mut InboxState, a: SetMeetMicOn, _| s.meet_mic_on = a.0)),
            )
            .id;
        let calendar_id = ctx
            .bind(
                SetCalendarSelected(today),
                reduce_with!(
                    (|s: &mut InboxState, a: SetCalendarSelected, _| s.calendar_selected =
                        Some(a.0))
                ),
            )
            .id;
        let toast_id = ctx
            .bind(
                ShowToast("".into()),
                reduce_with!(
                    (|s: &mut InboxState, a: ShowToast, _| {
                        s.toast_message = Some(a.0);
                        s.show_toast = true;
                    })
                ),
            )
            .id;

        let unread_total = view
            .state()
            .emails
            .iter()
            .filter(|e| e.folders.contains(&Folder::Inbox) && !e.is_read)
            .count();
        let starred_total = view.state().emails.iter().filter(|e| e.is_flagged).count();

        let quick_actions = Menu {
            items: vec![
                MenuItem {
                    label: t("quick.new_event"),
                    icon: None,
                    on_select: Some(fission::core::ActionEnvelope {
                        id: toast_id,
                        payload: serde_json::to_vec(&ShowToast(t("toast.new_event"))).unwrap(),
                    }),
                },
                MenuItem {
                    label: t("quick.new_task"),
                    icon: None,
                    on_select: Some(fission::core::ActionEnvelope {
                        id: toast_id,
                        payload: serde_json::to_vec(&ShowToast(t("toast.new_task"))).unwrap(),
                    }),
                },
                MenuItem {
                    label: t("quick.add_reminder"),
                    icon: None,
                    on_select: Some(fission::core::ActionEnvelope {
                        id: toast_id,
                        payload: serde_json::to_vec(&ShowToast(t("toast.add_reminder"))).unwrap(),
                    }),
                },
            ],
            width: None,
            max_height: Some(if ultra_compact_sidebar {
                120.0
            } else if compact_sidebar {
                144.0
            } else {
                200.0
            }),
        }
        .into();

        let sidebar_spacing = if ultra_compact_sidebar {
            10.0
        } else if compact_sidebar {
            12.0
        } else {
            16.0
        };
        let calendar_cell_size = if ultra_compact_sidebar {
            26.0
        } else if compact_sidebar {
            30.0
        } else {
            32.0
        };
        let calendar_padding = if ultra_compact_sidebar {
            8.0
        } else if compact_sidebar {
            10.0
        } else {
            12.0
        };

        Container::new(fission::core::ui::Scroll {
            direction: fission::op::FlexDirection::Column,
            show_scrollbar: true,
            flex_grow: 1.0,
            flex_shrink: 1.0,
            child: Some(
                VStack {
                    spacing: Some(sidebar_spacing),
                    children: vec![
                        Card {
                            child: Row {
                                gap: Some(12.0),
                                children: vec![
                                    VStack {
                                        spacing: Some(4.0),
                                        children: vec![
                                            Spinner {
                                                id: WidgetId::explicit("sync_spinner"),
                                                color: Some(tokens.colors.primary),
                                                animated: true,
                                            }
                                            .into(),
                                            Skeleton {
                                                id: WidgetId::explicit("sync_skeleton"),
                                                width: Some(20.0),
                                                height: Some(4.0),
                                                circle: false,
                                                animated: true,
                                            }
                                            .into(),
                                        ],
                                    }
                                    .into(),
                                    VStack {
                                        spacing: Some(4.0),
                                        children: vec![
                                            Text::new(TextContent::Key("quick.syncing".into()))
                                                .size(14.0)
                                                .into(),
                                            Text::new(TextContent::Key("quick.last_update".into()))
                                                .size(12.0)
                                                .color(tokens.colors.text_secondary)
                                                .into(),
                                        ],
                                    }
                                    .into(),
                                ],
                                ..Default::default()
                            }
                            .into(),
                            ..Default::default()
                        }
                        .into(),
                        Calendar {
                            year: today.year(),
                            month: today.month(),
                            selected_date: view.state().calendar_selected.or(Some(today)),
                            on_select: Some(std::sync::Arc::new(move |d| {
                                fission::core::ActionEnvelope {
                                    id: calendar_id,
                                    payload: serde_json::to_vec(&SetCalendarSelected(d)).unwrap(),
                                }
                            })),
                            on_navigate: None,
                            cell_size: Some(calendar_cell_size),
                            padding: Some(calendar_padding),
                        }
                        .into(),
                        Card {
                            child: VStack {
                                spacing: Some(10.0),
                                children: vec![
                                    Text::new(TextContent::Key("quick.actions".into()))
                                        .size(16.0)
                                        .into(),
                                    quick_actions,
                                ],
                            }
                            .into(),
                            ..Default::default()
                        }
                        .into(),
                        Card {
                            child: VStack {
                                spacing: Some(10.0),
                                children: vec![
                                    Text::new(TextContent::Key("quick.meet".into()))
                                        .size(16.0)
                                        .into(),
                                    HStack {
                                        spacing: Some(8.0),
                                        children: vec![
                                            Text::new(TextContent::Key("quick.camera".into()))
                                                .size(14.0)
                                                .into(),
                                            fission::core::ui::widgets::Spacer {
                                                flex_grow: 1.0,
                                                ..Default::default()
                                            }
                                            .into(),
                                            Switch {
                                                checked: view.state().meet_camera_on,
                                                on_toggle: Some(fission::core::ActionEnvelope {
                                                    id: meet_camera_id,
                                                    payload: serde_json::to_vec(&SetMeetCameraOn(
                                                        !view.state().meet_camera_on,
                                                    ))
                                                    .unwrap(),
                                                }),
                                                ..Default::default()
                                            }
                                            .into(),
                                        ],
                                    }
                                    .into(),
                                    HStack {
                                        spacing: Some(8.0),
                                        children: vec![
                                            Text::new(TextContent::Key("quick.microphone".into()))
                                                .size(14.0)
                                                .into(),
                                            fission::core::ui::widgets::Spacer {
                                                flex_grow: 1.0,
                                                ..Default::default()
                                            }
                                            .into(),
                                            Switch {
                                                checked: view.state().meet_mic_on,
                                                on_toggle: Some(fission::core::ActionEnvelope {
                                                    id: meet_mic_id,
                                                    payload: serde_json::to_vec(&SetMeetMicOn(
                                                        !view.state().meet_mic_on,
                                                    ))
                                                    .unwrap(),
                                                }),
                                                ..Default::default()
                                            }
                                            .into(),
                                        ],
                                    }
                                    .into(),
                                    fission::core::ui::widgets::Spacer {
                                        height: Some(4.0),
                                        ..Default::default()
                                    }
                                    .into(),
                                    Button {
                                        variant: ButtonVariant::Filled,
                                        child: Some(
                                            HStack {
                                                spacing: Some(8.0),
                                                children: vec![
                                                    Icon::svg(material::av::video_call::regular())
                                                        .size(18.0)
                                                        .into(),
                                                    Text::new(TextContent::Key(
                                                        "quick.start_meeting".into(),
                                                    ))
                                                    .into(),
                                                ],
                                            }
                                            .into(),
                                        ),
                                        on_press: None,
                                        ..Default::default()
                                    }
                                    .into(),
                                ],
                            }
                            .into(),
                            ..Default::default()
                        }
                        .into(),
                        Card {
                            child: VStack {
                                spacing: Some(10.0),
                                children: vec![
                                    Text::new(TextContent::Key("quick.mailbox_stats".into()))
                                        .size(16.0)
                                        .into(),
                                    HStack {
                                        spacing: Some(if ultra_compact_sidebar {
                                            12.0
                                        } else {
                                            16.0
                                        }),
                                        children: vec![
                                            fission::widgets::CircularProgress {
                                                value: Some(0.65),
                                                size: if ultra_compact_sidebar {
                                                    34.0
                                                } else {
                                                    40.0
                                                },
                                                ..Default::default()
                                            }
                                            .into(),
                                            VStack {
                                                spacing: Some(2.0),
                                                children: vec![
                                                    Text::new("65%")
                                                        .size(if ultra_compact_sidebar {
                                                            14.0
                                                        } else {
                                                            16.0
                                                        })
                                                        .into(),
                                                    Text::new(t("quick.unread"))
                                                        .size(12.0)
                                                        .color(tokens.colors.text_secondary)
                                                        .into(),
                                                ],
                                            }
                                            .into(),
                                        ],
                                    }
                                    .into(),
                                    if compact_sidebar {
                                        HStack {
                                            spacing: Some(if ultra_compact_sidebar {
                                                16.0
                                            } else {
                                                24.0
                                            }),
                                            children: vec![
                                                VStack {
                                                    spacing: Some(2.0),
                                                    children: vec![
                                                        Text::new(unread_total.to_string())
                                                            .size(15.0)
                                                            .into(),
                                                        Text::new(t("quick.unread"))
                                                            .size(12.0)
                                                            .color(tokens.colors.text_secondary)
                                                            .into(),
                                                    ],
                                                }
                                                .into(),
                                                VStack {
                                                    spacing: Some(2.0),
                                                    children: vec![
                                                        Text::new(starred_total.to_string())
                                                            .size(15.0)
                                                            .into(),
                                                        Text::new(t("quick.starred"))
                                                            .size(12.0)
                                                            .color(tokens.colors.text_secondary)
                                                            .into(),
                                                    ],
                                                }
                                                .into(),
                                            ],
                                        }
                                        .into()
                                    } else {
                                        HStack {
                                            spacing: Some(20.0),
                                            children: vec![
                                                VStack {
                                                    spacing: Some(2.0),
                                                    children: vec![
                                                        Text::new(unread_total.to_string())
                                                            .size(18.0)
                                                            .into(),
                                                        Text::new(t("quick.unread"))
                                                            .size(12.0)
                                                            .color(tokens.colors.text_secondary)
                                                            .into(),
                                                        Text::new(t("quick.in_inbox"))
                                                            .size(12.0)
                                                            .color(tokens.colors.text_secondary)
                                                            .into(),
                                                    ],
                                                }
                                                .into(),
                                                VStack {
                                                    spacing: Some(2.0),
                                                    children: vec![
                                                        Text::new(starred_total.to_string())
                                                            .size(18.0)
                                                            .into(),
                                                        Text::new(t("quick.starred"))
                                                            .size(12.0)
                                                            .color(tokens.colors.text_secondary)
                                                            .into(),
                                                        Text::new(t("quick.all_folders"))
                                                            .size(12.0)
                                                            .color(tokens.colors.text_secondary)
                                                            .into(),
                                                    ],
                                                }
                                                .into(),
                                            ],
                                        }
                                        .into()
                                    },
                                ],
                            }
                            .into(),
                            ..Default::default()
                        }
                        .into(),
                        Card {
                            child: VStack {
                                spacing: Some(12.0),
                                children: vec![
                                    Text::new(TextContent::Key("quick.setup".into()))
                                        .size(16.0)
                                        .into(),
                                    Stepper {
                                        steps: vec![
                                            t("quick.import"),
                                            t("quick.customize"),
                                            t("quick.invite"),
                                        ],
                                        active_index: 1,
                                    }
                                    .into(),
                                ],
                            }
                            .into(),
                            ..Default::default()
                        }
                        .into(),
                    ],
                    ..Default::default()
                }
                .into(),
            ),
            ..Default::default()
        })
        .padding_all(8.0)
        .bg(tokens.colors.surface)
        .into()
    }
}
