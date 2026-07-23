use super::common::{policy_allows_motion, preview_active, PreviewShell};
use crate::state::{
    current_composition_atoms, reset_timeline, toggle_play, AnimationGalleryState, MotionAtom,
    MotionChoice, MotionPolicy, ResetTimeline, TogglePlay,
};
use crate::style::MUTED;
use crate::ui;
use fission::build::BuildCtxHandle;
use fission::prelude::*;
use fission::widgets::{Modal, ModalAction, ModalMotion};

const MODAL_WIDTH: f32 = 420.0;

pub(super) struct ModalPreview<'a> {
    pub ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    pub state: &'a AnimationGalleryState,
}

impl From<ModalPreview<'_>> for Widget {
    fn from(preview: ModalPreview<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let close = preview
            .ctx
            .bind(ResetTimeline, reduce_with!(reset_timeline));

        PreviewShell {
            child: Column {
                gap: Some(view.env().theme.tokens.spacing.s),
                children: widgets![
                    Text::new(
                        "Real Modal widget. Use the playback control to mount its portal and run enter motion.",
                    )
                    .size(view.env().theme.tokens.typography.font_size_sm)
                    .color(MUTED),
                    ui::SmallButton {
                        ctx: preview.ctx,
                        label: "Open real modal",
                        action: TogglePlay,
                        reducer: toggle_play,
                    },
                    Modal {
                        id: WidgetId::explicit("gallery.real.modal"),
                        title: "Archive thread".into(),
                        content: Text::new(
                            "This is the actual Modal widget using the selected motion.",
                        )
                        .into(),
                        is_open: preview_active(preview.state),
                        on_dismiss: Some(close.clone()),
                        actions: vec![
                            ModalAction {
                                label: "Cancel".into(),
                                on_press: Some(close.clone()),
                                is_primary: false,
                            },
                            ModalAction {
                                label: "Confirm".into(),
                                on_press: Some(close),
                                is_primary: true,
                            },
                        ],
                        width: Some(MODAL_WIDTH),
                        motion: preview_active(preview.state)
                            .then(|| modal_motion(preview.state))
                            .flatten(),
                    },
                ],
                ..Default::default()
            }
            .into(),
        }
        .into()
    }
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
