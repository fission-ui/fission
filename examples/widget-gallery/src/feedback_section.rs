use fission::prelude::*;
use fission::widgets::{
    Alert, AlertKind, CircularProgress, EmptyState, HStack, ProgressBar, Skeleton, SkeletonMotion,
    Spinner, SpinnerMotion,
};

const CIRCULAR_PROGRESS_SIZE: f32 = 40.0;
const SKELETON_WIDTH: f32 = 120.0;
const SKELETON_HEIGHT: f32 = 20.0;

pub(crate) fn alert() -> Vec<Widget> {
    widgets![AlertDemo::new()]
}

fission_component! {
    AlertDemo<()> |_demo, _state, _cx| {} =>
    Column(children: [
        Alert(
            kind: AlertKind::Info,
            title: "Information",
            description: Some("This is an info alert.".to_string()),
        ),
        Alert(
            kind: AlertKind::Success,
            title: "Success",
            description: None,
        ),
        Alert(
            kind: AlertKind::Warning,
            title: "Warning",
            description: Some("Be careful!".to_string()),
        ),
        Alert(
            kind: AlertKind::Error,
            title: "Error",
            description: Some("Something went wrong.".to_string()),
        ),
    ])
}

pub(crate) fn progress() -> Vec<Widget> {
    widgets![ProgressDemo::new()]
}

fission_component! {
    ProgressDemo<()> |_demo, _state, cx| { let tokens = &cx.view.env().theme.tokens; } =>
    Column(children: [
        ProgressBar(
            value: 0.65,
            label: "Upload progress".to_string(),
        ),
        HStack(
            spacing: tokens.spacing.m,
            children: [
                Spinner(
                    id: WidgetId::explicit("spinner1"),
                    label: None,
                    color: None,
                    motion: SpinnerMotion::Default,
                ),
                CircularProgress(value: 0.7_f32, size: CIRCULAR_PROGRESS_SIZE),
            ],
        ),
    ])
}

pub(crate) fn skeleton() -> Vec<Widget> {
    widgets![SkeletonDemo::new()]
}

fission_component! {
    SkeletonDemo<()> |_demo, _state, cx| { let tokens = &cx.view.env().theme.tokens; } =>
    HStack(
        spacing: tokens.spacing.m,
        children: [
            Skeleton(
                id: WidgetId::explicit("skel_avatar"),
                width: SKELETON_HEIGHT * 2.0,
                height: SKELETON_HEIGHT * 2.0,
                circle: true,
                motion: SkeletonMotion::Default,
            ),
            Skeleton(
                id: WidgetId::explicit("skel1"),
                width: SKELETON_WIDTH,
                height: SKELETON_HEIGHT,
                circle: false,
                motion: SkeletonMotion::Default,
            ),
        ],
    )
}

pub(crate) fn empty_state() -> Vec<Widget> {
    widgets![EmptyStateDemo::new()]
}

fission_component! {
    EmptyStateDemo<()> |_demo, _state, _cx| {} =>
    EmptyState(
        icon: None,
        title: "No items yet",
        description: Some("Add your first item to get started.".to_string()),
        action: Some(
            Button(
                variant: ButtonVariant::Outline,
                child: Text("Add Item"),
            )
            .semantics_identifier("gallery.empty_state.add")
        ),
    )
}
