use super::common::PreviewShell;
use crate::state::{
    current_composition_atoms, toggle_play, AnimationGalleryState, MotionAtom, MotionChoice,
    TogglePlay,
};
use fission::build::BuildCtxHandle;
use fission::prelude::*;
use fission::widgets::ButtonMotion;

pub(super) struct ButtonPreview<'a> {
    pub ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    pub state: &'a AnimationGalleryState,
}

impl From<ButtonPreview<'_>> for Widget {
    fn from(preview: ButtonPreview<'_>) -> Self {
        PreviewShell {
            child: Button {
                id: Some(WidgetId::explicit("gallery.preview.button")),
                variant: ButtonVariant::Filled,
                child: Some(Text::new("Send").into()),
                on_press: Some(preview.ctx.bind(TogglePlay, reduce_with!(toggle_play))),
                motion: match preview.state.motion {
                    MotionChoice::None => None,
                    MotionChoice::Default => Some(ButtonMotion::Default),
                    MotionChoice::Scale => Some(ButtonMotion::HoverPressScale),
                    MotionChoice::Composition => {
                        compose_button_motion(current_composition_atoms(preview.state))
                    }
                    _ => Some(ButtonMotion::HoverPressRipple),
                },
                ..Default::default()
            }
            .into(),
        }
        .into()
    }
}

fn compose_button_motion(atoms: &[MotionAtom]) -> Option<ButtonMotion> {
    let mut motions = atoms.iter().copied().filter_map(|atom| match atom {
        MotionAtom::HoverScale | MotionAtom::Scale => Some(ButtonMotion::HoverScale),
        MotionAtom::PressScale => Some(ButtonMotion::PressScale),
        MotionAtom::Ripple => Some(ButtonMotion::Ripple),
        _ => None,
    });
    let first = motions.next()?;
    Some(motions.fold(first, |acc, motion| acc + motion))
}
