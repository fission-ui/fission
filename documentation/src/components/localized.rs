use super::brand_logo::BrandLogo;
use super::home_widgets::{page_fill, SemanticRow};
use super::state::DocsState;
use fission::op::{AlignItems, FlexWrap, JustifyContent};
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
        .min_height_length(Length::vh(100.0))
        .bg_fill(page_fill(tokens))
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

#[derive(Clone, Debug)]
struct LocalizedHero;

impl From<LocalizedHero> for Widget {
    fn from(_hero: LocalizedHero) -> Widget {
        Responsive::new(LocalizedHeroLayout { compact: false })
            .id(WidgetId::explicit("localized.hero.responsive"))
            .case(ResponsiveCase::max_width(
                760.0,
                LocalizedHeroLayout { compact: true },
            ))
            .into()
    }
}

#[derive(Clone, Debug)]
struct LocalizedHeroLayout {
    compact: bool,
}

impl From<LocalizedHeroLayout> for Widget {
    fn from(layout: LocalizedHeroLayout) -> Widget {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        Container::new(Column {
            children: vec![
                Text::new(TextContent::Key("site.hero.eyebrow".into()))
                    .size(tokens.typography.font_size_sm)
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.primary)
                    .into(),
                Text::new(TextContent::Key("site.hero.title".into()))
                    .size(if layout.compact { 50.0 } else { 76.0 })
                    .line_height(if layout.compact { 52.0 } else { 75.0 })
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.heading)
                    .max_width(760.0)
                    .into(),
                Text::new(TextContent::Key("site.hero.body".into()))
                    .size(if layout.compact { 17.0 } else { 20.0 })
                    .line_height(if layout.compact { 28.0 } else { 32.0 })
                    .color(tokens.colors.text_secondary)
                    .max_width(760.0)
                    .into(),
                Row {
                    children: vec![
                        LocalizedLink::new("site.hero.start", "/docs/learn/quickstart/").into(),
                        LocalizedLink::new("site.hero.crates", "/es/crates/").into(),
                    ],
                    gap: Some(tokens.spacing.l),
                    wrap: FlexWrap::Wrap,
                    ..Default::default()
                }
                .into(),
            ],
            gap: Some(tokens.spacing.l),
            ..Default::default()
        })
        .width_length(Length::clamp(
            Length::points(280.0),
            Length::percent(100.0),
            Length::points(1304.0),
        ))
        .padding_lengths(if layout.compact {
            [
                Length::points(tokens.spacing.xl),
                Length::points(tokens.spacing.l),
                Length::points(tokens.spacing.xxxl),
                Length::points(tokens.spacing.l),
            ]
        } else {
            [
                Length::points(tokens.spacing.xxxxl),
                Length::points(tokens.spacing.xl),
                Length::points(tokens.spacing.xxxxl),
                Length::points(tokens.spacing.xl),
            ]
        })
        .into()
    }
}
