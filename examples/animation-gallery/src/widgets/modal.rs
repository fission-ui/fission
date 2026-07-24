use super::common::*;
use super::modal_preview::ModalPreview;
use crate::state::{AnimationGalleryState, MotionChoice};
use crate::style::SOFT_VIOLET;
use fission::build::BuildCtxHandle;
use fission::Widget;

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
