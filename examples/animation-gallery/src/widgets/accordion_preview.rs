use super::common::PreviewShell;
use crate::state::{
    current_composition_atoms, toggle_play, AnimationGalleryState, MotionAtom, MotionChoice,
    TogglePlay,
};
use fission::build::BuildCtxHandle;
use fission::prelude::*;
use fission::widgets::{Accordion, AccordionItem, AccordionMotion};

pub(super) struct AccordionPreview<'a> {
    pub ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    pub state: &'a AnimationGalleryState,
}

impl From<AccordionPreview<'_>> for Widget {
    fn from(preview: AccordionPreview<'_>) -> Self {
        let progress = if preview.state.playing {
            1.0
        } else {
            preview.state.scrub_ms as f32 / 300.0
        };

        PreviewShell {
            child: Accordion {
                items: vec![AccordionItem {
                    title: "Motion details".into(),
                    content: Text::new(
                        "Panel height, opacity, and indicator rotation are inspectable.",
                    )
                    .into(),
                    is_expanded: progress > 0.2,
                    on_toggle: Some(preview.ctx.bind(TogglePlay, reduce_with!(toggle_play))),
                }],
                motion: match preview.state.motion {
                    MotionChoice::None => None,
                    MotionChoice::Composition => Some(
                        compose_accordion_motion(current_composition_atoms(preview.state))
                            .unwrap_or(AccordionMotion::Default),
                    ),
                    _ => Some(AccordionMotion::Default),
                },
            }
            .into(),
        }
        .into()
    }
}

fn compose_accordion_motion(atoms: &[MotionAtom]) -> Option<AccordionMotion> {
    let mut motions = atoms.iter().copied().filter_map(|atom| match atom {
        MotionAtom::Collapse => Some(AccordionMotion::Collapse),
        MotionAtom::Fade => Some(AccordionMotion::Fade),
        MotionAtom::Chevron => Some(AccordionMotion::Chevron),
        _ => None,
    });
    let first = motions.next()?;
    Some(motions.fold(first, |acc, motion| acc + motion))
}
