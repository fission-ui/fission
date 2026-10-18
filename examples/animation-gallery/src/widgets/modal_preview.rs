use super::common::{policy_allows_motion, preview_active, PreviewShell};
use crate::state::{
    current_composition_atoms, reset_timeline, toggle_play, AnimationGalleryState, MotionAtom,
    MotionChoice, MotionPolicy, ResetTimeline, TogglePlay,
};
use crate::style;
use crate::ui::*;
use fission::prelude::*;
use fission::widgets::{Modal, ModalAction, ModalMotion};

const MODAL_WIDTH: f32 = 420.0;

fission_component! {
    pub(super) ModalPreview<AnimationGalleryState> {}
    |_preview, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let close = cx.bind(ResetTimeline, reduce_with!(reset_timeline));
    } =>
        PreviewShell(
            Column(
                gap: tokens.spacing.s,
                children: [
                    Text(
                        "Real Modal widget. Use the playback control to mount its portal and run enter motion.",
                    )
                    .size(tokens.typography.font_size_sm)
                    .color(style::text_muted()),
                    SmallButton("Open real modal".to_string(), cx.bind(TogglePlay, reduce_with!(toggle_play))),
                    Modal(
                        id: WidgetId::explicit("gallery.real.modal"),
                        title: "Archive thread".to_string(),
                        content: Text(
                            "This is the actual Modal widget using the selected motion.",
                        ),
                        is_open: preview_active(state),
                        on_dismiss: close.clone(),
                        backdrop_semantics_identifier: "gallery.real.modal.backdrop".to_string(),
                        close_semantics_identifier: "gallery.real.modal.close".to_string(),
                        surface_semantics_identifier: "gallery.real.modal.surface".to_string(),
                        actions: vec![
                            ModalAction {
                                label: "Cancel".into(),
                                on_press: Some(close.clone()),
                                is_primary: false,
                                semantics_identifier: Some("gallery.real.modal.cancel".into()),
                            },
                            ModalAction {
                                label: "Confirm".into(),
                                on_press: Some(close.clone()),
                                is_primary: true,
                                semantics_identifier: Some("gallery.real.modal.confirm".into()),
                            },
                        ],
                        width: MODAL_WIDTH,
                        motion: preview_active(state)
                            .then(|| modal_motion(state))
                            .flatten(),
                    ),
                ],
            ),
        )
}

fn modal_motion(state: &AnimationGalleryState) -> Option<ModalMotion> {
    if !policy_allows_motion(state) {
        return None;
    }
    if state.policy == MotionPolicy::Reduced {
        return Some(ModalMotion::Fade);
    }
    match state.motion {
        MotionChoice::None => None,
        MotionChoice::Default => Some(ModalMotion::Default),
        MotionChoice::Fade => Some(ModalMotion::Fade),
        MotionChoice::Scale => Some(ModalMotion::Scale),
        MotionChoice::Directional => Some(ModalMotion::FromTop),
        MotionChoice::Composition => compose_modal_motion(current_composition_atoms(state)),
    }
}

fn compose_modal_motion(atoms: &[MotionAtom]) -> Option<ModalMotion> {
    let mut motions = atoms.iter().copied().filter_map(|atom| match atom {
        MotionAtom::FromTop => Some(ModalMotion::FromTop),
        MotionAtom::FromBottom => Some(ModalMotion::FromBottom),
        MotionAtom::FromLeft => Some(ModalMotion::FromLeft),
        MotionAtom::FromRight | MotionAtom::FromSide => Some(ModalMotion::FromRight),
        MotionAtom::Fade => Some(ModalMotion::Fade),
        MotionAtom::Scale | MotionAtom::OriginScale | MotionAtom::Pop => Some(ModalMotion::Scale),
        _ => None,
    });
    let first = motions.next()?;
    Some(motions.fold(first, |acc, motion| acc + motion))
}
