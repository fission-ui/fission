use super::common::PreviewShell;
use super::sidebar_rail::SidebarRail;
use crate::state::{current_composition_atoms, AnimationGalleryState, MotionAtom, MotionChoice};
use crate::style::MUTED;
use fission::prelude::*;

pub(super) struct SidebarPreview<'a> {
    pub state: &'a AnimationGalleryState,
}

impl From<SidebarPreview<'_>> for Widget {
    fn from(preview: SidebarPreview<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let progress = if preview.state.playing {
            1.0
        } else {
            preview.state.scrub_ms as f32 / 300.0
        };
        let width_progress = if preview.state.motion != MotionChoice::Composition
            || current_composition_atoms(preview.state)
                .iter()
                .any(|atom| matches!(atom, MotionAtom::Width))
        {
            progress
        } else {
            0.0
        };

        PreviewShell {
            child: Row {
                gap: Some(view.env().theme.tokens.spacing.s),
                children: widgets![
                    SidebarRail { width_progress },
                    Text::new("Sidebar uses Drawer-style native motion.")
                        .size(view.env().theme.tokens.typography.font_size_sm)
                        .color(MUTED),
                ],
                ..Default::default()
            }
            .into(),
        }
        .into()
    }
}
