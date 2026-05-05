use crate::model::{
    Folder, InboxState, SetCalendarSelected, SetMeetCameraOn, SetMeetMicOn, ShowToast,
};
use chrono::{Datelike, Local};
use fission_core::ui::{
    Button, ButtonVariant, Column, Container, Node, Row, Switch, Text, TextContent,
};
use fission_core::{BuildCtx, Handler, View, Widget, WidgetNodeId};
use fission_icons::material;
use fission_widgets::{
    Calendar, Card, HStack, Icon, Menu, MenuItem, Skeleton, Spinner, Stat, Stepper, VStack,
};
use serde_json;

pub struct RightSidebar;

impl Widget<InboxState> for RightSidebar {
    fn build(&self, ctx: &mut BuildCtx<InboxState>, view: &View<InboxState>) -> Node {
        let tokens = &view.env.theme.tokens;
        let t = |key: &str| {
            view.env
                .i18n
                .get(&view.env.locale, key)
                .map(|s| s.to_string())
                .unwrap_or_else(|| key.to_string())
        };
        let today = Local::now().date_naive();

        let meet_camera_id = ctx
            .bind(
                SetMeetCameraOn(false),
                (|s: &mut InboxState, a: SetMeetCameraOn, _| s.meet_camera_on = a.0)
                    as Handler<InboxState, SetMeetCameraOn>,
            )
            .id;
        let meet_mic_id = ctx
            .bind(
                SetMeetMicOn(false),
                (|s: &mut InboxState, a: SetMeetMicOn, _| s.meet_mic_on = a.0)
                    as Handler<InboxState, SetMeetMicOn>,
            )
            .id;
        let calendar_id = ctx
            .bind(
                SetCalendarSelected(today),
                (|s: &mut InboxState, a: SetCalendarSelected, _| s.calendar_selected = Some(a.0))
                    as Handler<InboxState, SetCalendarSelected>,
            )
            .id;
        let toast_id = ctx
            .bind(
                ShowToast("".into()),
                (|s: &mut InboxState, a: ShowToast, _| {
                    s.toast_message = Some(a.0);
                    s.show_toast = true;
                }) as Handler<InboxState, ShowToast>,
            )
            .id;

        let unread_total = view
            .state
            .emails
            .iter()
            .filter(|e| e.folders.contains(&Folder::Inbox) && !e.is_read)
            .count();
        let starred_total = view.state.emails.iter().filter(|e| e.is_flagged).count();

        let quick_actions = Menu {
            items: vec![
                MenuItem {
                    label: t("quick.new_event"),
                    icon: None,
                    on_select: Some(fission_core::ActionEnvelope {
                        id: toast_id,
                        payload: serde_json::to_vec(&ShowToast(t("toast.new_event"))).unwrap(),
                    }),
                },
                MenuItem {
                    label: t("quick.new_task"),
                    icon: None,
                    on_select: Some(fission_core::ActionEnvelope {
                        id: toast_id,
                        payload: serde_json::to_vec(&ShowToast(t("toast.new_task"))).unwrap(),
                    }),
                },
                MenuItem {
                    label: t("quick.add_reminder"),
                    icon: None,
                    on_select: Some(fission_core::ActionEnvelope {
                        id: toast_id,
                        payload: serde_json::to_vec(&ShowToast(t("toast.add_reminder"))).unwrap(),
                    }),
                },
            ],
            width: Some(252.0),
            max_height: Some(200.0),
        }
        .build(ctx, view);

        Container::new(
            fission_core::ui::Scroll {
                direction: fission_ir::op::FlexDirection::Column,
                show_scrollbar: false,
                flex_grow: 1.0,
                flex_shrink: 1.0,
                child: Some(Box::new(
                    Column {
                        children: vec![
                            Card {
                                child: Box::new(
                                    Row {
                                        gap: Some(12.0),
                                        children: vec![
                                            VStack {
                                                spacing: Some(4.0),
                                                children: vec![
                                                    Spinner {
                                                        id: WidgetNodeId::explicit("sync_spinner"),
                                                        color: Some(tokens.colors.primary),
                                                        animated: false,
                                                    }
                                                    .build(ctx, view),
                                                    Skeleton {
                                                        id: WidgetNodeId::explicit("sync_skeleton"),
                                                        width: Some(20.0),
                                                        height: Some(4.0),
                                                        circle: false,
                                                        animated: false,
                                                    }
                                                    .build(ctx, view),
                                                ],
                                            }
                                            .into_node(),
                                            VStack {
                                                spacing: Some(4.0),
                                                children: vec![
                                                    Text::new(TextContent::Key(
                                                        "quick.syncing".into(),
                                                    ))
                                                    .size(14.0)
                                                    .into_node(),
                                                    Text::new(TextContent::Key(
                                                        "quick.last_update".into(),
                                                    ))
                                                    .size(12.0)
                                                    .color(tokens.colors.text_secondary)
                                                    .into_node(),
                                                ],
                                            }
                                            .into_node(),
                                        ],
                                        ..Default::default()
                                    }
                                    .into_node(),
                                ),
                                ..Default::default()
                            }
                            .build(ctx, view),
                            Calendar {
                                year: today.year(),
                                month: today.month(),
                                selected_date: view.state.calendar_selected.or(Some(today)),
                                on_select: Some(std::sync::Arc::new(move |d| {
                                    fission_core::ActionEnvelope {
                                        id: calendar_id,
                                        payload: serde_json::to_vec(&SetCalendarSelected(d))
                                            .unwrap(),
                                    }
                                })),
                                on_navigate: None,
                            }
                            .build(ctx, view),
                            Card {
                                child: Box::new(
                                    VStack {
                                        spacing: Some(10.0),
                                        children: vec![
                                            Text::new(TextContent::Key("quick.actions".into()))
                                                .size(16.0)
                                                .into_node(),
                                            quick_actions,
                                        ],
                                    }
                                    .into_node(),
                                ),
                                ..Default::default()
                            }
                            .build(ctx, view),
                            Card {
                                child: Box::new(
                                    VStack {
                                        spacing: Some(10.0),
                                        children: vec![
                                            Text::new(TextContent::Key("quick.meet".into()))
                                                .size(16.0)
                                                .into_node(),
                                            HStack {
                                                spacing: Some(8.0),
                                                children: vec![
                                                    Text::new(TextContent::Key(
                                                        "quick.camera".into(),
                                                    ))
                                                    .size(14.0)
                                                    .into_node(),
                                                    fission_core::ui::widgets::Spacer {
                                                        flex_grow: 1.0,
                                                        ..Default::default()
                                                    }
                                                    .into_node(),
                                                    Switch {
                                                        checked: view.state.meet_camera_on,
                                                        on_toggle: Some(
                                                            fission_core::ActionEnvelope {
                                                                id: meet_camera_id,
                                                                payload: serde_json::to_vec(
                                                                    &SetMeetCameraOn(
                                                                        !view.state.meet_camera_on,
                                                                    ),
                                                                )
                                                                .unwrap(),
                                                            },
                                                        ),
                                                        ..Default::default()
                                                    }
                                                    .into_node(),
                                                ],
                                            }
                                            .into_node(),
                                            HStack {
                                                spacing: Some(8.0),
                                                children: vec![
                                                    Text::new(TextContent::Key(
                                                        "quick.microphone".into(),
                                                    ))
                                                    .size(14.0)
                                                    .into_node(),
                                                    fission_core::ui::widgets::Spacer {
                                                        flex_grow: 1.0,
                                                        ..Default::default()
                                                    }
                                                    .into_node(),
                                                    Switch {
                                                        checked: view.state.meet_mic_on,
                                                        on_toggle: Some(
                                                            fission_core::ActionEnvelope {
                                                                id: meet_mic_id,
                                                                payload: serde_json::to_vec(
                                                                    &SetMeetMicOn(
                                                                        !view.state.meet_mic_on,
                                                                    ),
                                                                )
                                                                .unwrap(),
                                                            },
                                                        ),
                                                        ..Default::default()
                                                    }
                                                    .into_node(),
                                                ],
                                            }
                                            .into_node(),
                                            fission_core::ui::widgets::Spacer {
                                                height: Some(4.0),
                                                ..Default::default()
                                            }
                                            .into_node(),
                                            Button {
                                                variant: ButtonVariant::Filled,
                                                child: Some(Box::new(
                                                    HStack {
                                                        spacing: Some(8.0),
                                                        children: vec![
                                                            Icon::svg(
                                                                material::av::video_call::regular(),
                                                            )
                                                            .size(18.0)
                                                            .into_node(),
                                                            Text::new(TextContent::Key(
                                                                "quick.start_meeting".into(),
                                                            ))
                                                            .into_node(),
                                                        ],
                                                    }
                                                    .into_node(),
                                                )),
                                                on_press: None,
                                                ..Default::default()
                                            }
                                            .into_node(),
                                        ],
                                    }
                                    .into_node(),
                                ),
                                ..Default::default()
                            }
                            .build(ctx, view),
                            Card {
                                child: Box::new(
                                    VStack {
                                        spacing: Some(10.0),
                                        children: vec![
                                            Text::new(TextContent::Key(
                                                "quick.mailbox_stats".into(),
                                            ))
                                            .size(16.0)
                                            .into_node(),
                                            HStack {
                                                spacing: Some(16.0),
                                                children: vec![
                                                    fission_widgets::CircularProgress {
                                                        value: Some(0.65),
                                                        size: 40.0,
                                                        ..Default::default()
                                                    }
                                                    .build(ctx, view),
                                                    VStack {
                                                        spacing: Some(2.0),
                                                        children: vec![
                                                            Text::new("65%").size(16.0).into_node(),
                                                            Text::new(t("quick.unread"))
                                                                .size(12.0)
                                                                .color(tokens.colors.text_secondary)
                                                                .into_node(),
                                                        ],
                                                    }
                                                    .into_node(),
                                                ],
                                            }
                                            .into_node(),
                                            HStack {
                                                spacing: Some(8.0),
                                                children: vec![
                                                    Container::new(
                                                        Stat {
                                                            label: t("quick.unread"),
                                                            value: unread_total.to_string(),
                                                            help_text: Some(t("quick.in_inbox")),
                                                        }
                                                        .build(ctx, view),
                                                    )
                                                    .flex_grow(1.0)
                                                    .into_node(),
                                                    Container::new(
                                                        Stat {
                                                            label: t("quick.starred"),
                                                            value: starred_total.to_string(),
                                                            help_text: Some(t("quick.all_folders")),
                                                        }
                                                        .build(ctx, view),
                                                    )
                                                    .flex_grow(1.0)
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
                            Card {
                                child: Box::new(
                                    VStack {
                                        spacing: Some(12.0),
                                        children: vec![
                                            Text::new(TextContent::Key("quick.setup".into()))
                                                .size(16.0)
                                                .into_node(),
                                            Stepper {
                                                steps: vec![
                                                    t("quick.import"),
                                                    t("quick.customize"),
                                                    t("quick.invite"),
                                                ],
                                                active_index: 1,
                                            }
                                            .build(ctx, view),
                                        ],
                                    }
                                    .into_node(),
                                ),
                                ..Default::default()
                            }
                            .build(ctx, view),
                        ],
                        ..Default::default()
                    }
                    .into_node(),
                )),
                ..Default::default()
            }
            .into_node(),
        )
        .padding_all(8.0)
        .width(240.0)
        .bg(tokens.colors.surface)
        .into_node()
    }
}
