use super::common::PreviewShell;
use super::sidebar_rail::SidebarRail;
use crate::state::{current_composition_atoms, AnimationGalleryState, MotionAtom, MotionChoice};
use crate::style;
use fission::prelude::*;

fission_component! {
    pub(super) SidebarPreview<AnimationGalleryState> {}
    |_preview, state, cx| {
        let progress = if state.playing {
            1.0
        } else {
            state.scrub_ms as f32 / 300.0
        };
        let width_progress = if state.motion != MotionChoice::Composition
            || current_composition_atoms(state)
                .iter()
                .any(|atom| matches!(atom, MotionAtom::Width))
        {
            progress
        } else {
            0.0
        };

    } =>
        PreviewShell(
            Row(
                gap: cx.view.env().theme.tokens.spacing.s,
                children: [
                    SidebarRail(width_progress),
                    Text("Sidebar uses Drawer-style native motion.")
                        .size(cx.view.env().theme.tokens.typography.font_size_sm)
                        .color(style::text_muted()),
                ],
            ),
        )
}
