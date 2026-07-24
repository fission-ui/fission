use crate::state::AnimationGalleryState;
use crate::style::{BORDER, INK, MUTED, SURFACE};
use fission::prelude::*;

const COLLAPSED_WIDTH: f32 = 120.0;
const EXPANSION_WIDTH: f32 = 40.0;
const RAIL_HEIGHT: f32 = 118.0;

pub(super) struct SidebarRail {
    pub width_progress: f32,
}

impl From<SidebarRail> for Widget {
    fn from(rail: SidebarRail) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let tokens = &view.env().theme.tokens;

        Container::new(Column {
            gap: Some(tokens.spacing.s),
            children: widgets![
                Text::new("Inbox")
                    .size(tokens.typography.font_size_sm)
                    .color(INK),
                Text::new("Archive")
                    .size(tokens.typography.font_size_sm)
                    .color(MUTED),
                Text::new("Settings")
                    .size(tokens.typography.font_size_sm)
                    .color(MUTED),
            ],
            ..Default::default()
        })
        .width(COLLAPSED_WIDTH + EXPANSION_WIDTH * rail.width_progress)
        .height(RAIL_HEIGHT)
        .padding_all(tokens.spacing.s)
        .border_radius(tokens.radii.xl)
        .border(BORDER, 1.0)
        .bg(SURFACE)
        .into()
    }
}
