use super::common::{policy_allows_motion, preview_active, PreviewShell};
use crate::state::{
    current_composition_atoms, AnimationGalleryState, MotionAtom, MotionChoice, MotionPolicy,
};
use crate::style::MUTED;
use fission::prelude::*;
use fission::widgets::{Tooltip, TooltipMotion};

pub(super) struct TooltipPreview<'a> {
    pub state: &'a AnimationGalleryState,
}

impl From<TooltipPreview<'_>> for Widget {
    fn from(preview: TooltipPreview<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let tokens = &view.env().theme.tokens;

        PreviewShell {
            child: Column {
                gap: Some(tokens.spacing.s),
                children: widgets![
                    Text::new("Real Tooltip widget. Play forces it visible; hover also works.",)
                        .size(tokens.typography.font_size_sm)
                        .color(MUTED),
                    Tooltip {
                        id: WidgetId::explicit("gallery.real.tooltip"),
                        child: Button {
                            variant: ButtonVariant::Outline,
                            child: Some(Text::new("Save").into()),
                            ..Default::default()
                        }
                        .into(),
                        text: "Saved locally with deterministic motion.".into(),
                        is_visible: preview_active(preview.state),
                        motion: preview_active(preview.state)
                            .then(|| tooltip_motion(preview.state))
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

fn tooltip_motion(state: &AnimationGalleryState) -> Option<TooltipMotion> {
    if !policy_allows_motion(state) {
        return None;
    }
    if state.policy == MotionPolicy::Reduced {
        return Some(TooltipMotion::Fade);
    }
    match state.motion {
        MotionChoice::None => None,
        MotionChoice::Default => Some(TooltipMotion::Default),
        MotionChoice::Fade => Some(TooltipMotion::Fade),
        MotionChoice::Scale => Some(TooltipMotion::Scale),
        MotionChoice::Composition => compose_tooltip_motion(current_composition_atoms(state)),
        MotionChoice::Directional => Some(TooltipMotion::FadeAndSlide),
    }
}

fn compose_tooltip_motion(atoms: &[MotionAtom]) -> Option<TooltipMotion> {
    let mut motions = atoms.iter().copied().filter_map(|atom| match atom {
        MotionAtom::Fade => Some(TooltipMotion::Fade),
        MotionAtom::Scale => Some(TooltipMotion::Scale),
        MotionAtom::FromTop => Some(TooltipMotion::FadeAndSlide),
        _ => None,
    });
    let first = motions.next()?;
    Some(motions.fold(first, |acc, motion| acc + motion))
}
