use super::common::{policy_allows_motion, preview_active, preview_progress, PreviewShell};
use crate::state::{
    current_composition_atoms, reset_timeline, toggle_play, AnimationGalleryState, MotionAtom,
    MotionChoice, ResetTimeline, TogglePlay,
};
use fission::build::BuildCtxHandle;
use fission::motion::{scalar, Motion, MotionPropertyId, MotionStartValue, MotionTrack};
use fission::prelude::*;

pub(super) struct CheckboxPreview<'a> {
    pub ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    pub state: &'a AnimationGalleryState,
}

impl From<CheckboxPreview<'_>> for Widget {
    fn from(preview: CheckboxPreview<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let progress = preview_progress(preview.state);
        let on_toggle = if preview_active(preview.state) {
            preview
                .ctx
                .bind(ResetTimeline, reduce_with!(reset_timeline))
        } else {
            preview.ctx.bind(TogglePlay, reduce_with!(toggle_play))
        };

        PreviewShell {
            child: Column {
                gap: Some(view.env().theme.tokens.spacing.s),
                children: widgets![
                    Text::new("Real Checkbox widget wrapped in native Motion.")
                        .size(view.env().theme.tokens.typography.font_size_sm),
                    Motion {
                        id: WidgetId::explicit("gallery.checkbox.motion"),
                        tracks: if checkbox_scale_enabled(preview.state) {
                            vec![MotionTrack::composite(
                                MotionPropertyId::Scale,
                                MotionStartValue::Explicit(scalar(0.94)),
                                scalar(1.0 + progress * 0.04),
                            )]
                        } else {
                            Vec::new()
                        },
                        child: Checkbox {
                            id: Some(WidgetId::explicit("gallery.real.checkbox")),
                            semantics_identifier: Some("gallery.checkbox.accept_motion".into(),),
                            checked: preview_active(preview.state),
                            on_toggle: Some(on_toggle),
                            label: Some("Accept motion terms".into()),
                            disabled: false,
                        }
                        .into(),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }
            .into(),
        }
        .into()
    }
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
