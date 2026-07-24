use super::*;

pub(super) struct BrandRail<'a> {
    pub(super) ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    pub(super) state: &'a AnimationGalleryState,
}

impl From<BrandRail<'_>> for Widget {
    fn from(rail: BrandRail<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let tokens = &view.env().theme.tokens;
        let typography = &tokens.typography;
        let mut nav_children = vec![NavButton {
            ctx: rail.ctx,
            state: rail.state,
            item: &NavItem {
                path: routes::OVERVIEW,
                label: "Overview",
            },
        }
        .into()];
        for group in routes::NAV_GROUPS {
            nav_children.push(
                NavGroupView {
                    ctx: rail.ctx,
                    state: rail.state,
                    group,
                }
                .into(),
            );
        }

        Container::new(Scroll {
            id: Some(WidgetId::explicit("animation-gallery.rail-scroll")),
            direction: FlexDirection::Column,
            show_scrollbar: false,
            child: Some(
                Container::new(Column {
                    gap: Some(tokens.spacing.l),
                    children: vec![
                        Row {
                            gap: Some(tokens.spacing.m),
                            children: vec![
                                Container::new(
                                    Text::new("F")
                                        .size(typography.heading_size)
                                        .color(fission::op::Color::WHITE),
                                )
                                .width(BRAND_MARK_SIZE)
                                .height(BRAND_MARK_SIZE)
                                .padding_all(tokens.spacing.m)
                                .border_radius(tokens.radii.xl)
                                .bg(color(6, 17, 57, 255))
                                .into(),
                                Column {
                                    gap: Some(tokens.spacing.xs),
                                    children: vec![
                                        Text::new("Fission")
                                            .size(typography.heading_size)
                                            .color(INK)
                                            .into(),
                                        Text::new("Animation Gallery")
                                            .size(typography.font_size_lg)
                                            .color(BLUE)
                                            .into(),
                                    ],
                                    ..Default::default()
                                }
                                .into(),
                            ],
                            ..Default::default()
                        }
                        .into(),
                        Text::new("Explore, compose, and ship deterministic motion for your UI.")
                            .size(typography.body_large_size)
                            .color(INK)
                            .into(),
                        Container::new(Column {
                            gap: Some(tokens.spacing.s),
                            children: vec![
                                Text::new("Motion principles")
                                    .size(typography.font_size_base)
                                    .color(INK)
                                    .into(),
                                PrincipleCard {
                                    title: "Explicit",
                                    body: "No hidden animation.",
                                    mark: "*",
                                }
                                .into(),
                                PrincipleCard {
                                    title: "Inspectable",
                                    body: "Everything lowers to MotionExpr.",
                                    mark: "</>",
                                }
                                .into(),
                            ],
                            ..Default::default()
                        })
                        .padding_all(tokens.spacing.m)
                        .border(BORDER, 1.0)
                        .border_radius(tokens.radii.xl)
                        .bg(SURFACE)
                        .into(),
                        Container::new(Column {
                            gap: Some(tokens.spacing.m),
                            children: nav_children,
                            ..Default::default()
                        })
                        .padding_all(tokens.spacing.m)
                        .border(BORDER, 1.0)
                        .border_radius(tokens.radii.xl)
                        .bg(SURFACE)
                        .into(),
                    ],
                    ..Default::default()
                })
                .padding_all(tokens.spacing.l)
                .into(),
            ),
            ..Default::default()
        })
        .width(BRAND_RAIL_WIDTH)
        .bg(color(236, 241, 248, 255))
        .into()
    }
}
