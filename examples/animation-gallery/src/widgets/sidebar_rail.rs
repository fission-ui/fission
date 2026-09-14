use crate::state::AnimationGalleryState;
use crate::style;
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
                    .color(style::text_primary()),
                Text::new("Archive")
                    .size(tokens.typography.font_size_sm)
                    .color(style::text_muted()),
                Text::new("Settings")
                    .size(tokens.typography.font_size_sm)
                    .color(style::text_muted()),
            ],
            ..Default::default()
        })
        .width(COLLAPSED_WIDTH + EXPANSION_WIDTH * rail.width_progress)
        .height(RAIL_HEIGHT)
        .padding_all(tokens.spacing.s)
        .border_radius(tokens.radii.xl)
        .border(style::border(), 1.0)
        .bg(style::surface())
        .into()
    }
}
