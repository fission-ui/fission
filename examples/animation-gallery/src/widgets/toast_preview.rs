use super::common::{policy_allows_motion, preview_active, PreviewShell};
use crate::state::{
    current_composition_atoms, reset_timeline, AnimationGalleryState, MotionAtom, MotionChoice,
    MotionPolicy, ResetTimeline,
};
use crate::style::MUTED;
use crate::ui;
use fission::build::BuildCtxHandle;
use fission::prelude::*;
use fission::widgets::{Toast, ToastKind, ToastMotion};

pub(super) struct ToastPreview<'a> {
    pub ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    pub state: &'a AnimationGalleryState,
}

impl From<ToastPreview<'_>> for Widget {
    fn from(preview: ToastPreview<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let tokens = &view.env().theme.tokens;
        let close = preview
            .ctx
            .bind(ResetTimeline, reduce_with!(reset_timeline));
        let toast: Widget = if preview_active(preview.state) {
            Toast {
                id: WidgetId::explicit("gallery.real.toast"),
                kind: ToastKind::Success,
                message: "Saved changes with real Toast motion.".into(),
                on_close: Some(close),
                motion: toast_motion(preview.state),
            }
            .into()
        } else {
            Text::new("Use the playback control to mount the real Toast widget.")
                .size(tokens.typography.font_size_sm)
                .color(MUTED)
                .into()
        };

        PreviewShell {
            child: Column {
                gap: Some(tokens.spacing.s),
                children: widgets![
                    Text::new("Actual Toast widget; app state controls its lifetime.")
                        .size(tokens.typography.font_size_sm)
                        .color(MUTED),
                    toast,
                    if preview_active(preview.state) {
                        Widget::from(ui::SmallButton {
                            ctx: preview.ctx,
                            label: "Dismiss toast",
                            action: ResetTimeline,
                            reducer: reset_timeline,
                        })
                    } else {
                        Widget::from(
                            Text::new("The toast close action is wired to application state.")
                                .size(tokens.typography.font_size_sm)
                                .color(MUTED),
                        )
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
