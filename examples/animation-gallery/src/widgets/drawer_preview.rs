use super::common::{policy_allows_motion, preview_active, PreviewShell};
use crate::state::{
    current_composition_atoms, reset_timeline, toggle_play, AnimationGalleryState, MotionAtom,
    MotionChoice, MotionPolicy, ResetTimeline, TogglePlay,
};
use crate::style::MUTED;
use crate::ui;
use fission::build::BuildCtxHandle;
use fission::prelude::*;
use fission::widgets::{Drawer, DrawerMotion, DrawerSide};

const DRAWER_WIDTH: f32 = 320.0;

pub(super) struct DrawerPreview<'a> {
    pub ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    pub state: &'a AnimationGalleryState,
}

impl From<DrawerPreview<'_>> for Widget {
    fn from(preview: DrawerPreview<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let tokens = &view.env().theme.tokens;
        let close = preview
            .ctx
            .bind(ResetTimeline, reduce_with!(reset_timeline));

        PreviewShell {
            child: Column {
                gap: Some(tokens.spacing.s),
                children: widgets![
                    Text::new(
                        "Real Drawer widget. Play opens the side panel through the portal layer.",
                    )
                    .size(tokens.typography.font_size_sm)
                    .color(MUTED),
                    ui::SmallButton {
                        ctx: preview.ctx,
                        label: "Open real drawer",
                        action: TogglePlay,
                        reducer: toggle_play,
                    },
                    Drawer {
                        id: WidgetId::explicit("gallery.real.drawer"),
                        side: DrawerSide::Right,
                        is_open: preview_active(preview.state),
                        on_dismiss: Some(close.clone()),
                        content: Column {
                            gap: Some(tokens.spacing.s),
                            children: widgets![
                                Text::new("Settings").size(tokens.typography.font_size_lg),
                                Text::new("This is the actual Drawer content.")
                                    .size(tokens.typography.font_size_sm),
                                Button {
                                    variant: ButtonVariant::Outline,
                                    child: Some(Text::new("Close drawer").into()),
                                    on_press: Some(close),
                                    ..Default::default()
                                },
                            ],
                            ..Default::default()
                        }
                        .into(),
                        width: Some(DRAWER_WIDTH),
                        motion: preview_active(preview.state)
                            .then(|| drawer_motion(preview.state))
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

fn drawer_motion(state: &AnimationGalleryState) -> Option<DrawerMotion> {
    if !policy_allows_motion(state) {
        return None;
    }
    if state.policy == MotionPolicy::Reduced {
        return Some(DrawerMotion::Fade);
    }
    match state.motion {
        MotionChoice::None => None,
        MotionChoice::Default => Some(DrawerMotion::Default),
        MotionChoice::Fade => Some(DrawerMotion::Fade),
        MotionChoice::Directional => Some(DrawerMotion::FromSide),
        MotionChoice::Composition => compose_drawer_motion(current_composition_atoms(state)),
        MotionChoice::Scale => Some(DrawerMotion::Default),
    }
}

fn compose_drawer_motion(atoms: &[MotionAtom]) -> Option<DrawerMotion> {
    let mut motions = atoms.iter().copied().filter_map(|atom| match atom {
        MotionAtom::FromSide => Some(DrawerMotion::FromSide),
        MotionAtom::FromLeft => Some(DrawerMotion::FromLeft),
        MotionAtom::FromRight => Some(DrawerMotion::FromRight),
        MotionAtom::FromTop => Some(DrawerMotion::FromTop),
        MotionAtom::FromBottom => Some(DrawerMotion::FromBottom),
        MotionAtom::Fade => Some(DrawerMotion::Fade),
        _ => None,
    });
    let first = motions.next()?;
    Some(motions.fold(first, |acc, motion| acc + motion))
}
