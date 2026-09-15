use fission::prelude::*;
use fission::widgets::{
    Alert, AlertKind, CircularProgress, EmptyState, HStack, ProgressBar, Skeleton, SkeletonMotion,
    Spinner, SpinnerMotion,
};

const CIRCULAR_PROGRESS_SIZE: f32 = 40.0;
const SKELETON_WIDTH: f32 = 120.0;
const SKELETON_HEIGHT: f32 = 20.0;

pub(crate) fn alert() -> Vec<Widget> {
    widgets![
        Alert {
            kind: AlertKind::Info,
            title: "Information".into(),
            description: Some("This is an info alert.".into()),
        },
        Alert {
            kind: AlertKind::Success,
            title: "Success".into(),
            description: None,
        },
        Alert {
            kind: AlertKind::Warning,
            title: "Warning".into(),
            description: Some("Be careful!".into()),
        },
        Alert {
            kind: AlertKind::Error,
            title: "Error".into(),
            description: Some("Something went wrong.".into()),
        },
    ]
}

pub(crate) fn progress() -> Vec<Widget> {
    let (_, view) = fission::build::current::<()>();
    let tokens = &view.env().theme.tokens;
    widgets![
        ProgressBar {
            value: 0.65,
            label: Some("Upload progress".into()),
        },
        HStack {
            spacing: Some(tokens.spacing.m),
            children: widgets![
                Spinner {
                    id: WidgetId::explicit("spinner1"),
                    label: None,
                    color: None,
                    motion: Some(SpinnerMotion::Default),
                },
                CircularProgress {
                    value: Some(0.7),
                    size: Some(CIRCULAR_PROGRESS_SIZE),
                    ..Default::default()
                },
            ],
        },
    ]
}

pub(crate) fn skeleton() -> Vec<Widget> {
    let (_, view) = fission::build::current::<()>();
    let tokens = &view.env().theme.tokens;
    widgets![HStack {
        spacing: Some(tokens.spacing.m),
        children: widgets![
            Skeleton {
                id: WidgetId::explicit("skel_avatar"),
                width: Some(SKELETON_HEIGHT * 2.0),
                height: Some(SKELETON_HEIGHT * 2.0),
                circle: true,
                motion: Some(SkeletonMotion::Default),
            },
            Skeleton {
                id: WidgetId::explicit("skel1"),
                width: Some(SKELETON_WIDTH),
                height: Some(SKELETON_HEIGHT),
                circle: false,
                motion: Some(SkeletonMotion::Default),
            },
        ],
    }]
}

pub(crate) fn empty_state() -> Vec<Widget> {
    widgets![EmptyState {
        icon: None,
        title: "No items yet".into(),
        description: Some("Add your first item to get started.".into()),
        action: Some(
            Button {
                variant: ButtonVariant::Outline,
                child: Some(Text::new("Add Item").into()),
                ..Default::default()
            }
            .semantics_identifier("gallery.empty_state.add")
            .into(),
        ),
    }]
}
