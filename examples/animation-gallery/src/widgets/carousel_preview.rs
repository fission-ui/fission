use super::carousel_tile::CarouselTile;
use super::common::PreviewShell;
use crate::state::{current_composition_atoms, AnimationGalleryState, MotionAtom, MotionChoice};
use crate::style;
use fission::motion::{px, Motion, MotionPropertyId, MotionStartValue, MotionTrack};
use fission::prelude::*;

const DEFAULT_OFFSET: f32 = 18.0;

fission_component! {
    pub(super) CarouselPreview<AnimationGalleryState> {}
    |_preview, state, cx| {
        let progress = if state.playing {
            1.0
        } else {
            state.scrub_ms as f32 / 300.0
        };
        let offset = carousel_offset(state);
    } =>
        PreviewShell(
            Row(
                gap: cx.view.env().theme.tokens.spacing.s,
                children: [
                    CarouselTile("One".to_string(), style::primary()),
                    Motion(
                        id: WidgetId::explicit("gallery.carousel.slide"),
                        tracks: vec![MotionTrack::composite(
                            MotionPropertyId::TranslateX,
                            MotionStartValue::Explicit(px(offset)),
                            px(offset * (1.0 - progress)),
                        )],
                        child: CarouselTile("Two".to_string(), style::success()),
                    ),
                    CarouselTile("Three".to_string(), style::secondary()),
                ],
            ),
        )
}

fn carousel_offset(state: &AnimationGalleryState) -> f32 {
    if state.motion != MotionChoice::Composition {
        return DEFAULT_OFFSET;
    }
    let atoms = current_composition_atoms(state);
    if atoms
        .iter()
        .any(|atom| matches!(atom, MotionAtom::FromLeft))
    {
        -DEFAULT_OFFSET
    } else if atoms
        .iter()
        .any(|atom| matches!(atom, MotionAtom::FromRight))
    {
        DEFAULT_OFFSET
    } else {
        0.0
    }
}
