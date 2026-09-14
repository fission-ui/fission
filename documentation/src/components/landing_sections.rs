//! The home page sections below the hero: shared foundation, targets, delivery and the closing
//! call to action.

use super::home_widgets::{site_semantics, Cta, SemanticColumn, SemanticRow};
use super::landing::{centred_band, landing_width, text_link};
use super::state::DocsState;
use fission::op::{AlignItems, Fill, FlexWrap, JustifyContent, TextAlign};
use fission::prelude::*;

/// A centred section with a heading, a lead paragraph and its content, lined up to the page width.
struct LandingSection {
    identifier: &'static str,
    anchor: &'static str,
    eyebrow: &'static str,
    title: &'static str,
    lead: &'static str,
    content: Widget,
    tinted: bool,
}

impl From<LandingSection> for Widget {
    fn from(section: LandingSection) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        let title = Text::new(section.title)
            .size(tokens.typography.heading2_size)
            .line_height(tokens.typography.heading2_size * tokens.typography.line_height_heading)
            .weight(tokens.typography.font_weight_bold)
            .color(tokens.colors.heading)
            .text_align(TextAlign::Center);
        // The anchor sits on the small label so in-page links land just above the heading.
        let eyebrow = Text::new(section.eyebrow)
            .size(tokens.typography.font_size_xs)
            .weight(tokens.typography.font_weight_bold)
            .color(tokens.colors.primary)
            .semantics_identifier(format!("site-anchor:{}", section.anchor));
        let inner = Container::new(SemanticColumn::new(
            section.identifier,
            vec![
                eyebrow.into(),
                title.into(),
                Text::new(section.lead)
                    .size(tokens.typography.body_large_size)
                    .line_height(
                        tokens.typography.body_large_size * tokens.typography.line_height_relaxed,
                    )
                    .color(tokens.colors.text_secondary)
                    .text_align(TextAlign::Center)
                    .max_width(640.0)
                    .into(),
                section.content,
            ],
            Some(tokens.spacing.l),
            AlignItems::Center,
        ))
        .width_length(Length::percent(100.0))
        .max_width(landing_width(tokens));
        let mut outer = Container::new(centred_band("site-landing-band", inner.into()))
            .padding([
                tokens.spacing.xl,
                tokens.spacing.xl,
                tokens.spacing.xxxl,
                tokens.spacing.xxxl,
            ])
            .width_length(Length::percent(100.0));
        if section.tinted {
            outer = outer.bg_fill(Fill::Solid(tokens.colors.surface_sunken));
        }
        outer.into()
    }
}

/// A bordered card used by every landing section.
fn card(tokens: &Tokens, identifier: &'static str, children: Vec<Widget>) -> Widget {
    Container::new(SemanticColumn::new(
        identifier,
        children,
        Some(tokens.spacing.m),
        AlignItems::Stretch,
    ))
    .padding_all(tokens.spacing.l)
    .bg_fill(Fill::Solid(tokens.colors.surface_raised))
    .border(tokens.colors.border, 1.0)
    .border_radius(tokens.radii.xl)
    .into()
}

fn icon_heading(
    tokens: &Tokens,
    icon: &'static str,
    title: &'static str,
    body: &'static str,
) -> Widget {
    Row {
        children: vec![
            Container::new(
                Icon::svg(icon)
                    .size(tokens.spacing.l)
                    .color(tokens.colors.primary),
            )
            .padding_all(tokens.spacing.s)
            .bg_fill(Fill::Solid(tokens.colors.primary_subtle))
            .border_radius(tokens.radii.full)
            .into(),
            Column {
                children: vec![
                    Text::new(title)
                        .size(tokens.typography.font_size_lg)
                        .weight(tokens.typography.font_weight_bold)
                        .color(tokens.colors.heading)
                        .into(),
                    Text::new(body)
                        .size(tokens.typography.font_size_base)
                        .line_height(
                            tokens.typography.font_size_base
                                * tokens.typography.line_height_relaxed,
                        )
                        .color(tokens.colors.text_secondary)
                        .into(),
                ],
                gap: Some(tokens.spacing.xs),
                flex_shrink: 1.0,
                ..Default::default()
            }
            .into(),
        ],
        gap: Some(tokens.spacing.m),
        align_items: AlignItems::Start,
        ..Default::default()
    }
    .into()
}

/// Monospaced code shown on a dark panel, identical in light and dark themes.
fn code_panel(tokens: &Tokens, identifier: &'static str, code: &'static str) -> Widget {
    Container::new(
        Text::new(code)
            .size(tokens.typography.font_size_sm)
            .line_height(tokens.typography.font_size_sm * 1.6)
            .family(tokens.typography.font_family_mono.clone())
            .color(Color {
                r: 226,
                g: 228,
                b: 255,
                a: 255,
            })
            .semantics_identifier(identifier),
    )
    .padding_all(tokens.spacing.l)
    .bg_fill(Fill::Solid(Color {
        r: 22,
        g: 22,
        b: 36,
        a: 255,
    }))
    .border_radius(tokens.radii.large)
    .into()
}

fn chip(tokens: &Tokens, icon: &'static str, label: &'static str) -> Widget {
    Container::new(Row {
        children: vec![
            Icon::svg(icon)
                .size(tokens.spacing.m)
                .color(tokens.colors.text_muted)
                .into(),
            Text::new(label)
                .size(tokens.typography.font_size_sm)
                .color(tokens.colors.text_primary)
                .into(),
        ],
        gap: Some(tokens.spacing.s),
        align_items: AlignItems::Center,
        ..Default::default()
    })
    .padding([
        tokens.spacing.m,
        tokens.spacing.m,
        tokens.spacing.s,
        tokens.spacing.s,
    ])
    .bg_fill(Fill::Solid(tokens.colors.surface))
    .border(tokens.colors.border, 1.0)
    .border_radius(tokens.radii.medium)
    .into()
}

const REDUCER_SNIPPET: &str = "#[fission_reducer(Increment)]
fn on_increment(state: &mut CounterState) {
    state.count += 1;
}";

const WIDGET_SNIPPET: &str = "impl From<CounterApp> for Widget {
    fn from(_: CounterApp) -> Self {
        let (ctx, view) = fission::build::current::<CounterState>();
        let increment = with_reducer!(ctx, Increment, on_increment);

        Column {
            gap: Some(16.0),
            children: vec![
                Text::new(format!(\"Count: {}\", view.state().count)).into(),
                Button {
                    on_press: Some(increment),
                    child: Some(Text::new(\"Increment\").into()),
                    ..Default::default()
                }
                .into(),
            ],
            ..Default::default()
        }
        .into()
    }
}";

#[derive(Clone, Debug)]
pub(super) struct FoundationSection;

impl From<FoundationSection> for Widget {
    fn from(_section: FoundationSection) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;

        let move_faster = card(
            tokens,
            "site-landing-feature:faster",
            vec![
                icon_heading(
                    tokens,
                    material::content::bolt::regular(),
                    "Move faster",
                    "Share application logic, state and UI across every target.",
                ),
                code_panel(tokens, "site-landing-code:reducer", REDUCER_SNIPPET),
            ],
        );

        let swatch = |color: Color| -> Widget {
            Container::new(Spacer::default())
                .width(tokens.spacing.l)
                .height(tokens.spacing.l)
                .bg_fill(Fill::Solid(color))
                .border_radius(tokens.radii.full)
                .into()
        };
        let sample_button = |label: &'static str, primary: bool| -> Widget {
            Container::new(
                Text::new(label)
                    .size(tokens.typography.font_size_sm)
                    .weight(tokens.typography.font_weight_semibold)
                    .color(if primary {
                        tokens.colors.on_primary
                    } else {
                        tokens.colors.text_primary
                    }),
            )
            .padding([
                tokens.spacing.l,
                tokens.spacing.l,
                tokens.spacing.s,
                tokens.spacing.s,
            ])
            .bg_fill(Fill::Solid(if primary {
                tokens.colors.primary
            } else {
                tokens.colors.surface
            }))
            .border(tokens.colors.border, if primary { 0.0 } else { 1.0 })
            .border_radius(tokens.radii.medium)
            .into()
        };
        let stay_consistent = card(
            tokens,
            "site-landing-feature:consistent",
            vec![
                icon_heading(
                    tokens,
                    material::image::palette::regular(),
                    "Stay consistent",
                    "Design tokens keep behaviour and look aligned as products grow.",
                ),
                Row {
                    children: vec![
                        sample_button("Primary", true),
                        sample_button("Secondary", false),
                    ],
                    gap: Some(tokens.spacing.s),
                    wrap: FlexWrap::Wrap,
                    ..Default::default()
                }
                .into(),
                Row {
                    children: vec![
                        swatch(tokens.colors.heading),
                        swatch(tokens.colors.primary),
                        swatch(tokens.colors.secondary),
                        swatch(tokens.colors.border_strong),
                        Text::new("One set of tokens for every target")
                            .size(tokens.typography.font_size_sm)
                            .color(tokens.colors.text_secondary)
                            .into(),
                    ],
                    gap: Some(tokens.spacing.s),
                    align_items: AlignItems::Center,
                    wrap: FlexWrap::Wrap,
                    ..Default::default()
                }
                .into(),
            ],
        );

        let own_the_stack = card(
            tokens,
            "site-landing-feature:stack",
            vec![
                icon_heading(
                    tokens,
                    material::maps::layers::regular(),
                    "Own the stack",
                    "Plain Rust types, portable primitives and explicit escape hatches.",
                ),
                Row {
                    children: vec![
                        chip(tokens, material::action::code::regular(), "Plain Rust"),
                        chip(
                            tokens,
                            material::device::widgets::regular(),
                            "Portable widgets",
                        ),
                        chip(
                            tokens,
                            material::action::extension::regular(),
                            "Custom widgets",
                        ),
                        chip(tokens, material::action::verified::regular(), "Apache 2.0"),
                    ],
                    gap: Some(tokens.spacing.s),
                    wrap: FlexWrap::Wrap,
                    ..Default::default()
                }
                .into(),
            ],
        );

        LandingSection {
            identifier: "site-landing-foundation",
            anchor: "why",
            eyebrow: "Why Fission",
            title: "One foundation. Less repeated work.",
            lead: "Tangible benefits for developers, teams and organisations.",
            content: SemanticRow::new(
                "site-landing-feature-grid",
                vec![move_faster, stay_consistent, own_the_stack],
                Some(tokens.spacing.l),
                FlexWrap::Wrap,
                AlignItems::Stretch,
                JustifyContent::Center,
            )
            .into(),
            tinted: false,
        }
        .into()
    }
}

#[derive(Clone, Copy, Debug)]
struct Target {
    label: &'static str,
    icon: fn() -> &'static str,
}

const TARGETS: [Target; 9] = [
    Target {
        label: "macOS",
        icon: material::hardware::laptop_mac::regular,
    },
    Target {
        label: "Windows",
        icon: material::hardware::desktop_windows::regular,
    },
    Target {
        label: "Linux",
        icon: material::hardware::computer::regular,
    },
    Target {
        label: "Web",
        icon: material::action::language::regular,
    },
    Target {
        label: "Android",
        icon: material::hardware::smartphone::regular,
    },
    Target {
        label: "iOS",
        icon: material::hardware::phone_iphone::regular,
    },
    Target {
        label: "Terminal",
        icon: material::action::terminal::regular,
    },
    Target {
        label: "Static site",
        icon: material::action::article::regular,
    },
    Target {
        label: "SSR",
        icon: material::action::dns::regular,
    },
];

#[derive(Clone, Debug)]
pub(super) struct TargetsSection;

impl From<TargetsSection> for Widget {
    fn from(_section: TargetsSection) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        let tile = |target: Target| -> Widget {
            Container::new(Column {
                children: vec![
                    Icon::svg((target.icon)())
                        .size(tokens.spacing.xl)
                        .color(tokens.colors.primary)
                        .into(),
                    Text::new(target.label)
                        .size(tokens.typography.font_size_sm)
                        .weight(tokens.typography.font_weight_medium)
                        .color(tokens.colors.text_primary)
                        .into(),
                ],
                gap: Some(tokens.spacing.s),
                align_items: AlignItems::Center,
                ..Default::default()
            })
            .padding_all(tokens.spacing.m)
            .width(tokens.spacing.xxxxl * 1.15)
            .bg_fill(Fill::Solid(tokens.colors.surface_raised))
            .border(tokens.colors.border, 1.0)
            .border_radius(tokens.radii.large)
            .into()
        };
        let model = Container::new(SemanticColumn::new(
            "site-landing-model",
            vec![
                Image::asset("/img/fission-mark.svg")
                    .size(44.0, 52.0)
                    .into(),
                Text::new("Fission")
                    .size(tokens.typography.font_size_lg)
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.heading)
                    .into(),
                Text::new("One app model\nPer-target shells\nOne toolchain")
                    .size(tokens.typography.font_size_sm)
                    .line_height(tokens.typography.font_size_sm * 1.6)
                    .color(tokens.colors.text_secondary)
                    .text_align(TextAlign::Center)
                    .into(),
            ],
            Some(tokens.spacing.s),
            AlignItems::Center,
        ))
        .padding_all(tokens.spacing.l)
        .bg_fill(Fill::Solid(tokens.colors.surface_raised))
        .border(tokens.colors.primary, 1.0)
        .border_radius(tokens.radii.xl)
        .into();

        LandingSection {
            identifier: "site-landing-targets",
            anchor: "how",
            eyebrow: "How it works",
            title: "Different targets. One way of working.",
            lead: "Write your app once as plain Rust, then run it on every surface with the fission command.",
            content: SemanticRow::new(
                "site-landing-targets-flow",
                vec![
                    Column {
                        children: vec![
                            Text::new("Your app")
                                .size(tokens.typography.font_size_sm)
                                .weight(tokens.typography.font_weight_semibold)
                                .color(tokens.colors.text_secondary)
                                .into(),
                            code_panel(tokens, "site-landing-code:widget", WIDGET_SNIPPET),
                        ],
                        gap: Some(tokens.spacing.s),
                        ..Default::default()
                    }
                    .into(),
                    model,
                    Column {
                        children: vec![
                            Text::new("Targets")
                                .size(tokens.typography.font_size_sm)
                                .weight(tokens.typography.font_weight_semibold)
                                .color(tokens.colors.text_secondary)
                                .into(),
                            SemanticRow::new(
                                "site-landing-target-grid",
                                TARGETS.iter().copied().map(tile).collect(),
                                Some(tokens.spacing.s),
                                FlexWrap::Wrap,
                                AlignItems::Stretch,
                                JustifyContent::Start,
                            )
                            .into(),
                        ],
                        gap: Some(tokens.spacing.s),
                        ..Default::default()
                    }
                    .into(),
                ],
                Some(tokens.spacing.xl),
                FlexWrap::Wrap,
                AlignItems::Center,
                JustifyContent::Center,
            )
            .into(),
            tinted: true,
        }
        .into()
    }
}

#[derive(Clone, Debug)]
pub(super) struct DeliverySection;

impl From<DeliverySection> for Widget {
    fn from(_section: DeliverySection) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        let item = |icon: &'static str, title: &'static str, body: &'static str| -> Widget {
            Container::new(icon_heading(tokens, icon, title, body))
                .max_width(tokens.spacing.xxxxl * 3.0)
                .into()
        };
        let strip = Container::new(SemanticRow::new(
            "site-landing-delivery",
            vec![
                Column {
                    children: vec![
                        Text::new("Built for real delivery")
                            .size(tokens.typography.heading_size)
                            .weight(tokens.typography.font_weight_bold)
                            .color(tokens.colors.heading)
                            .into(),
                        Text::new(
                            "What it takes to ship and maintain high-quality applications on every target.",
                        )
                        .size(tokens.typography.font_size_base)
                        .line_height(tokens.typography.font_size_base * tokens.typography.line_height_relaxed)
                        .color(tokens.colors.text_secondary)
                        .into(),
                    ],
                    gap: Some(tokens.spacing.s),
                    ..Default::default()
                }
                .into(),
                item(
                    material::communication::hub::regular(),
                    "Shared state and UI",
                    "One codebase with consistent behaviour across targets.",
                ),
                item(
                    material::device::devices::regular(),
                    "Native target integration",
                    "Notifications, biometrics, camera, deep links and more where the platform allows.",
                ),
                item(
                    material::action::accessibility_new::regular(),
                    "Accessible by default",
                    "Semantics, keyboard navigation and focus handling built in.",
                ),
                item(
                    material::action::rocket_launch::regular(),
                    "Tested and packaged",
                    "Drive real apps in tests, then package and publish with one command.",
                ),
            ],
            Some(tokens.spacing.xl),
            FlexWrap::Wrap,
            AlignItems::Start,
            JustifyContent::SpaceBetween,
        ))
        .width_length(Length::percent(100.0))
        .max_width(landing_width(tokens))
        .padding_all(tokens.spacing.xl)
        .bg_fill(Fill::Solid(tokens.colors.surface_raised))
        .border(tokens.colors.border, 1.0)
        .border_radius(tokens.radii.xxl);
        Container::new(centred_band("site-landing-band", strip.into()))
            .padding([
                tokens.spacing.xl,
                tokens.spacing.xl,
                tokens.spacing.xxxl,
                tokens.spacing.l,
            ])
            .width_length(Length::percent(100.0))
            .into()
    }
}

#[derive(Clone, Debug)]
pub(super) struct CtaBand;

impl From<CtaBand> for Widget {
    fn from(_band: CtaBand) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        let band = Container::new(SemanticRow::new(
            "site-landing-cta",
            vec![
                Column {
                    children: vec![
                        Text::new("Ready to build without limits?")
                            .size(tokens.typography.heading_size)
                            .weight(tokens.typography.font_weight_bold)
                            .color(Color::WHITE)
                            .into(),
                        Text::new(
                            "Install the fission command and have an app running in a minute.",
                        )
                        .size(tokens.typography.font_size_base)
                        .color(Color::WHITE.with_alpha(220))
                        .into(),
                    ],
                    gap: Some(tokens.spacing.xs),
                    ..Default::default()
                }
                .into(),
                Row {
                    children: vec![
                        Cta::new("Get started  →", "/docs/learn/quickstart/", false).into(),
                        text_link("Read the docs", "/docs/"),
                    ],
                    gap: Some(tokens.spacing.l),
                    align_items: AlignItems::Center,
                    wrap: FlexWrap::Wrap,
                    semantics: Some(site_semantics("site-landing-cta-actions")),
                    ..Default::default()
                }
                .into(),
            ],
            Some(tokens.spacing.l),
            FlexWrap::Wrap,
            AlignItems::Center,
            JustifyContent::SpaceBetween,
        ))
        .width_length(Length::percent(100.0))
        .max_width(landing_width(tokens))
        .padding_all(tokens.spacing.xl)
        .bg_fill(Fill::LinearGradient {
            start: (0.0, 0.0),
            end: (1.0, 0.0),
            stops: vec![(0.0, tokens.colors.primary), (1.0, tokens.colors.secondary)],
            extend: Default::default(),
        })
        .border_radius(tokens.radii.xxl);
        Container::new(centred_band("site-landing-band", band.into()))
            .padding([
                tokens.spacing.xl,
                tokens.spacing.xl,
                tokens.spacing.l,
                tokens.spacing.xxxl,
            ])
            .width_length(Length::percent(100.0))
            .into()
    }
}
