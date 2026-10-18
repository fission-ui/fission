use super::common::{policy_allows_motion, preview_active, preview_progress, PreviewShell};
use crate::state::{
    current_composition_atoms, reset_timeline, toggle_play, AnimationGalleryState, MotionAtom,
    MotionChoice, ResetTimeline, TogglePlay,
};
use fission::motion::{scalar, Motion, MotionPropertyId, MotionStartValue, MotionTrack};
use fission::prelude::*;

fission_component! {
    pub(super) CheckboxPreview<AnimationGalleryState> {}
    |_preview, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let progress = preview_progress(state);
        let on_toggle = if preview_active(state) {
            cx.bind(ResetTimeline, reduce_with!(reset_timeline))
        } else {
            cx.bind(TogglePlay, reduce_with!(toggle_play))
        };
    } =>
        PreviewShell(
            Column(
                gap: tokens.spacing.s,
                children: [
                    Text("Real Checkbox widget wrapped in native Motion.").size(tokens.typography.font_size_sm),
                    Motion(
                        id: WidgetId::explicit("gallery.checkbox.motion"),
                        tracks: if checkbox_scale_enabled(state) {
                            vec![MotionTrack::composite(
                                MotionPropertyId::Scale,
                                MotionStartValue::Explicit(scalar(0.94)),
                                scalar(1.0 + progress * 0.04),
                            )]
                        } else {
                            Vec::new()
                        },
                        child: Checkbox(
                            id: WidgetId::explicit("gallery.real.checkbox"),
                            semantics_identifier: "gallery.checkbox.accept_motion".to_string(),
                            checked: preview_active(state),
                            on_toggle: on_toggle,
                            label: Some("Accept motion terms".into()),
                            disabled: false,
                        ),
                    ),
                ],
            ),
        )
}

fn checkbox_scale_enabled(state: &AnimationGalleryState) -> bool {
    if !policy_allows_motion(state) {
        return false;
    }
    state.motion != MotionChoice::Composition
        || current_composition_atoms(state).iter().any(|atom| {
            matches!(
                atom,
                MotionAtom::Scale | MotionAtom::HoverScale | MotionAtom::PressScale
            )
        })
}
