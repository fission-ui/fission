use super::common::PreviewShell;
use crate::state::{
    current_composition_atoms, toggle_play, AnimationGalleryState, MotionAtom, MotionChoice,
    TogglePlay,
};
use fission::prelude::*;
use fission::widgets::{Accordion, AccordionItem, AccordionMotion};

fission_component! {
    pub(super) AccordionPreview<AnimationGalleryState> {}
    |_preview, state, cx| {
        let progress = if state.playing {
            1.0
        } else {
            state.scrub_ms as f32 / 300.0
        };
    } =>
        PreviewShell(
            Accordion(
                items: vec![AccordionItem {
                    title: "Motion details".into(),
                    content: Text::new(
                        "Panel height, opacity, and indicator rotation are inspectable.",
                    )
                    .into(),
                    is_expanded: progress > 0.2,
                    on_toggle: Some(cx.bind(TogglePlay, reduce_with!(toggle_play))),
                }],
                motion: match state.motion {
                    MotionChoice::None => None,
                    MotionChoice::Composition => Some(
                        compose_accordion_motion(current_composition_atoms(state))
                            .unwrap_or(AccordionMotion::Default),
                    ),
                    _ => Some(AccordionMotion::Default),
                },
            ),
        )
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
