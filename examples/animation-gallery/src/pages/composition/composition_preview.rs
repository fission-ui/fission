use crate::state::{
    reset_timeline, AnimationGalleryState, MotionAtom, MotionPolicy, ResetTimeline,
};
use crate::style;
use crate::widgets::common::{preview_active, PreviewShell};
use fission::prelude::*;
use fission::widgets::{Modal, ModalAction, ModalMotion};

const MODAL_WIDTH: f32 = 400.0;

fission_component! {
    pub(super) CompositionPreview<AnimationGalleryState> |_preview, state, cx| {
        let close = cx.bind(ResetTimeline, reduce_with!(reset_timeline));
    } =>
        PreviewShell(
            Column(
                gap: cx.view.env().theme.tokens.spacing.s,
                children: [
                    Text(
                        "Real Modal widget using the ordered expression. Use playback to open it.",
                    )
                    .size(cx.view.env().theme.tokens.typography.font_size_sm)
                    .color(style::text_muted()),
                    Modal(
                        id: WidgetId::explicit("gallery.composition.modal"),
                        title: "Composed motion".to_string(),
                        content: Text("This modal is using the selected composition."),
                        is_open: preview_active(state),
                        on_dismiss: close.clone(),
                        close_semantics_identifier: "gallery.composition.modal.close".to_string(),
                        surface_semantics_identifier: "gallery.composition.modal".to_string(),
                        actions: vec![ModalAction {
                            label: "Close".into(),
                            on_press: Some(close.clone()),
                            is_primary: true,
                            semantics_identifier: Some("gallery.composition.modal.done".into()),
                        }],
                        width: MODAL_WIDTH,
                        motion: composed_modal_motion(state),
                    ),
                ],
            ),
        )
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
