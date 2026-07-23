use super::common::{policy_allows_motion, preview_active, PreviewShell};
use super::popover_menu::PopoverMenu;
use crate::state::{
    current_composition_atoms, reset_timeline, AnimationGalleryState, MotionAtom, MotionChoice,
    MotionPolicy, ResetTimeline,
};
use crate::style::MUTED;
use fission::build::BuildCtxHandle;
use fission::prelude::*;
use fission::widgets::{Popover, PopoverMotion};

pub(super) struct PopoverPreview<'a> {
    pub ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    pub state: &'a AnimationGalleryState,
}

impl From<PopoverPreview<'_>> for Widget {
    fn from(preview: PopoverPreview<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let tokens = &view.env().theme.tokens;
        let close = preview
            .ctx
            .bind(ResetTimeline, reduce_with!(reset_timeline));

        PreviewShell {
            child: Column {
                gap: Some(tokens.spacing.s),
                children: widgets![
                    Text::new("Real Popover widget anchored to the trigger below.")
                        .size(tokens.typography.font_size_sm)
                        .color(MUTED),
                    Popover {
                        id: WidgetId::explicit("gallery.real.popover"),
                        is_open: preview_active(preview.state),
                        on_toggle: None,
                        on_close: Some(close.clone()),
                        trigger: Button {
                            variant: ButtonVariant::Outline,
                            child: Some(Text::new("Profile actions").into()),
                            ..Default::default()
                        }
                        .into(),
                        content: PopoverMenu {
                            close: close.clone(),
                        }
                        .into(),
                        motion: preview_active(preview.state)
                            .then(|| popover_motion(preview.state))
                            .flatten(),
                    },
                    if preview_active(preview.state) {
                        Widget::from(Button {
                            variant: ButtonVariant::Outline,
                            child: Some(Text::new("Close popover preview").into()),
                            on_press: Some(close),
                            ..Default::default()
                        })
                    } else {
                        Widget::from(Text::new(
                            "Use the playback control to open it; backdrop and close button dismiss it.",
                        )
                        .size(tokens.typography.font_size_sm)
                        .color(MUTED))
                    },
                ],
                ..Default::default()
            }
            .into(),
        }
        .into()
    }
}

fn popover_motion(state: &AnimationGalleryState) -> Option<PopoverMotion> {
    if !policy_allows_motion(state) {
        return None;
    }
    if state.policy == MotionPolicy::Reduced {
        return Some(PopoverMotion::Fade);
    }
    match state.motion {
        MotionChoice::None => None,
        MotionChoice::Default => Some(PopoverMotion::Default),
        MotionChoice::Fade => Some(PopoverMotion::Fade),
        MotionChoice::Scale => Some(PopoverMotion::Scale),
        MotionChoice::Composition => compose_popover_motion(current_composition_atoms(state)),
        MotionChoice::Directional => Some(PopoverMotion::OriginAwareScale),
    }
}

fn compose_popover_motion(atoms: &[MotionAtom]) -> Option<PopoverMotion> {
    let mut motions = atoms.iter().copied().filter_map(|atom| match atom {
        MotionAtom::Fade => Some(PopoverMotion::Fade),
        MotionAtom::Scale => Some(PopoverMotion::Scale),
        MotionAtom::OriginScale => Some(PopoverMotion::OriginAwareScale),
        _ => None,
    });
    let first = motions.next()?;
    Some(motions.fold(first, |acc, motion| acc + motion))
}
