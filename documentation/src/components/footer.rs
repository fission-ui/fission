use super::home_widgets::semantic_row;
use super::state::DocsState;
use fission::op::{AlignItems, Fill, FlexWrap, JustifyContent};
use fission::prelude::*;

#[derive(Clone, Debug)]
pub(crate) struct DocsFooter;

impl Widget<DocsState> for DocsFooter {
    fn build(&self, ctx: &mut BuildCtx<DocsState>, view: &View<DocsState>) -> Node {
        let tokens = &view.env.theme.tokens;
        Container::new(
            Column {
                children: vec![
                    Row {
                        children: vec![
                            footer_brand(ctx, view),
                            FooterColumn::new(
                                "Learn",
                                &[
                                    ("Overview", "/docs/learn/overview/"),
                                    ("Quickstart", "/docs/learn/quickstart/"),
                                    ("Runtime model", "/docs/learn/runtime-model/"),
                                ],
                            )
                            .build(ctx, view),
                            FooterColumn::new(
                                "Guides",
                                &[
                                    ("App structure", "/docs/guides/app-structure/"),
                                    ("Resources and async", "/docs/guides/resources-and-async/"),
                                    (
                                        "Testing and diagnostics",
                                        "/docs/guides/testing-and-diagnostics/",
                                    ),
                                    ("Theming and i18n", "/docs/guides/theming-and-i18n/"),
                                    (
                                        "Platform shells",
                                        "/docs/guides/platform-shells-cli-and-testing/",
                                    ),
                                    (
                                        "Terminal interfaces",
                                        "/docs/guides/terminal-user-interfaces/",
                                    ),
                                ],
                            )
                            .build(ctx, view),
                            FooterColumn::new(
                                "Charts",
                                &[
                                    ("Overview", "/reference/charts/overview/"),
                                    ("Catalog", "/docs/charts/catalog/"),
                                    ("Data and interaction", "/docs/charts/data-and-interaction/"),
                                    ("3D and GL", "/docs/charts/three-dimensional-and-gl/"),
                                ],
                            )
                            .build(ctx, view),
                            FooterColumn::new(
                                "Cookbook",
                                &[
                                    ("Build a counter", "/docs/cookbook/build-a-counter/"),
                                    ("Add platform targets", "/docs/cookbook/add-platform-targets/"),
                                    (
                                        "Write a live interface test",
                                        "/docs/cookbook/write-a-live-ui-test/",
                                    ),
                                ],
                            )
                            .build(ctx, view),
                            FooterColumn::new(
                                "Explore",
                                &[
                                    ("Reference", "/reference/overview/overview/"),
                                    ("Examples", "/docs/learn/examples-and-targets/"),
                                    ("GitHub", "https://github.com/worka-ai/fission"),
                                ],
                            )
                            .build(ctx, view),
                        ],
                        gap: Some(tokens.spacing.xxl),
                        wrap: FlexWrap::Wrap,
                        align_items: AlignItems::Start,
                        justify_content: JustifyContent::SpaceBetween,
                        ..Default::default()
                    }
                    .into_node(),
                    Container::new(
                        Row {
                            children: vec![
                                Text::new("Copyright (c) 2026 Fission - MIT License")
                                    .size(tokens.typography.font_size_sm)
                                    .color(tokens.colors.text_muted)
                                    .into_node(),
                                Text::new("The Fission framework is ready to use today, but some areas are actively under development. Widget APIs are expected to remain stable; some runtime or shell APIs may get breaking changes before 1.0.0.")
                                    .size(tokens.typography.font_size_sm)
                                    .color(tokens.colors.text_muted)
                                    .into_node(),
                            ],
                            gap: Some(tokens.spacing.l),
                            wrap: FlexWrap::Wrap,
                            justify_content: JustifyContent::SpaceBetween,
                            ..Default::default()
                        }
                        .into_node(),
                    )
                    .padding([0.0, 0.0, tokens.spacing.l, 0.0])
                    .border(tokens.colors.border, 1.0)
                    .into_node(),
                ],
                gap: Some(tokens.spacing.xxl),
                ..Default::default()
            }
            .into_node(),
        )
        .padding_all(tokens.spacing.xxxxl)
        .bg_fill(Fill::Solid(tokens.colors.background))
        .border(tokens.colors.border, 1.0)
        .into_node()
    }
}

fn footer_brand(ctx: &mut BuildCtx<DocsState>, view: &View<DocsState>) -> Node {
    let tokens = &view.env.theme.tokens;
    Column {
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
            Text::new("A cross-platform, GPU-accelerated user interface framework for Rust. MIT licensed.")
                .size(tokens.typography.body_medium_size)
                .line_height(tokens.typography.body_medium_size * tokens.typography.line_height_normal)
                .color(tokens.colors.text_secondary)
                .into_node(),
            Row {
                children: vec![
                    FooterLink::new("GitHub", "https://github.com/worka-ai/fission").build(ctx, view),
                    FooterLink::new("Quickstart", "/docs/learn/quickstart/").build(ctx, view),
                ],
                gap: Some(tokens.spacing.m),
                wrap: FlexWrap::Wrap,
                ..Default::default()
            }
            .into_node(),
            Text::new("main - v0.1.0 alpha")
                .size(tokens.typography.font_size_sm)
                .family(tokens.typography.font_family_mono.clone())
                .color(tokens.colors.text_muted)
                .into_node(),
        ],
        gap: Some(tokens.spacing.m),
        flex_shrink: 1.0,
        ..Default::default()
    }
    .into_node()
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

impl Widget<DocsState> for FooterColumn {
    fn build(&self, ctx: &mut BuildCtx<DocsState>, view: &View<DocsState>) -> Node {
        let tokens = &view.env.theme.tokens;
        Column {
            children: std::iter::once(
                Text::new(self.title)
                    .size(tokens.typography.font_size_sm)
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.heading)
                    .into_node(),
            )
            .chain(
                self.links
                    .iter()
                    .map(|(label, href)| FooterLink::new(label, href).build(ctx, view)),
            )
            .collect(),
            gap: Some(tokens.spacing.s),
            ..Default::default()
        }
        .into_node()
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

impl Widget<DocsState> for FooterLink {
    fn build(&self, _ctx: &mut BuildCtx<DocsState>, view: &View<DocsState>) -> Node {
        let tokens = &view.env.theme.tokens;
        let identifier = if self.href.starts_with("http://") || self.href.starts_with("https://") {
            format!("markdown-link:{}", self.href)
        } else {
            format!("site-route:{}", self.href)
        };
        Text::new(self.label)
            .size(tokens.typography.font_size_sm)
            .weight(tokens.typography.font_weight_medium)
            .color(tokens.colors.text_secondary)
            .semantics_identifier(identifier)
            .into_node()
    }
}
