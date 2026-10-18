use super::common::{policy_allows_motion, preview_active, PreviewShell};
use crate::state::{
    current_composition_atoms, reset_timeline, toggle_play, AnimationGalleryState, MotionAtom,
    MotionChoice, MotionPolicy, ResetTimeline, TogglePlay,
};
use crate::style;
use crate::ui::*;
use fission::prelude::*;
use fission::widgets::{Drawer, DrawerMotion, DrawerSide};

const DRAWER_WIDTH: f32 = 320.0;

fission_component! {
    pub(super) DrawerPreview<AnimationGalleryState> {}
    |_preview, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let close = cx.bind(ResetTimeline, reduce_with!(reset_timeline));
    } =>
        PreviewShell(
            Column(
                gap: tokens.spacing.s,
                children: [
                    Text(
                        "Real Drawer widget. Play opens the side panel through the portal layer.",
                    )
                    .size(tokens.typography.font_size_sm)
                    .color(style::text_muted()),
                    SmallButton("Open real drawer".to_string(), cx.bind(TogglePlay, reduce_with!(toggle_play))),
                    Drawer(
                        id: WidgetId::explicit("gallery.real.drawer"),
                        side: DrawerSide::Right,
                        is_open: preview_active(state),
                        on_dismiss: close.clone(),
                        dismiss_semantics_identifier: "gallery.real.drawer.backdrop".to_string(),
                        content: Column(
                            gap: tokens.spacing.s,
                            children: [
                                Text("Settings").size(tokens.typography.font_size_lg),
                                Text("This is the actual Drawer content.")
                                    .size(tokens.typography.font_size_sm),
                                Button(
                                    variant: ButtonVariant::Outline,
                                    child: Text("Close drawer"),
                                    on_press: close.clone(),
                                )
                                .semantics_identifier("gallery.real.drawer.close"),
                            ],
                        ),
                        width: DRAWER_WIDTH,
                        motion: preview_active(state)
                            .then(|| drawer_motion(state))
                            .flatten(),
                    ),
                ],
            ),
        )
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
