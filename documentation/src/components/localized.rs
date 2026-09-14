use super::brand_logo::BrandLogo;
use super::home_widgets::{SemanticColumn, SemanticRow};
use super::landing::{centred_band, landing_width};
use super::state::DocsState;
use fission::op::{AlignItems, Fill, FlexWrap, JustifyContent};
use fission::prelude::*;

#[derive(Clone, Debug)]
pub(crate) struct LocalizedLandingPage;

impl From<LocalizedLandingPage> for Widget {
    fn from(_page: LocalizedLandingPage) -> Widget {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        Container::new(Column {
            children: vec![LocalizedNav.into(), LocalizedHero.into()],
            gap: Some(0.0),
            ..Default::default()
        })
        .bg_fill(Fill::Solid(tokens.colors.background))
        .into()
    }
}

#[derive(Clone, Debug)]
pub(super) struct LocalizedNav;

impl From<LocalizedNav> for Widget {
    fn from(_nav: LocalizedNav) -> Widget {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        Container::new(SemanticRow::new(
            "site-home-header",
            vec![
                BrandLogo::new(28.0).route("/es/").into(),
                Row {
                    children: vec![
                        LocalizedLink::new("site.nav.platform", "/es/").into(),
                        LocalizedLink::new("site.nav.docs", "/docs/learn/overview/").into(),
                        LocalizedLink::new("site.nav.crates", "/es/crates/").into(),
                        LocalizedLink::new("site.nav.blog", "/blog/").into(),
                    ],
                    gap: Some(tokens.spacing.l),
                    wrap: FlexWrap::Wrap,
                    justify_content: JustifyContent::End,
                    ..Default::default()
                }
                .into(),
            ],
            Some(tokens.spacing.l),
            FlexWrap::NoWrap,
            AlignItems::Center,
            JustifyContent::SpaceBetween,
        ))
        .padding([
            tokens.spacing.m,
            tokens.spacing.xl,
            tokens.spacing.m,
            tokens.spacing.xl,
        ])
        .bg(tokens.colors.surface)
        .border(tokens.colors.border, 1.0)
        .into()
    }
}

#[derive(Clone, Debug)]
struct LocalizedLink {
    key: &'static str,
    href: &'static str,
}

impl LocalizedLink {
    fn new(key: &'static str, href: &'static str) -> Self {
        Self { key, href }
    }
}

impl From<LocalizedLink> for Widget {
    fn from(link: LocalizedLink) -> Widget {
        let (_ctx, view) = fission::build::current::<DocsState>();
        Text::new(TextContent::Key(link.key.into()))
            .size(view.env().theme.tokens.typography.body_medium_size)
            .weight(view.env().theme.tokens.typography.font_weight_bold)
            .semantics_identifier(format!("site-route:{}", link.href))
            .into()
    }
}

/// A translated call-to-action button styled like the home page's buttons.
#[derive(Clone, Debug)]
struct LocalizedButton {
    key: &'static str,
    href: &'static str,
    primary: bool,
}

impl From<LocalizedButton> for Widget {
    fn from(button: LocalizedButton) -> Widget {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        Container::new(
            Text::new(TextContent::Key(button.key.into()))
                .size(tokens.typography.label_large_size)
                .weight(tokens.typography.font_weight_bold)
                .color(if button.primary {
                    tokens.colors.on_primary
                } else {
                    tokens.colors.text_primary
                })
                .semantics_identifier(format!("site-route:{}", button.href)),
        )
        .padding([
            tokens.spacing.l,
            tokens.spacing.l,
            tokens.spacing.m,
            tokens.spacing.m,
        ])
        .bg_fill(Fill::Solid(if button.primary {
            tokens.colors.primary
        } else {
            tokens.colors.surface_raised
        }))
        .border(tokens.colors.border, if button.primary { 0.0 } else { 1.0 })
        .border_radius(tokens.radii.full)
        .into()
    }
}

#[derive(Clone, Debug)]
struct LocalizedHero;

impl From<LocalizedHero> for Widget {
    fn from(_hero: LocalizedHero) -> Widget {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        let copy = SemanticColumn::new(
            "site-localized-hero-copy",
            vec![
                Text::new(TextContent::Key("site.hero.eyebrow".into()))
                    .size(tokens.typography.font_size_sm)
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.primary)
                    .into(),
                Text::new(TextContent::Key("site.hero.title".into()))
                    .size(tokens.typography.heading1_size)
                    .line_height(
                        tokens.typography.heading1_size * tokens.typography.line_height_heading,
                    )
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.heading)
                    .max_width(760.0)
                    .into(),
                Text::new(TextContent::Key("site.hero.body".into()))
                    .size(tokens.typography.body_large_size)
                    .line_height(
                        tokens.typography.body_large_size * tokens.typography.line_height_relaxed,
                    )
                    .color(tokens.colors.text_secondary)
                    .max_width(680.0)
                    .into(),
                Row {
                    children: vec![
                        LocalizedButton {
                            key: "site.hero.start",
                            href: "/docs/learn/quickstart/",
                            primary: true,
                        }
                        .into(),
                        LocalizedButton {
                            key: "site.hero.crates",
                            href: "/es/crates/",
                            primary: false,
                        }
                        .into(),
                    ],
                    gap: Some(tokens.spacing.m),
                    wrap: FlexWrap::Wrap,
                    align_items: AlignItems::Center,
                    ..Default::default()
                }
                .into(),
            ],
            Some(tokens.spacing.l),
            AlignItems::Start,
        );
        Container::new(centred_band(
            "site-localized-hero",
            Container::new(copy)
                .width_length(Length::percent(100.0))
                .max_width(landing_width(tokens))
                .into(),
        ))
        .padding([
            tokens.spacing.xl,
            tokens.spacing.xl,
            tokens.spacing.xxxxl,
            tokens.spacing.xxxxl,
        ])
        .width_length(Length::percent(100.0))
        .bg_fill(Fill::LinearGradient {
            start: (0.0, 0.0),
            end: (1.0, 1.0),
            stops: vec![
                (0.0, tokens.colors.primary_subtle),
                (0.55, tokens.colors.background),
                (1.0, tokens.colors.primary_subtle),
            ],
            extend: Default::default(),
        })
        .into()
    }
}
