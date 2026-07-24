use super::carousel_tile::CarouselTile;
use super::common::PreviewShell;
use crate::state::{current_composition_atoms, AnimationGalleryState, MotionAtom, MotionChoice};
use crate::style::{BLUE, TEAL, VIOLET};
use fission::motion::{px, Motion, MotionPropertyId, MotionStartValue, MotionTrack};
use fission::prelude::*;

const DEFAULT_OFFSET: f32 = 18.0;

pub(super) struct CarouselPreview<'a> {
    pub state: &'a AnimationGalleryState,
}

impl From<CarouselPreview<'_>> for Widget {
    fn from(preview: CarouselPreview<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let progress = if preview.state.playing {
            1.0
        } else {
            preview.state.scrub_ms as f32 / 300.0
        };
        let offset = carousel_offset(preview.state);

        PreviewShell {
            child: Row {
                gap: Some(view.env().theme.tokens.spacing.s),
                children: widgets![
                    CarouselTile {
                        label: "One",
                        background: BLUE,
                    },
                    Motion {
                        id: WidgetId::explicit("gallery.carousel.slide"),
                        tracks: vec![MotionTrack::composite(
                            MotionPropertyId::TranslateX,
                            MotionStartValue::Explicit(px(offset)),
                            px(offset * (1.0 - progress)),
                        )],
                        child: CarouselTile {
                            label: "Two",
                            background: TEAL,
                        }
                        .into(),
                        ..Default::default()
                    },
                    CarouselTile {
                        label: "Three",
                        background: VIOLET,
                    },
                ],
                ..Default::default()
            }
            .into(),
        }
        .into()
    }
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
