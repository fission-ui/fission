use super::common::PreviewShell;
use crate::state::{
    current_composition_atoms, toggle_play, AnimationGalleryState, MotionAtom, MotionChoice,
    TogglePlay,
};
use fission::prelude::*;
use fission::widgets::ButtonMotion;

fission_component! {
    pub(super) ButtonPreview<AnimationGalleryState> {}
    |_preview, state, cx| {} =>
        PreviewShell(
            Button(
                id: WidgetId::explicit("gallery.preview.button"),
                variant: ButtonVariant::Filled,
                child: Text("Send"),
                on_press: cx.bind(TogglePlay, reduce_with!(toggle_play)),
                motion: match state.motion {
                    MotionChoice::None => None,
                    MotionChoice::Default => Some(ButtonMotion::Default),
                    MotionChoice::Scale => Some(ButtonMotion::HoverPressScale),
                    MotionChoice::Composition => {
                        compose_button_motion(current_composition_atoms(state))
                    },
                    _ => Some(ButtonMotion::HoverPressRipple),
                },
            ),
        )
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
