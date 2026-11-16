use super::*;
use crate::style;

const OVERVIEW_ITEM: NavItem = NavItem {
    path: routes::OVERVIEW,
    label: "Overview",
};

fission_component! {
    pub(super) BrandRail<AnimationGalleryState> |_rail, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let typography = &tokens.typography;
    } =>
        Container(
            Scroll(
                id: WidgetId::explicit("animation-gallery.rail-scroll"),
                direction: FlexDirection::Column,
                show_scrollbar: false,
                child: Container(
                    Column(
                        gap: tokens.spacing.l,
                        children: [
                            if !state.embedded {
                                Row(
                                    gap: tokens.spacing.m,
                                    children: [
                                        Container(
                                            Text("F")
                                                .size(typography.heading_size)
                                                .color(style::on_primary()),
                                        )
                                        .width(BRAND_MARK_SIZE)
                                        .height(BRAND_MARK_SIZE)
                                        .padding_all(tokens.spacing.m)
                                        .border_radius(tokens.radii.xl)
                                        .bg(style::primary()),
                                        Column(
                                            gap: tokens.spacing.xs,
                                            children: [
                                                Text("Fission")
                                                    .size(typography.heading_size)
                                                    .color(style::text_primary()),
                                                Text("Animation Gallery")
                                                    .size(typography.font_size_lg)
                                                    .color(style::primary()),
                                            ],
                                        ),
                                    ],
                                )
                            },
                            if !state.embedded {
                                Text("Explore, compose, and ship deterministic motion for your UI.")
                                    .size(typography.body_large_size)
                                    .color(style::text_primary())
                            },
                            Container(
                                Column(
                                    gap: tokens.spacing.s,
                                    children: [
                                        Text("Motion principles")
                                            .size(typography.font_size_base)
                                            .color(style::text_primary()),
                                        PrincipleCard("Explicit", "No hidden animation.", "*"),
                                        PrincipleCard("Inspectable", "Everything lowers to MotionExpr.", "</>"),
                                    ],
                                ),
                            )
                            .padding_all(tokens.spacing.m)
                            .border(style::border(), 1.0)
                            .border_radius(tokens.radii.xl)
                            .bg(style::surface()),
                            Container(
                                Column(
                                    gap: tokens.spacing.m,
                                    children: [
                                        NavButton(&OVERVIEW_ITEM),
                                        for group in routes::NAV_GROUPS.iter() {
                                            NavGroupView(group, key: group.title)
                                        },
                                    ],
                                ),
                            )
                            .padding_all(tokens.spacing.m)
                            .border(style::border(), 1.0)
                            .border_radius(tokens.radii.xl)
                            .bg(style::surface()),
                        ],
                    ),
                ).padding_all(tokens.spacing.l),
            ),
        )
        .width(BRAND_RAIL_WIDTH)
        .bg(style::surface_sunken())
}
