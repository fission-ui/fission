use crate::state::AnimationGalleryState;
use crate::style;
use fission::prelude::*;

const COLLAPSED_WIDTH: f32 = 120.0;
const EXPANSION_WIDTH: f32 = 40.0;
const RAIL_HEIGHT: f32 = 118.0;

fission_component! {
    pub(super) SidebarRail<AnimationGalleryState> { width_progress: f32 }
    |rail, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Container(
            Column(
            gap: tokens.spacing.s,
            children: [
                Text("Inbox")
                    .size(tokens.typography.font_size_sm)
                    .color(style::text_primary()),
                Text("Archive")
                    .size(tokens.typography.font_size_sm)
                    .color(style::text_muted()),
                Text("Settings")
                    .size(tokens.typography.font_size_sm)
                    .color(style::text_muted()),
            ],
        ))
        .width(COLLAPSED_WIDTH + EXPANSION_WIDTH * rail.width_progress)
        .height(RAIL_HEIGHT)
        .padding_all(tokens.spacing.s)
        .border_radius(tokens.radii.xl)
        .border(style::border(), 1.0)
        .bg(style::surface())
}
