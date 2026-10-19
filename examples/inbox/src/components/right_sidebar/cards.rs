use super::density::SidebarDensity;
use crate::model::quick::show_toast;
use crate::model::{Folder, InboxState, ShowToast};
use fission::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Serialize, Deserialize)]
enum MeetSetting {
    Camera,
    Microphone,
}

fission_component! {
    TitledCard<()> {
        title_key: &'static str,
        child: Widget,
    } |card, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
    Card(
        child: VStack(
            spacing: tokens.spacing.s,
            children: [
                Text(TextContent::Key(card.title_key.into()))
                    .size(tokens.typography.font_size_lg),
                card.child.clone(),
            ],
        ),
    )
}

fission_component! {
    pub(super) SyncStatusCard<()> |_card, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
    Card(
        child: Row(
            gap: tokens.spacing.s,
            children: [
                VStack(
                    spacing: tokens.spacing.xs,
                    children: [
                        Spinner(
                            id: WidgetId::explicit("sync_spinner"),
                            color: tokens.colors.primary,
                            motion: SpinnerMotion::Default,
                        ),
                        Skeleton(
                            id: WidgetId::explicit("sync_skeleton"),
                            width: tokens.spacing.l - tokens.spacing.xs,
                            height: tokens.spacing.xs,
                            motion: SkeletonMotion::Default,
                        ),
                    ],
                ),
                VStack(
                    spacing: tokens.spacing.xs,
                    children: [
                        Text(TextContent::Key("quick.syncing".into()))
                            .size(tokens.typography.font_size_base),
                        Text(TextContent::Key("quick.last_update".into()))
                            .size(tokens.typography.font_size_xs)
                            .color(tokens.colors.text_secondary),
                    ],
                ),
            ],
        ),
    )
}

const QUICK_ACTIONS: [(&str, &str, &str); 3] = [
    (
        "quick.new_event",
        "toast.new_event",
        "inbox.quick.new-event",
    ),
    ("quick.new_task", "toast.new_task", "inbox.quick.new-task"),
    (
        "quick.add_reminder",
        "toast.add_reminder",
        "inbox.quick.add-reminder",
    ),
];

fission_component! {
    pub(super) QuickActionsCard<InboxState> { density: SidebarDensity } |card, _state, cx| =>
    TitledCard(
        "quick.actions",
        Menu(
            items: QUICK_ACTIONS.iter().map(|(label, toast, identifier)| MenuItem {
                label: cx.tr(label),
                icon: None,
                on_select: Some(cx.bind(ShowToast(cx.tr(toast)), reduce_with!(show_toast))),
                semantics_identifier: Some((*identifier).into()),
            }).collect::<Vec<_>>(),
            max_height: Some(card.density.menu_max_height()),
        ),
    )
}

fission_component! {
    pub(super) MeetCard<InboxState> |_card, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
    TitledCard(
        "quick.meet",
        VStack(
            spacing: tokens.spacing.s,
            children: [
                MeetToggle(
                    "quick.camera",
                    state.meet_camera_on,
                    MeetSetting::Camera,
                ),
                MeetToggle(
                    "quick.microphone",
                    state.meet_mic_on,
                    MeetSetting::Microphone,
                ),
                Button(
                    variant: ButtonVariant::Filled,
                    child: HStack(
                        spacing: tokens.spacing.s,
                        children: [
                            Icon::svg(material::av::video_call::regular())
                                .size(tokens.typography.font_size_lg),
                            Text(TextContent::Key("quick.start_meeting".into())),
                        ],
                    ),
                    on_press: cx.bind(
                        ShowToast(cx.tr("quick.meeting_started")),
                        reduce_with!(show_toast),
                    ),
                )
                .semantics_identifier("inbox.meet.start"),
            ],
        ),
    )
}

fission_component! {
    MeetToggle<InboxState> {
        label_key: &'static str,
        checked: bool,
        setting: MeetSetting,
    } |toggle, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
    HStack(
        spacing: tokens.spacing.s,
        children: [
            Text(TextContent::Key(toggle.label_key.into()))
                .size(tokens.typography.font_size_base),
            Spacer(flex_grow: 1.0),
            Switch(
                checked: toggle.checked,
                on_toggle: |toggle, state, _cx| match toggle.setting {
                    MeetSetting::Camera => state.meet_camera_on = !state.meet_camera_on,
                    MeetSetting::Microphone => state.meet_mic_on = !state.meet_mic_on,
                },
            ),
        ],
    )
}

fission_component! {
    pub(super) MailboxStatsCard<InboxState> { density: SidebarDensity } |card, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let unread = state.emails.iter()
            .filter(|email| email.folders.contains(&Folder::Inbox) && !email.is_read)
            .count();
        let starred = state.emails.iter().filter(|email| email.is_flagged).count();
        let compact = card.density.is_compact();
    } =>
    TitledCard(
        "quick.mailbox_stats",
        VStack(
            spacing: tokens.spacing.s,
            children: [
                HStack(
                    spacing: tokens.spacing.m,
                    children: [
                        CircularProgress(
                            value: Some(0.65),
                            size: card.density.progress_size(),
                        ),
                        MailboxStat(String::from("65%"), "quick.unread", None),
                    ],
                ),
                HStack(
                    spacing: if compact { tokens.spacing.m } else { tokens.spacing.l },
                    children: [
                        MailboxStat(
                            unread.to_string(),
                            "quick.unread",
                            (!compact).then_some("quick.in_inbox"),
                        ),
                        MailboxStat(
                            starred.to_string(),
                            "quick.starred",
                            (!compact).then_some("quick.all_folders"),
                        ),
                    ],
                ),
            ],
        ),
    )
}

fission_component! {
    MailboxStat<()> {
        value: String,
        label_key: &'static str,
        detail_key: Option<&'static str>,
    } |stat, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
    VStack(
        spacing: tokens.spacing.xs / 2.0,
        children: [
            Text(stat.value.clone()).size(tokens.typography.font_size_lg),
            Text(cx.tr(stat.label_key))
                .size(tokens.typography.font_size_xs)
                .color(tokens.colors.text_secondary),
            if let Some(detail) = stat.detail_key {
                Text(cx.tr(detail))
                    .size(tokens.typography.font_size_xs)
                    .color(tokens.colors.text_secondary)
            },
        ],
    )
}

fission_component! {
    pub(super) SetupCard<()> |_card, _state, cx| =>
    TitledCard(
        "quick.setup",
        Stepper(
            steps: vec![
                cx.tr("quick.import"),
                cx.tr("quick.customize"),
                cx.tr("quick.invite"),
            ],
            active_index: 1_usize,
        ),
    )
}
