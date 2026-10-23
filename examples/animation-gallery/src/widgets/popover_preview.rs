use super::common::{policy_allows_motion, preview_active, PreviewShell};
use super::popover_menu::PopoverMenu;
use crate::state::{
    current_composition_atoms, reset_timeline, toggle_play, AnimationGalleryState, MotionAtom,
    MotionChoice, MotionPolicy, ResetTimeline, TogglePlay,
};
use crate::style;
use fission::prelude::*;
use fission::widgets::{Popover, PopoverMotion};

fission_component! {
    pub(super) PopoverPreview<AnimationGalleryState> {}
    |_preview, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let close = cx.bind(ResetTimeline, reduce_with!(reset_timeline));
        let toggle = cx.bind(TogglePlay, reduce_with!(toggle_play));
    } =>
        PreviewShell(
            Column(
                gap: tokens.spacing.s,
                children: [
                    Text("Real Popover widget anchored to the trigger below.")
                        .size(tokens.typography.font_size_sm)
                        .color(style::text_muted()),
                    Popover(
                        id: WidgetId::explicit("gallery.real.popover"),
                        is_open: preview_active(state),
                        on_close: close.clone(),
                        trigger: Button(
                            variant: ButtonVariant::Outline,
                            child: Text("Profile actions"),
                            on_press: toggle,
                        ).semantics_identifier("gallery.real.popover.trigger"),
                        content: PopoverMenu(close.clone()),
                        motion: preview_active(state)
                            .then(|| popover_motion(state))
                            .flatten(),
                    ),
                    if preview_active(state) {
                        Button(
                            variant: ButtonVariant::Outline,
                            child: Text("Close popover preview"),
                            on_press: close,
                        )
                    } else {
                        Text(
                            "Use the playback control to open it; backdrop and close button dismiss it.",
                        )
                        .size(tokens.typography.font_size_sm)
                        .color(style::text_muted())
                    },
                ],
            ),
        )
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
