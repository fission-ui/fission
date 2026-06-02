use super::home_widgets::{
    nav_inset, semantic_row, ExternalNavLink, NavLink, SearchPill, ThemeToggle,
};
use super::state::DocsState;
use fission::op::{AlignItems, Fill, FlexWrap, JustifyContent};
use fission::prelude::*;

const NAV_ITEMS: &[(&str, &str)] = &[
    ("Product", "/product/overview/"),
    ("Setup", "/docs/learn/quickstart/"),
    ("Learn", "/docs/learn/overview/"),
    ("Build", "/docs/build-and-package/overview/"),
    ("Test", "/docs/test-and-debug/overview/"),
    ("Publish", "/docs/release-and-distribute/overview/"),
];

#[derive(Clone, Debug)]
pub(super) struct HomePageNav;

impl From<HomePageNav> for Widget {
    fn from(_component: HomePageNav) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        let nav_items = NAV_ITEMS
            .iter()
            .map(|(label, href)| NavLink::new(label, href).into())
            .collect::<Vec<_>>();
        Container::new(Row {
            children: vec![
                semantic_row(
                    "site-route:/",
                    vec![
                        Image::asset("/img/fission-mark.svg")
                            .size(tokens.spacing.l, tokens.spacing.l)
                            .into(),
                        Text::new("Fission")
                            .size(tokens.typography.font_size_lg)
                            .weight(tokens.typography.font_weight_bold)
                            .color(tokens.colors.heading)
                            .into(),
                    ],
                    Some(tokens.spacing.s),
                    FlexWrap::NoWrap,
                    AlignItems::Center,
                    JustifyContent::Start,
                ),
                Row {
                    children: nav_items,
                    gap: Some(tokens.spacing.l),
                    justify_content: JustifyContent::End,
                    ..Default::default()
                }
                .into(),
                Row {
                    children: vec![
                        ExternalNavLink::new("GitHub", "https://github.com/worka-ai/fission")
                            .into(),
                        ThemeToggle.into(),
                        SearchPill.into(),
                    ],
                    gap: Some(tokens.spacing.m),
                    justify_content: JustifyContent::End,
                    align_items: AlignItems::Center,
                    ..Default::default()
                }
                .into(),
            ],
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            wrap: FlexWrap::Wrap,
            ..Default::default()
        })
        .padding([
            nav_inset(tokens),
            nav_inset(tokens),
            tokens.spacing.m,
            tokens.spacing.m,
        ])
        .bg_fill(Fill::Solid(tokens.colors.surface.with_alpha(232)))
        .border(tokens.colors.border, 1.0)
        .into()
    }
}
