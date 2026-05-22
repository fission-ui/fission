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

impl Widget<DocsState> for HomePageNav {
    fn build(&self, _ctx: &mut BuildCtx<DocsState>, view: &View<DocsState>) -> Node {
        let tokens = &view.env.theme.tokens;
        let nav_items = NAV_ITEMS
            .iter()
            .map(|(label, href)| NavLink::new(label, href).build(_ctx, view))
            .collect::<Vec<_>>();
        Container::new(
            Row {
                children: vec![
                    semantic_row(
                        "site-route:/",
                        vec![
                            Image {
                                source: "/img/fission-mark.svg".to_string(),
                                width: Some(tokens.spacing.l),
                                height: Some(tokens.spacing.l),
                                ..Default::default()
                            }
                            .into_node(),
                            Text::new("Fission")
                                .size(tokens.typography.font_size_lg)
                                .weight(tokens.typography.font_weight_bold)
                                .color(tokens.colors.heading)
                                .into_node(),
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
                    .into_node(),
                    Row {
                        children: vec![
                            ExternalNavLink::new("GitHub", "https://github.com/worka-ai/fission")
                                .build(_ctx, view),
                            ThemeToggle.build(_ctx, view),
                            SearchPill.build(_ctx, view),
                        ],
                        gap: Some(tokens.spacing.m),
                        justify_content: JustifyContent::End,
                        align_items: AlignItems::Center,
                        ..Default::default()
                    }
                    .into_node(),
                ],
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                wrap: FlexWrap::Wrap,
                ..Default::default()
            }
            .into_node(),
        )
        .padding([
            nav_inset(tokens),
            nav_inset(tokens),
            tokens.spacing.m,
            tokens.spacing.m,
        ])
        .bg_fill(Fill::Solid(tokens.colors.surface.with_alpha(232)))
        .border(tokens.colors.border, 1.0)
        .into_node()
    }
}
