use crate::state::{
    reset_timeline, AnimationGalleryState, MotionAtom, MotionPolicy, ResetTimeline,
};
use crate::style::MUTED;
use crate::widgets::common::{preview_active, PreviewShell};
use fission::build::BuildCtxHandle;
use fission::prelude::*;
use fission::widgets::{Modal, ModalAction, ModalMotion};

const MODAL_WIDTH: f32 = 400.0;

pub(super) struct CompositionPreview<'a> {
    pub ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    pub state: &'a AnimationGalleryState,
}

impl From<CompositionPreview<'_>> for Widget {
    fn from(preview: CompositionPreview<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let close = preview
            .ctx
            .bind(ResetTimeline, reduce_with!(reset_timeline));

        PreviewShell {
            child: Column {
                gap: Some(view.env().theme.tokens.spacing.s),
                children: widgets![
                    Text::new(
                        "Real Modal widget using the ordered expression. Use playback to open it.",
                    )
                    .size(view.env().theme.tokens.typography.font_size_sm)
                    .color(MUTED),
                    Modal {
                        id: WidgetId::explicit("gallery.composition.modal"),
                        title: "Composed motion".into(),
                        content: Text::new("This modal is using the selected composition.",).into(),
                        is_open: preview_active(preview.state),
                        on_dismiss: Some(close.clone()),
                        actions: vec![ModalAction {
                            label: "Close".into(),
                            on_press: Some(close),
                            is_primary: true,
                        }],
                        width: Some(MODAL_WIDTH),
                        motion: composed_modal_motion(preview.state),
                    },
                ],
                ..Default::default()
            }
            .into(),
        }
        .into()
    }
}

fn composed_modal_motion(state: &AnimationGalleryState) -> Option<ModalMotion> {
    if !preview_active(state)
        || state.composition_atoms.is_empty()
        || state.policy == MotionPolicy::Disabled
    {
        return None;
    }
    if state.policy == MotionPolicy::Reduced {
        return Some(ModalMotion::Fade);
    }

    let mut atoms = state.composition_atoms.iter().copied().map(atom_motion);
    let first = atoms.next()?;
    Some(atoms.fold(first, |acc, atom| acc + atom))
}

fn atom_motion(atom: MotionAtom) -> ModalMotion {
    match atom {
        MotionAtom::FromTop => ModalMotion::FromTop,
        MotionAtom::FromBottom => ModalMotion::FromBottom,
        MotionAtom::FromLeft => ModalMotion::FromLeft,
        MotionAtom::FromRight => ModalMotion::FromRight,
        MotionAtom::FromSide => ModalMotion::FromRight,
        MotionAtom::Fade => ModalMotion::Fade,
        MotionAtom::Scale | MotionAtom::OriginScale | MotionAtom::Pop => ModalMotion::Scale,
        MotionAtom::Collapse
        | MotionAtom::Chevron
        | MotionAtom::Indicator
        | MotionAtom::FadeContent
        | MotionAtom::SlideContent
        | MotionAtom::HoverScale
        | MotionAtom::PressScale
        | MotionAtom::Ripple
        | MotionAtom::Width => ModalMotion::Fade,
    }
}
