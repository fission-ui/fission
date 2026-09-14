use super::brand_logo::BrandLogo;
use super::home_widgets::site_semantics;
use super::state::DocsState;
use fission::op::{AlignItems, Fill, FlexWrap, JustifyContent};
use fission::prelude::*;

const PRODUCT_LINKS: &[(&str, &str)] = &[
    ("Overview", "/product/overview/"),
    ("Cross-platform apps", "/product/cross-platform-apps/"),
    ("Terminal apps", "/product/terminal-apps/"),
    ("Static and server sites", "/product/static-sites/"),
    ("Charts", "/product/charts/"),
];

const DEVELOPER_LINKS: &[(&str, &str)] = &[
    ("Quickstart", "/docs/learn/quickstart/"),
    ("Documentation", "/docs/"),
    ("Guides", "/docs/guides/layout-and-widgets/"),
    ("Crates", "/crates/"),
    ("API reference", "/reference/overview/overview/"),
];

const RESOURCE_LINKS: &[(&str, &str)] = &[
    ("Blog", "/blog/"),
    ("Widget catalog", "/reference/widgets/catalog/"),
    ("Design systems", "/product/design-systems/"),
    ("Production lifecycle", "/product/production-lifecycle/"),
    ("Developer tools", "/product/developer-tools/"),
];

const PROJECT_LINKS: &[(&str, &str)] = &[
    ("GitHub", "https://github.com/fission-ui/fission"),
    (
        "Contributing",
        "https://github.com/fission-ui/fission/blob/main/CONTRIBUTING.md",
    ),
    (
        "Code of conduct",
        "https://github.com/fission-ui/fission/blob/main/CODE_OF_CONDUCT.md",
    ),
    (
        "Security",
        "https://github.com/fission-ui/fission/blob/main/SECURITY.md",
    ),
];

#[derive(Clone, Debug)]
pub(crate) struct DocsFooter;

impl From<DocsFooter> for Widget {
    fn from(_component: DocsFooter) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        let columns = Row {
            children: vec![
                FooterColumn::new("Product", PRODUCT_LINKS).into(),
                FooterColumn::new("Developers", DEVELOPER_LINKS).into(),
                FooterColumn::new("Resources", RESOURCE_LINKS).into(),
                FooterColumn::new("Project", PROJECT_LINKS).into(),
            ],
            gap: Some(tokens.spacing.xl),
            wrap: FlexWrap::Wrap,
            align_items: AlignItems::Start,
            semantics: Some(site_semantics("site-footer-columns")),
            ..Default::default()
        };
        let top = Row {
            children: vec![FooterIdentity.into(), columns.into()],
            gap: Some(tokens.spacing.xxl),
            wrap: FlexWrap::Wrap,
            align_items: AlignItems::Start,
            justify_content: JustifyContent::SpaceBetween,
            semantics: Some(site_semantics("site-footer-top")),
            ..Default::default()
        };
        let legal = Row {
            children: vec![
                Text::new("© 2026 Fission. Apache 2.0 licensed.")
                    .size(tokens.typography.font_size_sm)
                    .color(tokens.colors.text_muted)
                    .into(),
                Text::new("Fission 0.14.1")
                    .size(tokens.typography.font_size_sm)
                    .family(tokens.typography.font_family_mono.clone())
                    .color(tokens.colors.text_muted)
                    .into(),
            ],
            gap: Some(tokens.spacing.m),
            wrap: FlexWrap::Wrap,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            semantics: Some(site_semantics("site-footer-legal")),
            ..Default::default()
        };
        Container::new(Column {
            children: vec![top.into(), legal.into()],
            gap: Some(tokens.spacing.xl),
            semantics: Some(site_semantics("site-footer")),
            ..Default::default()
        })
        .padding([
            tokens.spacing.xl,
            tokens.spacing.xl,
            tokens.spacing.xxl,
            tokens.spacing.xl,
        ])
        .bg_fill(Fill::Solid(tokens.colors.background))
        .into()
    }
}

#[derive(Clone, Debug)]
struct FooterIdentity;

impl From<FooterIdentity> for Widget {
    fn from(_identity: FooterIdentity) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        Column {
            children: vec![
                BrandLogo::new(tokens.spacing.xl).into(),
                Text::new("One UI model. Every surface.")
                    .size(tokens.typography.body_medium_size)
                    .color(tokens.colors.text_secondary)
                    .into(),
                Text::new("Ready to use today. Widget APIs are stable; some runtime and shell APIs may change before 1.0.")
                    .size(tokens.typography.font_size_sm)
                    .line_height(tokens.typography.font_size_sm * tokens.typography.line_height_normal)
                    .color(tokens.colors.text_muted)
                    .max_width(tokens.spacing.xxxxl * 3.5)
                    .flex_shrink(1.0)
                    .into(),
                FooterLink::new("GitHub", "https://github.com/fission-ui/fission").into(),
            ],
            gap: Some(tokens.spacing.m),
            semantics: Some(site_semantics("site-footer-identity")),
            ..Default::default()
        }
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
        .width(170.0)
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
            .color(tokens.colors.text_secondary)
            .semantics_identifier(identifier)
            .into()
    }
}
