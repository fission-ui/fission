use super::brand_logo::BrandLogo;
use super::home_widgets::site_semantics;
use super::state::DocsState;
use fission::op::{AlignItems, Fill, FlexWrap, JustifyContent, TextAlign};
use fission::prelude::*;

#[derive(Clone, Debug)]
pub(crate) struct DocsFooter;

impl From<DocsFooter> for Widget {
    fn from(_component: DocsFooter) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        Container::new(Column {
            children: vec![
                Row {
                    children: vec![
                        FooterColumn::new(
                            "Platform",
                            &[
                                ("Overview", "/product/overview/"),
                                ("Cross-platform apps", "/product/cross-platform-apps/"),
                                ("Static sites", "/product/static-sites/"),
                                ("Server-rendered sites", "/product/server-rendered-sites/"),
                                ("Terminal apps", "/product/terminal-apps/"),
                            ],
                        )
                        .into(),
                        FooterColumn::new(
                            "Build",
                            &[
                                ("Quickstart", "/docs/learn/quickstart/"),
                                ("Documentation", "/docs/learn/overview/"),
                                ("Guides", "/docs/guides/layout-and-widgets/"),
                                ("Cookbook", "/docs/cookbook/add-platform-targets/"),
                                ("Examples", "/example-showcase/"),
                            ],
                        )
                        .into(),
                        FooterColumn::new(
                            "Explore",
                            &[
                                ("Crate atlas", "/crates/"),
                                ("API reference", "/reference/overview/overview/"),
                                ("Charts", "/product/charts/"),
                                ("Design systems", "/product/design-systems/"),
                                ("Blog", "/blog/"),
                            ],
                        )
                        .into(),
                        FooterColumn::new(
                            "Ship",
                            &[
                                ("Production lifecycle", "/product/production-lifecycle/"),
                                ("Build and package", "/docs/build-and-package/overview/"),
                                ("Test and debug", "/docs/test-and-debug/overview/"),
                                ("Release", "/docs/release-and-distribute/overview/"),
                                ("Developer tools", "/product/developer-tools/"),
                            ],
                        )
                        .into(),
                    ],
                    gap: Some(tokens.spacing.xxl),
                    wrap: FlexWrap::Wrap,
                    align_items: AlignItems::Start,
                    justify_content: JustifyContent::Center,
                    ..Default::default()
                }
                .into(),
                FooterIdentity.into(),
            ],
            gap: Some(tokens.spacing.xxl),
            align_items: AlignItems::Center,
            semantics: Some(site_semantics("site-footer")),
            ..Default::default()
        })
        .padding_all(tokens.spacing.xxxxl)
        .bg_fill(Fill::Solid(tokens.colors.background))
        .border(tokens.colors.border, 1.0)
        .into()
    }
}

#[derive(Clone, Debug)]
struct FooterIdentity;

impl From<FooterIdentity> for Widget {
    fn from(_identity: FooterIdentity) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        Container::new(Column {
            children: vec![
                BrandLogo::new(tokens.spacing.l).centered().into(),
                Text::new("One Rust application model for native, mobile, web, terminal, static, and server-rendered products. Apache 2.0 licensed.")
                    .size(tokens.typography.body_medium_size)
                    .line_height(tokens.typography.body_medium_size * tokens.typography.line_height_normal)
                    .color(tokens.colors.text_secondary)
                    .max_width(tokens.spacing.xxxxl * 7.0)
                    .text_align(TextAlign::Center)
                    .flex_shrink(1.0)
                    .into(),
                Text::new("Copyright (c) 2026 Fission")
                    .size(tokens.typography.font_size_sm)
                    .color(tokens.colors.text_muted)
                    .text_align(TextAlign::Center)
                    .into(),
                Text::new("Ready to use today. Widget APIs are expected to remain stable; some runtime and shell APIs may change before 1.0.0.")
                    .size(tokens.typography.font_size_sm)
                    .line_height(tokens.typography.font_size_sm * tokens.typography.line_height_normal)
                    .color(tokens.colors.text_muted)
                    .max_width(tokens.spacing.xxxxl * 8.0)
                    .text_align(TextAlign::Center)
                    .flex_shrink(1.0)
                    .into(),
                Row {
                    children: vec![
                        FooterLink::new("GitHub", "https://github.com/fission-ui/fission")
                            .into(),
                        FooterLink::new("Quickstart", "/docs/learn/quickstart/").into(),
                        FooterLink::new("Reference", "/reference/overview/overview/")
                            .into(),
                    ],
                    gap: Some(tokens.spacing.m),
                    wrap: FlexWrap::Wrap,
                    justify_content: JustifyContent::Center,
                    ..Default::default()
                }
                .into(),
                Text::new("Fission 0.12.0")
                    .size(tokens.typography.font_size_sm)
                    .family(tokens.typography.font_family_mono.clone())
                    .color(tokens.colors.text_muted)
                    .text_align(TextAlign::Center)
                    .into(),
            ],
            gap: Some(tokens.spacing.m),
            align_items: AlignItems::Center,
            ..Default::default()
        })
        .padding([0.0, 0.0, tokens.spacing.l, 0.0])
        .into()
    }
}

#[derive(Clone, Debug)]
struct FooterColumn {
    title: &'static str,
    links: &'static [(&'static str, &'static str)],
}

impl FooterColumn {
    fn new(title: &'static str, links: &'static [(&'static str, &'static str)]) -> Self {
        Self { title, links }
    }
}

impl From<FooterColumn> for Widget {
    fn from(component: FooterColumn) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        Container::new(Column {
            children: std::iter::once(
                Text::new(component.title)
                    .size(tokens.typography.font_size_sm)
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.heading)
                    .into(),
            )
            .chain(
                component
                    .links
                    .iter()
                    .map(|(label, href)| FooterLink::new(label, href).into()),
            )
            .collect(),
            gap: Some(tokens.spacing.s),
            ..Default::default()
        })
        .width(190.0)
        .flex_shrink(1.0)
        .into()
    }
}
#[derive(Clone, Debug)]
struct FooterLink {
    label: &'static str,
    href: &'static str,
}

impl FooterLink {
    fn new(label: &'static str, href: &'static str) -> Self {
        Self { label, href }
    }
}

impl From<FooterLink> for Widget {
    fn from(component: FooterLink) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        let identifier =
            if component.href.starts_with("http://") || component.href.starts_with("https://") {
                format!("markdown-link:{}", component.href)
            } else {
                format!("site-route:{}", component.href)
            };
        Text::new(component.label)
            .size(tokens.typography.font_size_sm)
            .weight(tokens.typography.font_weight_medium)
            .color(tokens.colors.text_secondary)
            .semantics_identifier(identifier)
            .into()
    }
}
