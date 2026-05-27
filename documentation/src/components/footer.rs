use super::home_widgets::semantic_row;
use super::state::DocsState;
use fission::op::{AlignItems, Fill, FlexWrap, JustifyContent, TextAlign};
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
                            FooterColumn::new(
                                "Setup",
                                &[
                                    ("Quickstart", "/docs/learn/quickstart/"),
                                    ("Add targets", "/docs/cookbook/add-platform-targets/"),
                                    ("Project structure", "/docs/guides/app-structure/"),
                                ],
                            )
                            .build(ctx, view),
                            FooterColumn::new(
                                "Learn",
                                &[
                                    ("Overview", "/docs/learn/overview/"),
                                    ("Runtime model", "/docs/learn/runtime-model/"),
                                    ("Widgets", "/docs/guides/layout-and-widgets/"),
                                    ("Design systems", "/docs/guides/design-system/"),
                                    ("Charts", "/docs/charts/overview/"),
                                ],
                            )
                            .build(ctx, view),
                            FooterColumn::new(
                                "Build",
                                &[
                                    (
                                        "Platform shells",
                                        "/docs/guides/platform-shells-cli-and-testing/",
                                    ),
                                    ("Terminal UI", "/docs/guides/terminal-user-interfaces/"),
                                    ("Static sites", "/docs/guides/static-sites/"),
                                    ("Packaging", "/docs/build-and-package/overview/"),
                                ],
                            )
                            .build(ctx, view),
                            FooterColumn::new(
                                "Test",
                                &[
                                    ("Testing lifecycle", "/docs/test-and-debug/overview/"),
                                    ("Diagnostics", "/docs/guides/testing-and-diagnostics/"),
                                    ("Live UI test", "/docs/cookbook/write-a-live-ui-test/"),
                                ],
                            )
                            .build(ctx, view),
                            FooterColumn::new(
                                "Publish",
                                &[
                                    ("Release overview", "/docs/release-and-distribute/overview/"),
                                    (
                                        "Lifecycle details",
                                        "/docs/release-and-distribute/post-build-lifecycle/",
                                    ),
                                    ("CLI reference", "/reference/cli/overview/"),
                                    ("Examples", "/docs/learn/examples-and-targets/"),
                                ],
                            )
                            .build(ctx, view),
                        ],
                        gap: Some(tokens.spacing.xxl),
                        wrap: FlexWrap::Wrap,
                        align_items: AlignItems::Start,
                        justify_content: JustifyContent::Center,
                        ..Default::default()
                    }
                    .into_node(),
                    footer_identity(ctx, view),
                ],
                gap: Some(tokens.spacing.xxl),
                align_items: AlignItems::Center,
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

fn footer_identity(ctx: &mut BuildCtx<DocsState>, view: &View<DocsState>) -> Node {
    let tokens = &view.env.theme.tokens;
    Container::new(
        Column {
            children: vec![
                semantic_row(
                    "site-route:/",
                    vec![
                        Image::asset("/img/fission-mark.svg")
                            .size(tokens.spacing.l, tokens.spacing.l)
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
                    JustifyContent::Center,
                ),
                Text::new("A cross-platform, GPU-accelerated user interface framework for Rust. MIT licensed.")
                    .size(tokens.typography.body_medium_size)
                    .line_height(tokens.typography.body_medium_size * tokens.typography.line_height_normal)
                    .color(tokens.colors.text_secondary)
                    .max_width(tokens.spacing.xxxxl * 7.0)
                    .text_align(TextAlign::Center)
                    .flex_shrink(1.0)
                    .into_node(),
                Text::new("Copyright (c) 2026 Fission")
                    .size(tokens.typography.font_size_sm)
                    .color(tokens.colors.text_muted)
                    .text_align(TextAlign::Center)
                    .into_node(),
                Text::new("Ready to use today. Widget APIs are expected to remain stable; some runtime and shell APIs may change before 1.0.0.")
                    .size(tokens.typography.font_size_sm)
                    .line_height(tokens.typography.font_size_sm * tokens.typography.line_height_normal)
                    .color(tokens.colors.text_muted)
                    .max_width(tokens.spacing.xxxxl * 8.0)
                    .text_align(TextAlign::Center)
                    .flex_shrink(1.0)
                    .into_node(),
                Row {
                    children: vec![
                        FooterLink::new("GitHub", "https://github.com/worka-ai/fission")
                            .build(ctx, view),
                        FooterLink::new("Quickstart", "/docs/learn/quickstart/").build(ctx, view),
                        FooterLink::new("Reference", "/reference/overview/overview/")
                            .build(ctx, view),
                    ],
                    gap: Some(tokens.spacing.m),
                    wrap: FlexWrap::Wrap,
                    justify_content: JustifyContent::Center,
                    ..Default::default()
                }
                .into_node(),
                Text::new("main - v0.1.0 alpha")
                    .size(tokens.typography.font_size_sm)
                    .family(tokens.typography.font_family_mono.clone())
                    .color(tokens.colors.text_muted)
                    .text_align(TextAlign::Center)
                    .into_node(),
            ],
            gap: Some(tokens.spacing.m),
            align_items: AlignItems::Center,
            ..Default::default()
        }
        .into_node(),
    )
    .padding([0.0, 0.0, tokens.spacing.l, 0.0])
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
        Container::new(
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
            .into_node(),
        )
        .width(tokens.spacing.xxxxl * 1.75)
        .flex_shrink(1.0)
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
