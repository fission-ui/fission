//! The home page hero: headline, calls to action, install command and a composite of real
//! screenshots of Fission examples on several surfaces.

use super::home_widgets::{site_semantics, Cta, NavLink, SemanticColumn, SemanticRow};
use super::state::DocsState;
use fission::op::{AlignItems, BoxShadow, Fill, FlexWrap, JustifyContent};
use fission::prelude::*;

/// The page width every landing section lines up to.
pub(super) fn landing_width(tokens: &Tokens) -> f32 {
    tokens.spacing.xxxxl * 12.0
}

#[derive(Clone, Debug)]
pub(super) struct LandingHero;

impl From<LandingHero> for Widget {
    fn from(_hero: LandingHero) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        Container::new(centred_band(
            "site-landing-hero",
            Container::new(SemanticRow::new(
                "site-landing-hero-main",
                vec![HeroCopy.into(), SurfaceComposite.into()],
                Some(tokens.spacing.xxxl),
                FlexWrap::Wrap,
                AlignItems::Center,
                JustifyContent::SpaceBetween,
            ))
            .width_length(Length::percent(100.0))
            .max_width(landing_width(tokens))
            .into(),
        ))
        .padding([
            tokens.spacing.xl,
            tokens.spacing.xl,
            tokens.spacing.xxxl,
            tokens.spacing.xxxl,
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

/// Centres a section's content column inside its full-width band and names the band for styling.
pub(super) fn centred_band(identifier: &'static str, child: Widget) -> Widget {
    Column {
        children: vec![child],
        align_items: AlignItems::Center,
        flex_grow: 1.0,
        semantics: Some(site_semantics(identifier)),
        ..Default::default()
    }
    .into()
}

#[derive(Clone, Debug)]
struct HeroCopy;

impl From<HeroCopy> for Widget {
    fn from(_copy: HeroCopy) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        SemanticColumn::new(
            "site-landing-hero-copy",
            vec![
                Text::new("ONE UI MODEL. EVERY SURFACE.")
                    .size(tokens.typography.font_size_sm)
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.primary)
                    .into(),
                RichText {
                    runs: vec![
                        RichTextRun::new("Build once.\n")
                            .size(68.0)
                            .line_height(70.0)
                            .weight(tokens.typography.font_weight_bold)
                            .color(tokens.colors.heading),
                        RichTextRun::new("Deliver everywhere.")
                            .size(68.0)
                            .line_height(70.0)
                            .weight(tokens.typography.font_weight_bold)
                            .color(tokens.colors.heading),
                    ],
                    max_width: Some(620.0),
                    semantics: Some(site_semantics("site-heading-1:top")),
                    ..Default::default()
                }
                .into(),
                Text::new(
                    "Fission gives Rust teams one coherent way to build polished applications for \
                     desktop, web, mobile, terminal, static sites and server-rendered sites, from one \
                     codebase and one widget model.",
                )
                .size(tokens.typography.body_large_size)
                .line_height(tokens.typography.body_large_size * tokens.typography.line_height_relaxed)
                .color(tokens.colors.text_secondary)
                .max_width(580.0)
                .into(),
                Row {
                    children: vec![
                        Cta::new("Get started  →", "/docs/learn/quickstart/", true).into(),
                        Cta::new("See how it works", "/#how", false).into(),
                    ],
                    gap: Some(tokens.spacing.m),
                    wrap: FlexWrap::Wrap,
                    align_items: AlignItems::Center,
                    ..Default::default()
                }
                .into(),
                InstallCommand.into(),
                ProofPoints.into(),
            ],
            Some(tokens.spacing.l),
            AlignItems::Start,
        )
        .into()
    }
}

#[derive(Clone, Debug)]
struct InstallCommand;

impl From<InstallCommand> for Widget {
    fn from(_command: InstallCommand) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        Container::new(SemanticRow::new(
            "site-landing-install",
            vec![
                Text::new("$")
                    .size(tokens.typography.font_size_base)
                    .family(tokens.typography.font_family_mono.clone())
                    .color(tokens.colors.primary)
                    .into(),
                Text::new("cargo install cargo-fission")
                    .size(tokens.typography.font_size_base)
                    .family(tokens.typography.font_family_mono.clone())
                    .color(tokens.colors.text_primary)
                    .semantics_identifier("site-landing-install-command")
                    .into(),
                Icon::svg(material::content::content_copy::regular())
                    .size(tokens.spacing.m)
                    .color(tokens.colors.text_muted)
                    .into(),
            ],
            Some(tokens.spacing.m),
            FlexWrap::NoWrap,
            AlignItems::Center,
            JustifyContent::Start,
        ))
        .padding([
            tokens.spacing.l,
            tokens.spacing.l,
            tokens.spacing.m,
            tokens.spacing.m,
        ])
        .bg_fill(Fill::Solid(tokens.colors.surface_raised))
        .border(tokens.colors.border, 1.0)
        .border_radius(tokens.radii.large)
        .into()
    }
}

#[derive(Clone, Debug)]
struct ProofPoints;

impl From<ProofPoints> for Widget {
    fn from(_points: ProofPoints) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        let point = |icon: &'static str, title: &'static str, detail: &'static str| -> Widget {
            Row {
                children: vec![
                    Icon::svg(icon)
                        .size(tokens.spacing.l)
                        .color(tokens.colors.primary)
                        .into(),
                    Column {
                        children: vec![
                            Text::new(title)
                                .size(tokens.typography.font_size_sm)
                                .weight(tokens.typography.font_weight_semibold)
                                .color(tokens.colors.text_primary)
                                .into(),
                            Text::new(detail)
                                .size(tokens.typography.font_size_xs)
                                .color(tokens.colors.text_secondary)
                                .into(),
                        ],
                        gap: Some(2.0),
                        ..Default::default()
                    }
                    .into(),
                ],
                gap: Some(tokens.spacing.s),
                align_items: AlignItems::Center,
                ..Default::default()
            }
            .into()
        };
        SemanticRow::new(
            "site-landing-proof",
            vec![
                point(
                    material::action::code::regular(),
                    "Plain Rust",
                    "No DSL or bridge",
                ),
                point(
                    material::device::devices::regular(),
                    "Nine targets",
                    "One codebase",
                ),
                point(
                    material::action::verified::regular(),
                    "Production ready",
                    "Shipping today",
                ),
            ],
            Some(tokens.spacing.l),
            FlexWrap::Wrap,
            AlignItems::Center,
            JustifyContent::Start,
        )
        .into()
    }
}

/// Real screenshots of examples arranged as the surfaces one Fission app runs on.
#[derive(Clone, Debug)]
struct SurfaceComposite;

impl From<SurfaceComposite> for Widget {
    fn from(_composite: SurfaceComposite) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        let frame = |image: &'static str, label: &'static str, width: f32, height: f32| -> Widget {
            Column {
                children: vec![
                    SurfaceLabel { label }.into(),
                    Container::new(Image::asset(image).size(width, height))
                        .bg_fill(Fill::Solid(tokens.colors.surface_raised))
                        .border(tokens.colors.border, 1.0)
                        .border_radius(tokens.radii.large)
                        .shadow(BoxShadow {
                            spread_radius: 0.0,
                            inset: false,
                            color: tokens.colors.text_primary.with_alpha(36),
                            blur_radius: 28.0,
                            offset: (0.0, 14.0),
                        })
                        .into(),
                ],
                gap: Some(tokens.spacing.xs),
                ..Default::default()
            }
            .into()
        };
        let place = |left: f32, top: f32, child: Widget| -> Widget {
            Positioned {
                left: Some(left),
                top: Some(top),
                child: Some(child),
                ..Default::default()
            }
            .into()
        };
        let stack = Container::new(ZStack {
            children: vec![
                place(
                    70.0,
                    0.0,
                    frame("/img/examples/inbox.png", "Desktop", 448.0, 280.0),
                ),
                place(
                    0.0,
                    232.0,
                    frame("/img/examples/terminal.png", "Terminal", 264.0, 168.0),
                ),
                place(
                    214.0,
                    262.0,
                    frame("/img/examples/widget-gallery.png", "Web", 300.0, 188.0),
                ),
                place(
                    472.0,
                    46.0,
                    frame(
                        "/img/examples/inbox-compact.png",
                        "Compact layout",
                        132.0,
                        286.0,
                    ),
                ),
            ],
            ..Default::default()
        })
        .width(610.0)
        .height(500.0);
        Column {
            children: vec![stack.into()],
            semantics: Some(site_semantics("site-landing-surfaces")),
            ..Default::default()
        }
        .into()
    }
}

#[derive(Clone, Debug)]
struct SurfaceLabel {
    label: &'static str,
}

impl From<SurfaceLabel> for Widget {
    fn from(component: SurfaceLabel) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        Container::new(
            Text::new(component.label)
                .size(tokens.typography.font_size_xs)
                .weight(tokens.typography.font_weight_semibold)
                .color(tokens.colors.text_primary),
        )
        .padding([tokens.spacing.s, tokens.spacing.s, 2.0, 2.0])
        .bg_fill(Fill::Solid(tokens.colors.surface_raised))
        .border(tokens.colors.border, 1.0)
        .border_radius(tokens.radii.full)
        .into()
    }
}

/// A "Read the docs" style text link, kept here so sections can share it.
pub(super) fn text_link(label: &'static str, href: &'static str) -> Widget {
    NavLink::new(label, href).into()
}
