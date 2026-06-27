use super::common::*;
use crate::state::{
    current_composition_atoms, reset_timeline, AnimationGalleryState, MotionAtom, MotionChoice,
    MotionPolicy, ResetTimeline,
};
use crate::style::{MUTED, SOFT_TEAL};
use crate::ui;
use fission::build::BuildCtxHandle;
use fission::widgets::{Toast, ToastKind, ToastMotion};
use fission::{Column, Text, Widget, WidgetId};

pub const PATH: &str = "/widgets/toast";

pub const SUMMARY: WidgetSummary = WidgetSummary {
    path: PATH,
    title: "Toast",
    subtitle: "5 motions",
    glyph: "toast",
    tint: SOFT_TEAL,
};

pub struct ToastPage<'a> {
    pub ctx: BuildCtxHandle<AnimationGalleryState>,
    pub state: &'a AnimationGalleryState,
}

impl From<ToastPage<'_>> for Widget {
    fn from(page: ToastPage<'_>) -> Self {
        WidgetPage {
            ctx: &page.ctx,
            state: page.state,
            case: case(),
            preview: ToastPreview {
                ctx: &page.ctx,
                state: page.state,
            }
            .into(),
        }
        .into()
    }
}

struct ToastPreview<'a> {
    ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    state: &'a AnimationGalleryState,
}

impl From<ToastPreview<'_>> for Widget {
    fn from(preview: ToastPreview<'_>) -> Self {
        let state = preview.state;
        let close = preview
            .ctx
            .bind(ResetTimeline, fission::reduce_with!(reset_timeline));
        let toast: Widget = if preview_active(state) {
            Toast {
                id: WidgetId::explicit("gallery.real.toast"),
                kind: ToastKind::Success,
                message: "Saved changes with real Toast motion.".into(),
                on_close: Some(close),
                motion: toast_motion(state),
            }
            .into()
        } else {
            Text::new("Use the playback control to mount the real Toast widget.")
                .size(12.0)
                .color(MUTED)
                .into()
        };

        PreviewShell {
            child: Column {
                gap: Some(12.0),
                children: vec![
                    Text::new("Actual Toast widget; app state controls its lifetime.")
                        .size(12.0)
                        .color(MUTED)
                        .into(),
                    toast,
                    if preview_active(state) {
                        ui::SmallButton {
                            ctx: preview.ctx,
                            label: "Dismiss toast",
                            action: ResetTimeline,
                            reducer: reset_timeline,
                        }
                        .into()
                    } else {
                        Text::new("The toast close action is wired to application state.")
                            .size(11.0)
                            .color(MUTED)
                            .into()
                    },
                ],
                ..Default::default()
            }
            .into(),
        }
        .into()
    }
}

fn toast_motion(state: &AnimationGalleryState) -> Option<ToastMotion> {
    if !policy_allows_motion(state) {
        return None;
    }
    if state.policy == MotionPolicy::Reduced {
        return Some(ToastMotion::Fade);
    }
    match state.motion {
        MotionChoice::None => None,
        MotionChoice::Default => Some(ToastMotion::Default),
        MotionChoice::Fade => Some(ToastMotion::Fade),
        MotionChoice::Scale => Some(ToastMotion::Pop),
        MotionChoice::Directional => Some(ToastMotion::SlideFromTop),
        MotionChoice::Composition => compose_toast_motion(current_composition_atoms(state)),
    }
}

fn compose_toast_motion(atoms: &[MotionAtom]) -> Option<ToastMotion> {
    let mut motions = atoms.iter().copied().filter_map(|atom| match atom {
        MotionAtom::FromTop => Some(ToastMotion::SlideFromTop),
        MotionAtom::FromBottom => Some(ToastMotion::SlideFromBottom),
        MotionAtom::Fade => Some(ToastMotion::Fade),
        MotionAtom::Pop | MotionAtom::Scale => Some(ToastMotion::Pop),
        _ => None,
    });
    let first = motions.next()?;
    Some(motions.fold(first, |acc, motion| acc + motion))
}

fn case() -> GalleryCase {
    GalleryCase {
        title: "Toast",
        description: "Presence-driven notification motion for surface enter and exit.",
        motions: DIRECTIONAL_MOTIONS,
        slots: &["surface"],
        tracks: &["surface.translate_y", "surface.opacity"],
        exprs: &["px(-18) -> px(0)", "scalar(0) -> scalar(1)"],
        ergonomic_source: SOURCE,
        native_source: GENERIC_NATIVE_SOURCE,
        declaration_source: GENERIC_DECLARATION_SOURCE,
        test_source: TEST_SOURCE,
        diagnostic: "Toast motion is presence-based; app code owns lifetime and dismissal.",
    }
}

const SOURCE: &str = r#"Toast {
    id: WidgetId::explicit("saved_toast"),
    kind: ToastKind::Success,
    message: "Saved".into(),
    motion: Some(ToastMotion::SlideFromTop + ToastMotion::Fade),
    ..toast
}.into()"#;
