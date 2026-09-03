use super::common::{policy_allows_motion, preview_active, preview_progress, PreviewShell};
use crate::state::{
    current_composition_atoms, reset_timeline, toggle_play, AnimationGalleryState, MotionAtom,
    MotionChoice, ResetTimeline, TogglePlay,
};
use fission::build::BuildCtxHandle;
use fission::motion::{scalar, Motion, MotionPropertyId, MotionStartValue, MotionTrack};
use fission::prelude::*;

pub(super) struct SwitchPreview<'a> {
    pub ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    pub state: &'a AnimationGalleryState,
}

impl From<SwitchPreview<'_>> for Widget {
    fn from(preview: SwitchPreview<'_>) -> Self {
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
                    Text::new("Real Switch widget wrapped in native Motion.")
                        .size(view.env().theme.tokens.typography.font_size_sm),
                    Motion {
                        id: WidgetId::explicit("gallery.switch.motion"),
                        tracks: if switch_scale_enabled(preview.state) {
                            vec![MotionTrack::composite(
                                MotionPropertyId::Scale,
                                MotionStartValue::Explicit(scalar(0.94)),
                                scalar(1.0 + progress * 0.04),
                            )]
                        } else {
                            Vec::new()
                        },
                        child: Switch {
                            id: Some(WidgetId::explicit("gallery.real.switch")),
                            semantics_identifier: Some("gallery.switch.sync_preview".into(),),
                            checked: preview_active(preview.state),
                            on_toggle: Some(on_toggle),
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

fn switch_scale_enabled(state: &AnimationGalleryState) -> bool {
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
