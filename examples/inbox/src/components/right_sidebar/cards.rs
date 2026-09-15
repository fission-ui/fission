use super::density::SidebarDensity;
use crate::model::quick::{set_meet_camera_on, set_meet_mic_on, show_toast};
use crate::model::{Folder, InboxState, SetMeetCameraOn, SetMeetMicOn, ShowToast};
use fission::core::reduce_with;
use fission::core::ui::widgets::Spacer;
use fission::core::ui::{Button, ButtonVariant, Row, Switch, Text, TextContent, Widget};
use fission::core::{ActionEnvelope, WidgetId};
use fission::icons::material;
use fission::widgets::{
    Card, CircularProgress, HStack, Icon, Menu, MenuItem, Skeleton, SkeletonMotion, Spinner,
    SpinnerMotion, Stepper, VStack,
};

/// A card with a heading above its content.
struct TitledCard {
    title_key: &'static str,
    child: Widget,
}

impl From<TitledCard> for Widget {
    fn from(card: TitledCard) -> Self {
        let (_, view) = fission::build::current::<()>();
        let tokens = &view.env().theme.tokens;
        Card {
            child: VStack {
                spacing: Some(tokens.spacing.s),
                children: vec![
                    Text::new(TextContent::Key(card.title_key.into()))
                        .size(tokens.typography.font_size_lg)
                        .into(),
                    card.child,
                ],
            }
            .into(),
            ..Default::default()
        }
        .into()
    }
}

/// Shows that mail is syncing and when it last updated.
pub(super) struct SyncStatusCard;

impl From<SyncStatusCard> for Widget {
    fn from(_: SyncStatusCard) -> Self {
        let (_, view) = fission::build::current::<()>();
        let tokens = &view.env().theme.tokens;
        Card {
            child: Row {
                gap: Some(tokens.spacing.s),
                children: vec![
                    VStack {
                        spacing: Some(tokens.spacing.xs),
                        children: vec![
                            Spinner {
                                id: WidgetId::explicit("sync_spinner"),
                                label: None,
                                color: Some(tokens.colors.primary),
                                motion: Some(SpinnerMotion::Default),
                            }
                            .into(),
                            Skeleton {
                                id: WidgetId::explicit("sync_skeleton"),
                                width: Some(tokens.spacing.l - tokens.spacing.xs),
                                height: Some(tokens.spacing.xs),
                                circle: false,
                                motion: Some(SkeletonMotion::Default),
                            }
                            .into(),
                        ],
                    }
                    .into(),
                    VStack {
                        spacing: Some(tokens.spacing.xs),
                        children: vec![
                            Text::new(TextContent::Key("quick.syncing".into()))
                                .size(tokens.typography.font_size_base)
                                .into(),
                            Text::new(TextContent::Key("quick.last_update".into()))
                                .size(tokens.typography.font_size_xs)
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
        .into()
    }
}

/// Quick actions, with each one's label key, toast key and semantics identifier.
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

/// Shortcuts that create an event, task or reminder.
pub(super) struct QuickActionsCard {
    pub density: SidebarDensity,
}

impl From<QuickActionsCard> for Widget {
    fn from(card: QuickActionsCard) -> Self {
        let (ctx, view) = fission::build::current::<InboxState>();
        TitledCard {
            title_key: "quick.actions",
            child: Menu {
                items: QUICK_ACTIONS
                    .iter()
                    .map(|(label, toast, identifier)| MenuItem {
                        label: view.tr(label),
                        icon: None,
                        on_select: Some(
                            ctx.bind(ShowToast(view.tr(toast)), reduce_with!(show_toast)),
                        ),
                        semantics_identifier: Some((*identifier).into()),
                    })
                    .collect(),
                width: None,
                max_height: Some(card.density.menu_max_height()),
            }
            .into(),
        }
        .into()
    }
}

/// Camera and microphone switches and the start meeting button.
pub(super) struct MeetCard;

impl From<MeetCard> for Widget {
    fn from(_: MeetCard) -> Self {
        let (ctx, view) = fission::build::current::<InboxState>();
        let state = view.state();
        let tokens = &view.env().theme.tokens;
        TitledCard {
            title_key: "quick.meet",
            child: VStack {
                spacing: Some(tokens.spacing.s),
                children: vec![
                    MeetToggle {
                        label_key: "quick.camera",
                        checked: state.meet_camera_on,
                        on_toggle: ctx.bind(
                            SetMeetCameraOn(!state.meet_camera_on),
                            reduce_with!(set_meet_camera_on),
                        ),
                    }
                    .into(),
                    MeetToggle {
                        label_key: "quick.microphone",
                        checked: state.meet_mic_on,
                        on_toggle: ctx.bind(
                            SetMeetMicOn(!state.meet_mic_on),
                            reduce_with!(set_meet_mic_on),
                        ),
                    }
                    .into(),
                    Button {
                        variant: ButtonVariant::Filled,
                        child: Some(
                            HStack {
                                spacing: Some(tokens.spacing.s),
                                children: vec![
                                    Icon::svg(material::av::video_call::regular())
                                        .size(tokens.typography.font_size_lg)
                                        .into(),
                                    Text::new(TextContent::Key("quick.start_meeting".into()))
                                        .into(),
                                ],
                            }
                            .into(),
                        ),
                        on_press: Some(ctx.bind(
                            ShowToast(view.tr("quick.meeting_started")),
                            reduce_with!(show_toast),
                        )),
                        ..Default::default()
                    }
                    .semantics_identifier("inbox.meet.start")
                    .into(),
                ],
            }
            .into(),
        }
        .into()
    }
}

/// One labelled meeting switch.
struct MeetToggle {
    label_key: &'static str,
    checked: bool,
    on_toggle: ActionEnvelope,
}

impl From<MeetToggle> for Widget {
    fn from(toggle: MeetToggle) -> Self {
        let (_, view) = fission::build::current::<()>();
        let tokens = &view.env().theme.tokens;
        HStack {
            spacing: Some(tokens.spacing.s),
            children: vec![
                Text::new(TextContent::Key(toggle.label_key.into()))
                    .size(tokens.typography.font_size_base)
                    .into(),
                Spacer {
                    flex_grow: 1.0,
                    ..Default::default()
                }
                .into(),
                Switch {
                    checked: toggle.checked,
                    on_toggle: Some(toggle.on_toggle),
                    ..Default::default()
                }
                .into(),
            ],
        }
        .into()
    }
}

/// Storage used and the unread and starred counts.
pub(super) struct MailboxStatsCard {
    pub density: SidebarDensity,
}

impl From<MailboxStatsCard> for Widget {
    fn from(card: MailboxStatsCard) -> Self {
        let (_, view) = fission::build::current::<InboxState>();
        let state = view.state();
        let tokens = &view.env().theme.tokens;
        let unread = state
            .emails
            .iter()
            .filter(|email| email.folders.contains(&Folder::Inbox) && !email.is_read)
            .count();
        let starred = state.emails.iter().filter(|email| email.is_flagged).count();
        let compact = card.density.is_compact();

        TitledCard {
            title_key: "quick.mailbox_stats",
            child: VStack {
                spacing: Some(tokens.spacing.s),
                children: vec![
                    HStack {
                        spacing: Some(tokens.spacing.m),
                        children: vec![
                            CircularProgress {
                                value: Some(0.65),
                                size: Some(card.density.progress_size()),
                                ..Default::default()
                            }
                            .into(),
                            MailboxStat {
                                value: "65%".into(),
                                label_key: "quick.unread",
                                detail_key: None,
                            }
                            .into(),
                        ],
                    }
                    .into(),
                    HStack {
                        spacing: Some(if compact {
                            tokens.spacing.m
                        } else {
                            tokens.spacing.l
                        }),
                        children: vec![
                            MailboxStat {
                                value: unread.to_string(),
                                label_key: "quick.unread",
                                detail_key: (!compact).then_some("quick.in_inbox"),
                            }
                            .into(),
                            MailboxStat {
                                value: starred.to_string(),
                                label_key: "quick.starred",
                                detail_key: (!compact).then_some("quick.all_folders"),
                            }
                            .into(),
                        ],
                    }
                    .into(),
                ],
            }
            .into(),
        }
        .into()
    }
}

/// A figure with its label and an optional detail line.
struct MailboxStat {
    value: String,
    label_key: &'static str,
    detail_key: Option<&'static str>,
}

impl From<MailboxStat> for Widget {
    fn from(stat: MailboxStat) -> Self {
        let (_, view) = fission::build::current::<()>();
        let tokens = &view.env().theme.tokens;
        let caption = |key: &str| {
            Text::new(view.tr(key))
                .size(tokens.typography.font_size_xs)
                .color(tokens.colors.text_secondary)
                .into()
        };
        let mut children: Vec<Widget> = vec![
            Text::new(stat.value)
                .size(tokens.typography.font_size_lg)
                .into(),
            caption(stat.label_key),
        ];
        if let Some(detail) = stat.detail_key {
            children.push(caption(detail));
        }
        VStack {
            spacing: Some(tokens.spacing.xs / 2.0),
            children,
        }
        .into()
    }
}

/// Onboarding progress.
pub(super) struct SetupCard;

impl From<SetupCard> for Widget {
    fn from(_: SetupCard) -> Self {
        let (_, view) = fission::build::current::<()>();
        TitledCard {
            title_key: "quick.setup",
            child: Stepper {
                steps: vec![
                    view.tr("quick.import"),
                    view.tr("quick.customize"),
                    view.tr("quick.invite"),
                ],
                active_index: 1,
            }
            .into(),
        }
        .into()
    }
}
