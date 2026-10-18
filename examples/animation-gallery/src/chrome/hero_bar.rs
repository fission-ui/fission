use super::*;
use crate::style;

fission_component! {
    pub(super) HeroBar<AnimationGalleryState> |_bar, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let typography = &tokens.typography;
        let title = if state.current_path == routes::OVERVIEW {
            "Motion Workbench"
        } else {
            "Fission Motion Workbench"
        };
    } =>
        Container(
            Row(
                gap: tokens.spacing.m,
                children: [
                    Column(
                        gap: tokens.spacing.xs,
                        children: [
                            Text(title)
                                .size(typography.heading_size)
                                .color(style::text_primary()),
                            Text("Preview real widgets, compose motion atoms, inspect lowered tracks, and validate deterministic behaviour.")
                                .size(typography.font_size_sm)
                                .color(style::text_muted()),
                        ],
                    ),
                    Container(
                        Text("explicit / deterministic / testable")
                            .size(typography.font_size_xs)
                            .color(style::primary()),
                    )
                    .width(HERO_BADGE_WIDTH)
                    .padding_all(tokens.spacing.m)
                    .border(style::border(), 1.0)
                    .border_radius(tokens.radii.full)
                    .bg(style::surface()),
                ],
            ),
        )
        .padding_all(tokens.spacing.m)
        .border_radius(tokens.radii.xxl)
        .border(style::border(), 1.0)
        .bg(style::primary_subtle())
}
