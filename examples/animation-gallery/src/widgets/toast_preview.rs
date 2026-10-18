use super::common::{policy_allows_motion, preview_active, PreviewShell};
use crate::state::{
    current_composition_atoms, reset_timeline, AnimationGalleryState, MotionAtom, MotionChoice,
    MotionPolicy, ResetTimeline,
};
use crate::style;
use crate::ui::*;
use fission::prelude::*;
use fission::widgets::{Toast, ToastKind, ToastMotion};

fission_component! {
    pub(super) ToastPreview<AnimationGalleryState> {}
    |_preview, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        PreviewShell(
            Column(
                gap: tokens.spacing.s,
                children: [
                    Text("Actual Toast widget; app state controls its lifetime.")
                        .size(tokens.typography.font_size_sm)
                        .color(style::text_muted()),
                    if preview_active(state) {
                        Toast(
                            id: WidgetId::explicit("gallery.real.toast"),
                            kind: ToastKind::Success,
                            message: "Saved changes with real Toast motion.",
                            on_close: cx.bind(ResetTimeline, reduce_with!(reset_timeline)),
                            duration: fission::widgets::ToastDuration::Default,
                            motion: toast_motion(state),
                        )
                    } else {
                        Text("Use the playback control to mount the real Toast widget.")
                            .size(tokens.typography.font_size_sm)
                            .color(style::text_muted())
                    },
                    if preview_active(state) {
                        SmallButton("Dismiss toast".to_string(), cx.bind(ResetTimeline, reduce_with!(reset_timeline)))
                    } else {
                        Text("The toast close action is wired to application state.")
                                .size(tokens.typography.font_size_sm)
                                .color(style::text_muted())
                    },
                ],
            ),
        )
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
