use crate::pages;
use crate::routes::{self, NavGroup, NavItem};
use crate::state::{navigate_to, AnimationGalleryState, NavigateTo};
use crate::style::*;
use crate::widgets::common::PolicyControl;
use fission::build::BuildCtxHandle;
use fission::widgets::Wrap;
use fission::{
    Button, ButtonVariant, Column, Container, FlexDirection, Row, Scroll, Text, Widget, WidgetId,
};

pub struct AppShell<'a> {
    pub ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    pub state: &'a AnimationGalleryState,
    pub viewport_width: f32,
}

impl From<AppShell<'_>> for Widget {
    fn from(shell: AppShell<'_>) -> Self {
        if shell.viewport_width < 980.0 {
            return CompactShell {
                ctx: shell.ctx,
                state: shell.state,
                content_width: (shell.viewport_width - 48.0).max(340.0),
            }
            .into();
        }

        let content_width = (shell.viewport_width - 340.0).max(720.0);
        Row {
            gap: Some(0.0),
            align_items: fission::op::AlignItems::Start,
            children: vec![
                BrandRail {
                    ctx: shell.ctx,
                    state: shell.state,
                }
                .into(),
                Container::new(Scroll {
                    id: Some(WidgetId::explicit(&format!(
                        "animation-gallery.content-scroll.{}",
                        shell.state.current_path
                    ))),
                    direction: FlexDirection::Column,
                    show_scrollbar: true,
                    flex_grow: 1.0,
                    child: Some(
                        Container::new(Column {
                            gap: Some(16.0),
                            children: vec![
                                HeroBar { state: shell.state }.into(),
                                Container::new(pages::router(shell.state.current_path.clone()))
                                    .width(content_width)
                                    .into(),
                                BottomStrip.into(),
                            ],
                            ..Default::default()
                        })
                        .width(content_width)
                        .padding_all(24.0)
                        .into(),
                    ),
                    ..Default::default()
                })
                .flex_grow(1.0)
                .into(),
            ],
            ..Default::default()
        }
        .into()
    }
}

struct CompactShell<'a> {
    ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    state: &'a AnimationGalleryState,
    content_width: f32,
}

impl From<CompactShell<'_>> for Widget {
    fn from(shell: CompactShell<'_>) -> Self {
        let mut children: Vec<Widget> = vec![CompactBrand.into()];
        if shell.state.current_path == routes::OVERVIEW {
            children.push(
                TopStoryRow {
                    ctx: shell.ctx,
                    state: shell.state,
                }
                .into(),
            );
        }
        children.push(
            Dashboard {
                ctx: shell.ctx,
                state: shell.state,
                width: shell.content_width,
            }
            .into(),
        );
        children.push(BottomStrip.into());

        Container::new(Scroll {
            id: Some(WidgetId::explicit(&format!(
                "animation-gallery.compact-scroll.{}",
                shell.state.current_path
            ))),
            direction: FlexDirection::Column,
            show_scrollbar: true,
            flex_grow: 1.0,
            child: Some(
                Container::new(Column {
                    gap: Some(16.0),
                    children,
                    ..Default::default()
                })
                .width(shell.content_width)
                .padding_all(18.0)
                .into(),
            ),
            ..Default::default()
        })
        .flex_grow(1.0)
        .into()
    }
}

struct BrandRail<'a> {
    ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    state: &'a AnimationGalleryState,
}

impl From<BrandRail<'_>> for Widget {
    fn from(rail: BrandRail<'_>) -> Self {
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
                    gap: Some(24.0),
                    children: vec![
                        Row {
                            gap: Some(12.0),
                            children: vec![
                                Container::new(
                                    Text::new("F").size(28.0).color(fission::op::Color::WHITE),
                                )
                                .width(64.0)
                                .height(64.0)
                                .padding_all(16.0)
                                .border_radius(14.0)
                                .bg(color(6, 17, 57, 255))
                                .into(),
                                Column {
                                    gap: Some(3.0),
                                    children: vec![
                                        Text::new("Fission").size(30.0).color(INK).into(),
                                        Text::new("Animation Gallery")
                                            .size(21.0)
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
                            .size(17.0)
                            .color(INK)
                            .into(),
                        Container::new(Column {
                            gap: Some(8.0),
                            children: vec![
                                Text::new("Motion principles").size(14.0).color(INK).into(),
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
                        .padding_all(14.0)
                        .border(BORDER, 1.0)
                        .border_radius(14.0)
                        .bg(SURFACE)
                        .into(),
                        Container::new(Column {
                            gap: Some(12.0),
                            children: nav_children,
                            ..Default::default()
                        })
                        .padding_all(14.0)
                        .border(BORDER, 1.0)
                        .border_radius(14.0)
                        .bg(SURFACE)
                        .into(),
                    ],
                    ..Default::default()
                })
                .padding_all(24.0)
                .into(),
            ),
            ..Default::default()
        })
        .width(300.0)
        .bg(color(236, 241, 248, 255))
        .into()
    }
}

struct HeroBar<'a> {
    state: &'a AnimationGalleryState,
}

impl From<HeroBar<'_>> for Widget {
    fn from(bar: HeroBar<'_>) -> Self {
        let title = if bar.state.current_path == routes::OVERVIEW {
            "Motion Workbench"
        } else {
            "Fission Motion Workbench"
        };
        Container::new(Row {
            gap: Some(18.0),
            children: vec![
                Column {
                    gap: Some(5.0),
                    children: vec![
                        Text::new(title).size(28.0).color(INK).into(),
                        Text::new("Preview real widgets, compose motion atoms, inspect lowered tracks, and validate deterministic behavior.")
                            .size(13.0)
                            .color(MUTED)
                            .into(),
                    ],
                    ..Default::default()
                }
                .into(),
                Container::new(
                    Text::new("explicit / deterministic / testable")
                        .size(12.0)
                        .color(BLUE),
                )
                    .width(238.0)
                    .padding_all(12.0)
                    .border(BORDER, 1.0)
                    .border_radius(999.0)
                    .bg(SURFACE)
                    .into(),
            ],
            ..Default::default()
        })
        .padding_all(18.0)
        .border_radius(20.0)
        .border(BORDER, 1.0)
        .bg(color(247, 250, 255, 255))
        .into()
    }
}

struct CompactBrand;

impl From<CompactBrand> for Widget {
    fn from(_: CompactBrand) -> Self {
        Container::new(Column {
            gap: Some(10.0),
            children: vec![
                Row {
                    gap: Some(12.0),
                    children: vec![
                        Container::new(Text::new("F").size(24.0).color(fission::op::Color::WHITE))
                            .width(54.0)
                            .height(54.0)
                            .padding_all(13.0)
                            .border_radius(13.0)
                            .bg(color(6, 17, 57, 255))
                            .into(),
                        Column {
                            gap: Some(2.0),
                            children: vec![
                                Text::new("Fission").size(24.0).color(INK).into(),
                                Text::new("Animation Gallery").size(18.0).color(BLUE).into(),
                            ],
                            ..Default::default()
                        }
                        .into(),
                    ],
                    ..Default::default()
                }
                .into(),
                Text::new("Calm, explicit, deterministic motion for Fission apps.")
                    .size(13.0)
                    .color(MUTED)
                    .into(),
            ],
            ..Default::default()
        })
        .padding_all(14.0)
        .border(BORDER, 1.0)
        .border_radius(18.0)
        .bg(SURFACE)
        .into()
    }
}

struct PrincipleCard<'a> {
    title: &'a str,
    body: &'a str,
    mark: &'a str,
}

impl From<PrincipleCard<'_>> for Widget {
    fn from(card: PrincipleCard<'_>) -> Self {
        Row {
            gap: Some(12.0),
            children: vec![
                Container::new(Text::new(card.mark).size(16.0).color(BLUE))
                    .width(54.0)
                    .height(54.0)
                    .padding_all(14.0)
                    .border_radius(12.0)
                    .border(color(199, 209, 255, 255), 1.0)
                    .bg(SURFACE)
                    .into(),
                Column {
                    gap: Some(4.0),
                    children: vec![
                        Text::new(card.title).size(13.0).color(INK).into(),
                        Text::new(card.body).size(12.0).color(MUTED).into(),
                    ],
                    ..Default::default()
                }
                .into(),
            ],
            ..Default::default()
        }
        .into()
    }
}

struct TopStoryRow<'a> {
    ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    state: &'a AnimationGalleryState,
}

impl From<TopStoryRow<'_>> for Widget {
    fn from(row: TopStoryRow<'_>) -> Self {
        Wrap {
            direction: FlexDirection::Row,
            spacing: Some(18.0),
            children: vec![
                StoryLabel {
                    step: "1",
                    title: "Widgets",
                    caption: "Browse available widget motions",
                }
                .into(),
                StoryLabel {
                    step: "2",
                    title: "Widget Detail",
                    caption: "Live preview, policies, APIs, and inspection",
                }
                .into(),
                StoryLabel {
                    step: "3",
                    title: "Properties",
                    caption: "Explore a single property in depth",
                }
                .into(),
                StoryLabel {
                    step: "4",
                    title: "Composition Workbench",
                    caption: "Compose effects with clarity and determinism",
                }
                .into(),
                PolicyControl {
                    ctx: row.ctx,
                    state: row.state,
                }
                .into(),
            ],
        }
        .into()
    }
}

struct StoryLabel<'a> {
    step: &'a str,
    title: &'a str,
    caption: &'a str,
}

impl From<StoryLabel<'_>> for Widget {
    fn from(label: StoryLabel<'_>) -> Self {
        Row {
            gap: Some(8.0),
            children: vec![
                Container::new(
                    Text::new(label.step)
                        .size(12.0)
                        .color(fission::op::Color::WHITE),
                )
                .width(26.0)
                .height(26.0)
                .padding_all(6.0)
                .border_radius(13.0)
                .bg(BLUE)
                .into(),
                Column {
                    gap: Some(2.0),
                    children: vec![
                        Text::new(label.title).size(13.0).color(BLUE).into(),
                        Text::new(label.caption).size(11.0).color(MUTED).into(),
                    ],
                    ..Default::default()
                }
                .into(),
            ],
            ..Default::default()
        }
        .into()
    }
}

struct Dashboard<'a> {
    ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    state: &'a AnimationGalleryState,
    width: f32,
}

impl From<Dashboard<'_>> for Widget {
    fn from(dashboard: Dashboard<'_>) -> Self {
        if dashboard.width < 880.0 {
            return Column {
                gap: Some(16.0),
                children: vec![
                    CompactNavPanel {
                        ctx: dashboard.ctx,
                        state: dashboard.state,
                        width: dashboard.width,
                    }
                    .into(),
                    Container::new(pages::router(dashboard.state.current_path.clone()))
                        .width(dashboard.width)
                        .into(),
                ],
                ..Default::default()
            }
            .into();
        }

        Row {
            gap: Some(16.0),
            align_items: fission::op::AlignItems::Start,
            children: vec![
                NavPanel {
                    ctx: dashboard.ctx,
                    state: dashboard.state,
                    width: 236.0,
                }
                .into(),
                Container::new(pages::router(dashboard.state.current_path.clone()))
                    .width((dashboard.width - 252.0).max(480.0))
                    .into(),
            ],
            ..Default::default()
        }
        .into()
    }
}

struct CompactNavPanel<'a> {
    ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    state: &'a AnimationGalleryState,
    width: f32,
}

impl From<CompactNavPanel<'_>> for Widget {
    fn from(panel: CompactNavPanel<'_>) -> Self {
        let mut chips: Vec<Widget> = vec![NavChip {
            ctx: panel.ctx,
            state: panel.state,
            label: "Overview",
            path: routes::OVERVIEW,
        }
        .into()];
        for item in routes::WIDGET_ROUTES {
            chips.push(
                NavChip {
                    ctx: panel.ctx,
                    state: panel.state,
                    label: item.label,
                    path: item.path,
                }
                .into(),
            );
        }
        for (label, path) in [
            ("Opacity", "/properties/opacity"),
            ("Composition", "/composition/additive"),
            ("Policy", "/policy/full"),
            ("Diagnostics", "/diagnostics/declarations"),
        ] {
            chips.push(
                NavChip {
                    ctx: panel.ctx,
                    state: panel.state,
                    label,
                    path,
                }
                .into(),
            );
        }

        Container::new(Column {
            gap: Some(10.0),
            children: vec![
                Text::new("Routes").size(12.0).color(MUTED).into(),
                Wrap {
                    direction: FlexDirection::Row,
                    spacing: Some(8.0),
                    children: chips,
                }
                .into(),
            ],
            ..Default::default()
        })
        .width(panel.width)
        .padding_all(14.0)
        .border(BORDER, 1.0)
        .border_radius(18.0)
        .bg(SURFACE)
        .into()
    }
}

struct NavChip<'a> {
    ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    state: &'a AnimationGalleryState,
    label: &'a str,
    path: &'a str,
}

impl From<NavChip<'_>> for Widget {
    fn from(chip: NavChip<'_>) -> Self {
        let active = chip.state.current_path == chip.path;
        Button {
            width: Some(116.0),
            variant: if active {
                ButtonVariant::Filled
            } else {
                ButtonVariant::Outline
            },
            child: Some(Text::new(chip.label).size(11.0).into()),
            on_press: Some(chip.ctx.bind(
                NavigateTo(chip.path.to_string()),
                fission::reduce_with!(navigate_to),
            )),
            ..Default::default()
        }
        .into()
    }
}

struct NavPanel<'a> {
    ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    state: &'a AnimationGalleryState,
    width: f32,
}

impl From<NavPanel<'_>> for Widget {
    fn from(panel: NavPanel<'_>) -> Self {
        let mut children = vec![Text::new("Fission").size(16.0).color(INK).into()];
        for group in routes::NAV_GROUPS {
            children.push(
                NavGroupView {
                    ctx: panel.ctx,
                    state: panel.state,
                    group,
                }
                .into(),
            );
        }

        Container::new(Column {
            gap: Some(12.0),
            children,
            ..Default::default()
        })
        .width(panel.width)
        .padding_all(14.0)
        .border(BORDER, 1.0)
        .border_radius(18.0)
        .bg(SURFACE)
        .into()
    }
}

struct NavGroupView<'a> {
    ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    state: &'a AnimationGalleryState,
    group: &'a NavGroup,
}

impl From<NavGroupView<'_>> for Widget {
    fn from(group: NavGroupView<'_>) -> Self {
        let mut children = vec![Text::new(group.group.title).size(12.0).color(MUTED).into()];
        for item in group.group.items {
            children.push(
                NavButton {
                    ctx: group.ctx,
                    state: group.state,
                    item,
                }
                .into(),
            );
        }
        Column {
            gap: Some(5.0),
            children,
            ..Default::default()
        }
        .into()
    }
}

struct NavButton<'a> {
    ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    state: &'a AnimationGalleryState,
    item: &'a NavItem,
}

impl From<NavButton<'_>> for Widget {
    fn from(button: NavButton<'_>) -> Self {
        let active = button.state.current_path == button.item.path;
        Button {
            variant: if active {
                ButtonVariant::Filled
            } else {
                ButtonVariant::Ghost
            },
            child: Some(Text::new(button.item.label).size(12.0).into()),
            on_press: Some(button.ctx.bind(
                NavigateTo(button.item.path.to_string()),
                fission::reduce_with!(navigate_to),
            )),
            ..Default::default()
        }
        .into()
    }
}

struct BottomStrip;

impl From<BottomStrip> for Widget {
    fn from(_: BottomStrip) -> Self {
        Container::new(Row {
            gap: Some(28.0),
            children: vec![
                StripItem {
                    title: "Typed & discoverable",
                    body: "via widget-owned enums",
                }
                .into(),
                StripItem {
                    title: "Composability by design",
                    body: "Additive + last-wins",
                }
                .into(),
                StripItem {
                    title: "One runtime",
                    body: "Everything lowers to MotionExpr",
                }
                .into(),
                StripItem {
                    title: "Deterministic",
                    body: "Pure, replayable, testable",
                }
                .into(),
                StripItem {
                    title: "Accessibility first",
                    body: "Policy respects users",
                }
                .into(),
            ],
            ..Default::default()
        })
        .padding_all(12.0)
        .border(BORDER, 1.0)
        .border_radius(14.0)
        .bg(SURFACE)
        .into()
    }
}

struct StripItem<'a> {
    title: &'a str,
    body: &'a str,
}

impl From<StripItem<'_>> for Widget {
    fn from(item: StripItem<'_>) -> Self {
        Column {
            gap: Some(3.0),
            children: vec![
                Text::new(item.title).size(12.0).color(INK).into(),
                Text::new(item.body).size(10.0).color(MUTED).into(),
            ],
            ..Default::default()
        }
        .into()
    }
}
