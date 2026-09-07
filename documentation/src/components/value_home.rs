use super::home_widgets::{site_semantics, Cta, NavLink, SemanticColumn, SemanticRow};
use super::state::DocsState;
use fission::op::{AlignItems, Fill, FlexWrap, JustifyContent, TextAlign};
use fission::prelude::*;

#[derive(Clone, Debug)]
pub(super) struct HomePageHero;

impl From<HomePageHero> for Widget {
    fn from(_component: HomePageHero) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        SemanticColumn::new(
            "value-home-hero",
            vec![
                SemanticRow::new(
                    "value-home-hero-main",
                    vec![HeroCopy.into(), ProductMap.into()],
                    Some(tokens.spacing.xxxl),
                    FlexWrap::NoWrap,
                    AlignItems::Center,
                    JustifyContent::SpaceBetween,
                )
                .into(),
                HeroProof.into(),
            ],
            Some(tokens.spacing.xxl),
            AlignItems::Stretch,
        )
        .into()
    }
}

#[derive(Clone, Debug)]
struct HeroCopy;

impl From<HeroCopy> for Widget {
    fn from(_copy: HeroCopy) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        SemanticColumn::new(
            "value-home-hero-copy",
            vec![
                Eyebrow::new("A Rust application platform").into(),
                Text::new("Build one Rust product.\nShip it everywhere.")
                    .size(76.0)
                    .line_height(75.0)
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.heading)
                    .max_width(760.0)
                    .semantics_identifier("site-heading-1:top")
                    .into(),
                Text::new("Fission keeps application state, interface code, platform integrations, tests, and release workflow in one Rust codebase. Add native, mobile, web, terminal, and server targets without maintaining a separate product implementation for each one.")
                    .size(tokens.typography.body_large_size)
                    .line_height(tokens.typography.body_large_size * tokens.typography.line_height_relaxed)
                    .color(tokens.colors.text_secondary)
                    .max_width(680.0)
                    .into(),
                Row {
                    children: vec![
                        Cta::new("Build your first app  ↗", "/docs/learn/quickstart/", true).into(),
                        NavLink::new("See why teams choose Fission  ↓", "/#why").into(),
                    ],
                    gap: Some(tokens.spacing.l),
                    wrap: FlexWrap::Wrap,
                    align_items: AlignItems::Center,
                    ..Default::default()
                }
                .into(),
            ],
            Some(tokens.spacing.l),
            AlignItems::Start,
        )
        .into()
    }
}

#[derive(Clone, Debug)]
struct ProductMap;

impl From<ProductMap> for Widget {
    fn from(_map: ProductMap) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        let targets = [
            "macOS", "Web", "iOS", "Linux", "Android", "Windows", "Terminal", "Static", "SSR",
        ]
        .into_iter()
        .map(|label| PlatformTarget { label }.into())
        .collect();
        SemanticColumn::new(
            "value-home-product-map",
            vec![
                SemanticColumn::new(
                    "value-home-map-caption",
                    vec![
                        Text::new("One product model")
                            .size(tokens.typography.font_size_xs)
                            .family(tokens.typography.font_family_mono.clone())
                            .color(tokens.colors.text_muted)
                            .into(),
                        Text::new("Every surface that matters")
                            .size(tokens.typography.font_size_base)
                            .weight(tokens.typography.font_weight_bold)
                            .color(tokens.colors.heading)
                            .into(),
                    ],
                    Some(tokens.spacing.xs),
                    AlignItems::Start,
                )
                .into(),
                Container::new(Column {
                    children: vec![
                        Container::new(Column {
                            children: vec![
                                Image::asset("/img/fission-mark.svg")
                                    .size(42.0, 50.0)
                                    .into(),
                                Text::new("your product")
                                    .size(tokens.typography.font_size_xs)
                                    .family(tokens.typography.font_family_mono.clone())
                                    .color(tokens.colors.on_primary)
                                    .into(),
                            ],
                            gap: Some(tokens.spacing.s),
                            align_items: AlignItems::Center,
                            semantics: Some(site_semantics("value-home-map-core")),
                            ..Default::default()
                        })
                        .padding_all(tokens.spacing.l)
                        .bg_fill(Fill::Solid(tokens.colors.primary))
                        .into(),
                        Row {
                            children: targets,
                            gap: Some(tokens.spacing.s),
                            wrap: FlexWrap::Wrap,
                            justify_content: JustifyContent::Center,
                            semantics: Some(site_semantics("value-home-map-targets")),
                            ..Default::default()
                        }
                        .into(),
                    ],
                    gap: Some(tokens.spacing.l),
                    align_items: AlignItems::Center,
                    semantics: Some(site_semantics("value-home-map-stage")),
                    ..Default::default()
                })
                .padding_all(tokens.spacing.l)
                .border(tokens.colors.border, 1.0)
                .into(),
            ],
            Some(tokens.spacing.m),
            AlignItems::Stretch,
        )
        .into()
    }
}

#[derive(Clone, Debug)]
struct PlatformTarget {
    label: &'static str,
}

impl From<PlatformTarget> for Widget {
    fn from(target: PlatformTarget) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        Container::new(
            Text::new(target.label)
                .size(tokens.typography.font_size_xs)
                .family(tokens.typography.font_family_mono.clone())
                .color(tokens.colors.text_secondary),
        )
        .padding([
            tokens.spacing.s,
            tokens.spacing.s,
            tokens.spacing.xs,
            tokens.spacing.xs,
        ])
        .border(tokens.colors.border, 1.0)
        .into()
    }
}

#[derive(Clone, Debug)]
struct HeroProof;

impl From<HeroProof> for Widget {
    fn from(_proof: HeroProof) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        SemanticRow::new(
            "value-home-hero-proof",
            [
                "One language",
                "One application model",
                "Platform-native delivery",
                "Apache-2.0 licensed",
            ]
            .into_iter()
            .map(|label| {
                Text::new(label)
                    .size(tokens.typography.font_size_xs)
                    .family(tokens.typography.font_family_mono.clone())
                    .color(tokens.colors.text_secondary)
                    .into()
            })
            .collect(),
            Some(0.0),
            FlexWrap::Wrap,
            AlignItems::Center,
            JustifyContent::SpaceBetween,
        )
        .into()
    }
}

#[derive(Clone, Debug)]
pub(super) struct WhySection;

impl From<WhySection> for Widget {
    fn from(_section: WhySection) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        SemanticRow::new(
            "value-home-why",
            vec![
                SectionIndex::new("01 / WHY FISSION", "why").into(),
                SemanticColumn::new(
                    "value-home-why-copy",
                    vec![
                        Text::new("A Fission application has one shared source of product behaviour.")
                            .size(tokens.typography.font_size_base)
                            .weight(tokens.typography.font_weight_bold)
                            .color(tokens.colors.heading)
                            .into(),
                        Text::new("One codebase should mean one place to make product decisions.")
                            .size(tokens.typography.heading1_size)
                            .line_height(tokens.typography.heading1_size * tokens.typography.line_height_heading)
                            .weight(tokens.typography.font_weight_bold)
                            .color(tokens.colors.heading)
                            .into(),
                        Text::new("State changes, navigation, widgets, services, and design rules are expressed once. Platform shells still own the details that genuinely differ—windows, lifecycle, input, signing, and distribution. A product change stays a product change instead of becoming a coordination job across several application teams.")
                            .size(tokens.typography.body_large_size)
                            .line_height(tokens.typography.body_large_size * tokens.typography.line_height_relaxed)
                            .color(tokens.colors.text_secondary)
                            .into(),
                    ],
                    Some(tokens.spacing.l),
                    AlignItems::Start,
                )
                .into(),
            ],
            Some(tokens.spacing.xxxl),
            FlexWrap::NoWrap,
            AlignItems::Start,
            JustifyContent::Start,
        )
        .into()
    }
}

#[derive(Clone, Debug)]
pub(super) struct OutcomesSection;

impl From<OutcomesSection> for Widget {
    fn from(_section: OutcomesSection) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        SemanticColumn::new(
            "value-home-outcomes",
            vec![
                SemanticRow::new(
                    "value-home-section-heading",
                    vec![
                        Eyebrow::new("The practical difference").into(),
                        Text::new("What a shared application model changes in practice.")
                            .size(tokens.typography.heading2_size)
                            .line_height(tokens.typography.heading2_size * tokens.typography.line_height_heading)
                            .weight(tokens.typography.font_weight_bold)
                            .color(tokens.colors.heading)
                            .into(),
                    ],
                    Some(tokens.spacing.xl),
                    FlexWrap::NoWrap,
                    AlignItems::Start,
                    JustifyContent::SpaceBetween,
                )
                .into(),
                Outcome::new("01", "Add a target without starting another application", "The same state, reducers, widgets, and services can reach desktop, mobile, browser, terminal, and server surfaces. Teams implement platform-specific work only where the platform actually requires it.", "Shared implementation → more supported targets").into(),
                Outcome::new("02", "Fix behaviour once, then test each real target", "Shared behaviour is tested at its source. Target tests then verify the boundaries that differ, including input, rendering, accessibility, lifecycle, and packaging.", "Shared tests → less behavioural drift").into(),
                Outcome::new("03", "Keep release work inside the engineering system", "Fission carries target configuration into builds, packages, signing checks, and distribution workflows. Release knowledge remains reviewable and repeatable instead of living in disconnected scripts.", "Explicit delivery → fewer manual hand-offs").into(),
            ],
            Some(0.0),
            AlignItems::Stretch,
        )
        .into()
    }
}

#[derive(Clone, Debug)]
struct Outcome {
    number: &'static str,
    title: &'static str,
    body: &'static str,
    result: &'static str,
}

impl Outcome {
    fn new(
        number: &'static str,
        title: &'static str,
        body: &'static str,
        result: &'static str,
    ) -> Self {
        Self {
            number,
            title,
            body,
            result,
        }
    }
}

impl From<Outcome> for Widget {
    fn from(outcome: Outcome) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        SemanticRow::new(
            "value-home-outcome",
            vec![
                Text::new(outcome.number)
                    .size(tokens.typography.font_size_xs)
                    .family(tokens.typography.font_family_mono.clone())
                    .color(tokens.colors.primary)
                    .into(),
                Column {
                    children: vec![
                        Text::new(outcome.title)
                            .size(tokens.typography.heading_size)
                            .weight(tokens.typography.font_weight_bold)
                            .color(tokens.colors.heading)
                            .into(),
                        Text::new(outcome.body)
                            .size(tokens.typography.body_medium_size)
                            .line_height(
                                tokens.typography.body_medium_size
                                    * tokens.typography.line_height_relaxed,
                            )
                            .color(tokens.colors.text_secondary)
                            .into(),
                    ],
                    gap: Some(tokens.spacing.m),
                    ..Default::default()
                }
                .into(),
                Text::new(outcome.result)
                    .size(tokens.typography.font_size_sm)
                    .family(tokens.typography.font_family_mono.clone())
                    .color(tokens.colors.primary)
                    .text_align(TextAlign::Right)
                    .into(),
            ],
            Some(tokens.spacing.xl),
            FlexWrap::NoWrap,
            AlignItems::Start,
            JustifyContent::SpaceBetween,
        )
        .into()
    }
}

#[derive(Clone, Debug)]
pub(super) struct AudienceSection;

impl From<AudienceSection> for Widget {
    fn from(_section: AudienceSection) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        SemanticColumn::new(
            "value-home-audience",
            vec![
                SectionIndex::new("02 / VALUE AT EVERY SCALE", "value").into(),
                SemanticRow::new(
                    "value-home-audience-heading",
                    vec![
                        Text::new("The same foundation solves different problems at each scale.")
                            .size(tokens.typography.heading2_size)
                            .line_height(tokens.typography.heading2_size * tokens.typography.line_height_heading)
                            .weight(tokens.typography.font_weight_bold)
                            .color(tokens.colors.heading)
                            .into(),
                        Text::new("Developers, delivery teams, and organisations all benefit from removing duplicate application implementations, but in concrete and different ways.")
                            .size(tokens.typography.body_large_size)
                            .line_height(tokens.typography.body_large_size * tokens.typography.line_height_relaxed)
                            .color(tokens.colors.text_secondary)
                            .into(),
                    ],
                    Some(tokens.spacing.xxxl),
                    FlexWrap::NoWrap,
                    AlignItems::Start,
                    JustifyContent::SpaceBetween,
                )
                .into(),
                SemanticRow::new(
                    "value-home-audience-switcher",
                    vec![AudienceTabs.into(), AudiencePanels.into()],
                    Some(tokens.spacing.xxxl),
                    FlexWrap::NoWrap,
                    AlignItems::Start,
                    JustifyContent::Start,
                )
                .into(),
            ],
            Some(tokens.spacing.xxl),
            AlignItems::Stretch,
        )
        .into()
    }
}

#[derive(Clone, Debug)]
struct AudienceTabs;

impl From<AudienceTabs> for Widget {
    fn from(_tabs: AudienceTabs) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        SemanticColumn::new(
            "value-home-audience-tabs",
            [
                ("developers", "Developers", true),
                ("teams", "Teams", false),
                ("organisations", "Organisations", false),
            ]
            .into_iter()
            .map(|(key, label, active)| {
                Button {
                    child: Some(Text::new(label).into()),
                    semantics: Some(site_semantics(format!(
                        "value-home-audience-tab:{key}:{active}"
                    ))),
                    variant: ButtonVariant::Ghost,
                    ..Default::default()
                }
                .into()
            })
            .collect(),
            Some(tokens.spacing.s),
            AlignItems::Stretch,
        )
        .into()
    }
}

#[derive(Clone, Debug)]
struct AudiencePanels;

impl From<AudiencePanels> for Widget {
    fn from(_panels: AudiencePanels) -> Self {
        SemanticColumn::new(
            "value-home-audience-panels",
            vec![
                AudiencePanel::new("developers", true, "For developers", "Use Rust from state changes to rendered output.", "Work with one language and trace one pipeline through actions, reducers, layout, input, rendering, and platform effects. You can investigate the application without switching between unrelated UI frameworks and state models.", &["Carry one set of skills across every supported surface.", "Trace behaviour through explicit state, actions, layout, input, and rendering.", "Use strong types to catch drift before customers do."], "Learn the application model  ↗", "/docs/learn/overview/").into(),
                AudiencePanel::new("teams", false, "For teams", "Review one product change instead of several translations.", "Design, engineering, QA, and release work against the same application structure. A new workflow is implemented once, then its platform-specific boundaries are reviewed and tested explicitly.", &["Align around one source of product behaviour.", "Reuse tests and design intent across target boundaries.", "Review changes as coherent product changes, not parallel rewrites."], "See how teams stay aligned  ↗", "/product/design-systems/").into(),
                AudiencePanel::new("organisations", false, "For organisations", "Support more platforms without duplicating the organisation.", "Platform coverage becomes a capability of the product team rather than a set of isolated codebases with separate ownership, release knowledge, and maintenance schedules.", &["Reduce duplicated implementation and coordination overhead.", "Keep release evidence and platform delivery in one operating model.", "Retain control with an open-source foundation and inspectable architecture."], "Explore the production journey  ↗", "/product/production-lifecycle/").into(),
            ],
            None,
            AlignItems::Stretch,
        )
        .into()
    }
}

#[derive(Clone, Debug)]
struct AudiencePanel {
    key: &'static str,
    active: bool,
    label: &'static str,
    title: &'static str,
    body: &'static str,
    points: &'static [&'static str],
    link: &'static str,
    href: &'static str,
}

impl AudiencePanel {
    #[allow(clippy::too_many_arguments)]
    fn new(
        key: &'static str,
        active: bool,
        label: &'static str,
        title: &'static str,
        body: &'static str,
        points: &'static [&'static str],
        link: &'static str,
        href: &'static str,
    ) -> Self {
        Self {
            key,
            active,
            label,
            title,
            body,
            points,
            link,
            href,
        }
    }
}

impl From<AudiencePanel> for Widget {
    fn from(panel: AudiencePanel) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        SemanticColumn::new(
            format!("value-home-audience-panel:{}:{}", panel.key, panel.active),
            vec![
                Text::new(panel.label)
                    .size(tokens.typography.font_size_xs)
                    .family(tokens.typography.font_family_mono.clone())
                    .color(tokens.colors.primary)
                    .into(),
                Text::new(panel.title)
                    .size(tokens.typography.heading2_size)
                    .line_height(
                        tokens.typography.heading2_size * tokens.typography.line_height_heading,
                    )
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.heading)
                    .into(),
                Text::new(panel.body)
                    .size(tokens.typography.body_large_size)
                    .line_height(
                        tokens.typography.body_large_size * tokens.typography.line_height_relaxed,
                    )
                    .color(tokens.colors.text_secondary)
                    .into(),
                Row {
                    children: panel
                        .points
                        .iter()
                        .map(|point| {
                            Container::new(
                                Text::new(*point)
                                    .size(tokens.typography.body_medium_size)
                                    .line_height(
                                        tokens.typography.body_medium_size
                                            * tokens.typography.line_height_normal,
                                    )
                                    .color(tokens.colors.text_secondary),
                            )
                            .padding([tokens.spacing.m, 0.0, tokens.spacing.m, tokens.spacing.m])
                            .border(tokens.colors.border, 1.0)
                            .flex_grow(1.0)
                            .into()
                        })
                        .collect(),
                    gap: Some(tokens.spacing.l),
                    wrap: FlexWrap::NoWrap,
                    align_items: AlignItems::Stretch,
                    ..Default::default()
                }
                .into(),
                NavLink::new(panel.link, panel.href).into(),
            ],
            Some(tokens.spacing.l),
            AlignItems::Start,
        )
        .into()
    }
}

#[derive(Clone, Debug)]
pub(super) struct HowItWorksSection;

impl From<HowItWorksSection> for Widget {
    fn from(_section: HowItWorksSection) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        SemanticColumn::new(
            "value-home-approach",
            vec![
                SectionIndex::new("03 / THE APPROACH", "approach").into(),
                SemanticRow::new(
                    "value-home-approach-intro",
                    vec![
                        Text::new("The product model is shared.\nThe platform boundary stays explicit.")
                            .size(tokens.typography.heading2_size)
                            .line_height(tokens.typography.heading2_size * tokens.typography.line_height_heading)
                            .weight(tokens.typography.font_weight_bold)
                            .color(tokens.colors.heading)
                            .into(),
                        Text::new("Fission does not pretend every platform is identical. It shares application behaviour and UI intent, then gives each shell responsibility for the operating-system and delivery work that cannot honestly be shared.")
                            .size(tokens.typography.body_large_size)
                            .line_height(tokens.typography.body_large_size * tokens.typography.line_height_relaxed)
                            .color(tokens.colors.text_secondary)
                            .into(),
                    ],
                    Some(tokens.spacing.xxxl),
                    FlexWrap::NoWrap,
                    AlignItems::End,
                    JustifyContent::SpaceBetween,
                )
                .into(),
                SemanticRow::new(
                    "value-home-approach-steps",
                    vec![
                        ApproachStep::new("01", "Write the application in Rust", "Define state, reducers, retained widgets, services, and design-system values together.").into(),
                        ApproachStep::new("02", "Connect target shells", "Use native ownership for windows, events, lifecycle, accessibility, and host capabilities.").into(),
                        ApproachStep::new("03", "Build and test the shipped form", "Exercise the actual target, then package, validate, sign, and distribute its artifact.").into(),
                    ],
                    Some(0.0),
                    FlexWrap::NoWrap,
                    AlignItems::Stretch,
                    JustifyContent::SpaceBetween,
                )
                .into(),
            ],
            Some(tokens.spacing.xxl),
            AlignItems::Stretch,
        )
        .into()
    }
}

#[derive(Clone, Debug)]
struct ApproachStep {
    number: &'static str,
    title: &'static str,
    body: &'static str,
}

impl ApproachStep {
    fn new(number: &'static str, title: &'static str, body: &'static str) -> Self {
        Self {
            number,
            title,
            body,
        }
    }
}

impl From<ApproachStep> for Widget {
    fn from(step: ApproachStep) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        SemanticColumn::new(
            "value-home-approach-step",
            vec![
                Text::new(step.number)
                    .size(tokens.typography.font_size_xs)
                    .family(tokens.typography.font_family_mono.clone())
                    .color(tokens.colors.primary)
                    .into(),
                Text::new(step.title)
                    .size(tokens.typography.heading_size)
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.heading)
                    .into(),
                Text::new(step.body)
                    .size(tokens.typography.body_medium_size)
                    .line_height(
                        tokens.typography.body_medium_size * tokens.typography.line_height_relaxed,
                    )
                    .color(tokens.colors.text_secondary)
                    .into(),
            ],
            Some(tokens.spacing.m),
            AlignItems::Start,
        )
        .into()
    }
}

#[derive(Clone, Debug)]
pub(super) struct ComparisonSection;

impl From<ComparisonSection> for Widget {
    fn from(_section: ComparisonSection) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        SemanticColumn::new(
            "value-home-comparison",
            vec![
                SemanticRow::new(
                    "value-home-comparison-heading",
                    vec![
                        SectionIndex::new("04 / THE OPERATING DIFFERENCE", "comparison").into(),
                        Text::new("What changes when the application is shared?")
                            .size(tokens.typography.heading2_size)
                            .line_height(
                                tokens.typography.heading2_size
                                    * tokens.typography.line_height_heading,
                            )
                            .weight(tokens.typography.font_weight_bold)
                            .color(tokens.colors.heading)
                            .into(),
                    ],
                    Some(tokens.spacing.xxxl),
                    FlexWrap::NoWrap,
                    AlignItems::Start,
                    JustifyContent::SpaceBetween,
                )
                .into(),
                ComparisonRow::new(
                    "Separate framework for each target",
                    "One Fission application",
                    true,
                )
                .into(),
                ComparisonRow::new(
                    "Product behaviour is translated between codebases.",
                    "State, actions, reducers, and interface intent have one source.",
                    false,
                )
                .into(),
                ComparisonRow::new(
                    "Platform teams discover drift late in parallel QA.",
                    "Shared behaviour is tested once; target boundaries are qualified explicitly.",
                    false,
                )
                .into(),
                ComparisonRow::new(
                    "Release knowledge lives in separate tools and hand-offs.",
                    "Build, packaging, signing, and distribution remain part of one workflow.",
                    false,
                )
                .into(),
            ],
            Some(0.0),
            AlignItems::Stretch,
        )
        .into()
    }
}

#[derive(Clone, Debug)]
struct ComparisonRow {
    separate: &'static str,
    fission: &'static str,
    labels: bool,
}

impl ComparisonRow {
    fn new(separate: &'static str, fission: &'static str, labels: bool) -> Self {
        Self {
            separate,
            fission,
            labels,
        }
    }
}

impl From<ComparisonRow> for Widget {
    fn from(row: ComparisonRow) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        SemanticRow::new(
            if row.labels {
                "value-home-comparison-labels"
            } else {
                "value-home-comparison-row"
            },
            vec![
                Text::new(row.separate)
                    .size(if row.labels {
                        tokens.typography.font_size_xs
                    } else {
                        tokens.typography.body_medium_size
                    })
                    .line_height(
                        tokens.typography.body_medium_size * tokens.typography.line_height_normal,
                    )
                    .color(tokens.colors.text_secondary)
                    .into(),
                Text::new(row.fission)
                    .size(if row.labels {
                        tokens.typography.font_size_xs
                    } else {
                        tokens.typography.body_medium_size
                    })
                    .line_height(
                        tokens.typography.body_medium_size * tokens.typography.line_height_normal,
                    )
                    .weight(tokens.typography.font_weight_semibold)
                    .color(if row.labels {
                        tokens.colors.primary
                    } else {
                        tokens.colors.text_primary
                    })
                    .into(),
            ],
            Some(0.0),
            FlexWrap::NoWrap,
            AlignItems::Stretch,
            JustifyContent::Start,
        )
        .into()
    }
}

#[derive(Clone, Debug)]
pub(super) struct StartSection;

impl From<StartSection> for Widget {
    fn from(_section: StartSection) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        SemanticRow::new(
            "value-home-start",
            vec![
                SemanticColumn::new(
                    "value-home-start-copy",
                    vec![
                        SectionIndex::new("05 / START", "start").into(),
                        Text::new("Start with a running app, then follow the real delivery path.")
                            .size(tokens.typography.heading2_size)
                            .line_height(tokens.typography.heading2_size * tokens.typography.line_height_heading)
                            .weight(tokens.typography.font_weight_bold)
                            .color(tokens.colors.heading)
                            .into(),
                        Text::new("The quickstart creates an application, runs it, and explains the model you will extend. From there, the documentation follows development, testing, packaging, and release in order.")
                            .size(tokens.typography.body_large_size)
                            .line_height(tokens.typography.body_large_size * tokens.typography.line_height_relaxed)
                            .color(tokens.colors.text_secondary)
                            .into(),
                        Cta::new("Open the quickstart  ↗", "/docs/learn/quickstart/", true).into(),
                    ],
                    Some(tokens.spacing.l),
                    AlignItems::Start,
                )
                .into(),
                Column {
                    children: vec![MarkdownViewer {
                        markdown: "```sh\ncargo install fission\nfission create my-app\ncd my-app\nfission run\n```\n\nReady: one Rust application, ready for its first target."
                            .to_string(),
                        show_scrollbar: false,
                    }
                    .into()],
                    semantics: Some(site_semantics("value-home-quickstart")),
                    ..Default::default()
                }
                .into(),
            ],
            Some(tokens.spacing.xxxl),
            FlexWrap::NoWrap,
            AlignItems::Center,
            JustifyContent::SpaceBetween,
        )
        .into()
    }
}

#[derive(Clone, Debug)]
struct Eyebrow {
    label: &'static str,
}

impl Eyebrow {
    fn new(label: &'static str) -> Self {
        Self { label }
    }
}

impl From<Eyebrow> for Widget {
    fn from(eyebrow: Eyebrow) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        Text::new(eyebrow.label)
            .size(tokens.typography.font_size_xs)
            .family(tokens.typography.font_family_mono.clone())
            .color(tokens.colors.text_secondary)
            .semantics_identifier("site-value-home-eyebrow")
            .into()
    }
}

#[derive(Clone, Debug)]
struct SectionIndex {
    label: &'static str,
    anchor: &'static str,
}

impl SectionIndex {
    fn new(label: &'static str, anchor: &'static str) -> Self {
        Self { label, anchor }
    }
}

impl From<SectionIndex> for Widget {
    fn from(index: SectionIndex) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        Text::new(index.label)
            .size(tokens.typography.font_size_xs)
            .family(tokens.typography.font_family_mono.clone())
            .color(tokens.colors.text_muted)
            .semantics_identifier(format!("site-anchor:{}", index.anchor))
            .into()
    }
}
