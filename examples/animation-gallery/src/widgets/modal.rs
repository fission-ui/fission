use super::common::*;
use crate::state::{
    current_composition_atoms, reset_timeline, toggle_play, AnimationGalleryState, MotionAtom,
    MotionChoice, MotionPolicy, ResetTimeline, TogglePlay,
};
use crate::style::{MUTED, SOFT_VIOLET};
use crate::ui;
use fission::build::BuildCtxHandle;
use fission::widgets::{Modal, ModalAction, ModalMotion};
use fission::{Column, Text, Widget, WidgetId};

pub const PATH: &str = "/widgets/modal";

pub const SUMMARY: WidgetSummary = WidgetSummary {
    path: PATH,
    title: "Modal",
    subtitle: "6 motions",
    glyph: "window",
    tint: SOFT_VIOLET,
};

pub struct ModalPage<'a> {
    pub ctx: BuildCtxHandle<AnimationGalleryState>,
    pub state: &'a AnimationGalleryState,
}

impl From<ModalPage<'_>> for Widget {
    fn from(page: ModalPage<'_>) -> Self {
        WidgetPage {
            ctx: &page.ctx,
            state: page.state,
            case: case(page.state.motion),
            preview: ModalPreview {
                ctx: &page.ctx,
                state: page.state,
            }
            .into(),
        }
        .into()
    }
}

struct ModalPreview<'a> {
    ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    state: &'a AnimationGalleryState,
}

impl From<ModalPreview<'_>> for Widget {
    fn from(preview: ModalPreview<'_>) -> Self {
        let ctx = preview.ctx;
        let state = preview.state;
        let close = ctx.bind(ResetTimeline, fission::reduce_with!(reset_timeline));
        PreviewShell {
            child: Column {
                gap: Some(12.0),
                children: vec![
                    Text::new(
                        "Real Modal widget. Use the playback control to mount its portal and run enter motion.",
                    )
                    .size(12.0)
                    .color(MUTED)
                    .into(),
                    ui::SmallButton {
                        ctx,
                        label: "Open real modal",
                        action: TogglePlay,
                        reducer: toggle_play,
                    }
                    .into(),
                    Modal {
                        id: WidgetId::explicit("gallery.real.modal"),
                        title: "Archive thread".into(),
                        content: Text::new(
                            "This is the actual Modal widget using the selected motion.",
                        )
                        .into(),
                        is_open: preview_active(state),
                        on_dismiss: Some(close.clone()),
                        actions: vec![
                            ModalAction {
                                label: "Cancel".into(),
                                on_press: Some(close.clone()),
                                is_primary: false,
                            },
                            ModalAction {
                                label: "Confirm".into(),
                                on_press: Some(close),
                                is_primary: true,
                            },
                        ],
                        width: Some(420.0),
                        motion: preview_active(state).then(|| modal_motion(state)).flatten(),
                    }
                    .into(),
                ],
                ..Default::default()
            }
            .into(),
        }
        .into()
    }
}

fn modal_motion(state: &AnimationGalleryState) -> Option<ModalMotion> {
    if !policy_allows_motion(state) {
        return None;
    }
    if state.policy == MotionPolicy::Reduced {
        return Some(ModalMotion::Fade);
    }
    match state.motion {
        MotionChoice::None => None,
        MotionChoice::Default => Some(ModalMotion::Default),
        MotionChoice::Fade => Some(ModalMotion::Fade),
        MotionChoice::Scale => Some(ModalMotion::Scale),
        MotionChoice::Directional => Some(ModalMotion::FromTop),
        MotionChoice::Composition => compose_modal_motion(current_composition_atoms(state)),
    }
}

fn compose_modal_motion(atoms: &[MotionAtom]) -> Option<ModalMotion> {
    let mut motions = atoms.iter().copied().filter_map(|atom| match atom {
        MotionAtom::FromTop => Some(ModalMotion::FromTop),
        MotionAtom::FromBottom => Some(ModalMotion::FromBottom),
        MotionAtom::FromLeft => Some(ModalMotion::FromLeft),
        MotionAtom::FromRight | MotionAtom::FromSide => Some(ModalMotion::FromRight),
        MotionAtom::Fade => Some(ModalMotion::Fade),
        MotionAtom::Scale | MotionAtom::OriginScale | MotionAtom::Pop => Some(ModalMotion::Scale),
        _ => None,
    });
    let first = motions.next()?;
    Some(motions.fold(first, |acc, motion| acc + motion))
}

fn case(motion: MotionChoice) -> GalleryCase {
    GalleryCase {
        title: "Modal",
        description: "Explicit motion API for modal entrance, backdrop, and surface slots.",
        motions: MODAL_MOTIONS,
        slots: &["backdrop", "surface"],
        tracks: match motion {
            MotionChoice::None => &[],
            MotionChoice::Composition => &[
                "surface.translate_y",
                "surface.opacity",
                "surface.scale",
                "backdrop.opacity",
            ],
            _ => &["surface.opacity", "surface.scale", "backdrop.opacity"],
        },
        exprs: match motion {
            MotionChoice::None => &["No MotionDeclaration emitted"],
            MotionChoice::Composition => &[
                "px(-24) -> px(0)",
                "scalar(0) -> scalar(1)",
                "scalar(0.96) -> scalar(1)",
            ],
            _ => &["scalar(0) -> scalar(1)", "MotionTransition::tween"],
        },
        ergonomic_source: match motion {
            MotionChoice::None => MODAL_NONE_SOURCE,
            MotionChoice::Composition => MODAL_COMPOSED_SOURCE,
            _ => MODAL_DEFAULT_SOURCE,
        },
        native_source: MODAL_NATIVE_SOURCE,
        declaration_source: MODAL_DECLARATION_SOURCE,
        test_source: TEST_SOURCE,
        diagnostic: match motion {
            MotionChoice::None => "motion: None emits no modal-owned MotionDeclaration.",
            MotionChoice::Default => {
                "Default is explicit opt-in. It is not Rust Default::default()."
            }
            MotionChoice::Composition => {
                "FromTop, Fade, and Scale compose because they target distinct tracks."
            }
            _ => "Lowered modal tracks are deterministic runtime data.",
        },
    }
}

const MODAL_NONE_SOURCE: &str = r#"Modal {
    id: WidgetId::explicit("gallery_modal"),
    title: "Archive thread".into(),
    content: Text::new("This action can be undone from the archive.").into(),
    is_open: view.state().modal_open,
    on_dismiss: Some(close_modal),
    motion: None,
    ..Default::default()
}.into()"#;

const MODAL_DEFAULT_SOURCE: &str = r#"Modal {
    id: WidgetId::explicit("gallery_modal"),
    title: "Archive thread".into(),
    motion: Some(ModalMotion::Default), // explicit opt-in
    ..Default::default()
}.into()"#;

const MODAL_COMPOSED_SOURCE: &str = r#"Modal {
    id: WidgetId::explicit("gallery_modal"),
    title: "Archive thread".into(),
    motion: Some(
        ModalMotion::FromTop + ModalMotion::Fade + ModalMotion::Scale,
    ),
    ..Default::default()
}.into()"#;

const MODAL_NATIVE_SOURCE: &str = r#"Presence {
    id: WidgetId::explicit("gallery_modal.surface"),
    visible: view.state().modal_open,
    enter: vec![surface.translate_y, surface.opacity, surface.scale],
    exit: reverse_tracks_for_exit(&enter),
    child: modal_surface,
    ..Default::default()
}.into()"#;

const MODAL_DECLARATION_SOURCE: &str = r#"MotionDeclaration {
    id: WidgetId::derived(gallery_modal, [surface]),
    kind: MotionDeclarationKind::Presence {
        visible: true,
        enter: vec![surface.translate_y, surface.opacity, surface.scale],
        exit: reverse_tracks_for_exit(&enter),
        keep_rendered: false,
        inert_while_exiting: true,
    },
}"#;
