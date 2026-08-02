use super::home_widgets::{
    CenteredSection, ChartImageCard, Cta, ExampleCard, LinkCard, NavLink, Pill, SectionHeader,
    SemanticColumn, SemanticRow, ShellSection, TargetRowCard,
};
use super::state::DocsState;
use fission::op::{AlignItems, Fill, FlexWrap, JustifyContent, TextAlign};
use fission::prelude::*;

#[derive(Clone, Debug)]
pub(super) struct HomePageHero;

impl From<HomePageHero> for Widget {
    fn from(_component: HomePageHero) -> Self {
        Responsive::new(HomeHeroDesktop)
            .id(WidgetId::explicit("home.hero.responsive"))
            .case(ResponsiveCase::max_width(760.0, HomeHeroPhone))
            .into()
    }
}

#[derive(Clone, Debug)]
struct HomeHeroDesktop;

impl From<HomeHeroDesktop> for Widget {
    fn from(_component: HomeHeroDesktop) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        SemanticRow::new(
            "site-home-hero",
            vec![
                Container::new(HomeHeroCopy { compact: false })
                    .flex_grow(1.0)
                    .into(),
                Container::new(PlatformAtlas)
                    .width_length(Length::clamp(
                        Length::points(360.0),
                        Length::percent(44.0),
                        Length::points(560.0),
                    ))
                    .flex_shrink(1.0)
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
struct HomeHeroPhone;

impl From<HomeHeroPhone> for Widget {
    fn from(_component: HomeHeroPhone) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        SemanticColumn::new(
            "site-home-hero",
            vec![HomeHeroCopy { compact: true }.into(), PlatformAtlas.into()],
            Some(tokens.spacing.xxl),
            AlignItems::Stretch,
        )
        .into()
    }
}

#[derive(Clone, Debug)]
struct HomeHeroCopy {
    compact: bool,
}

impl From<HomeHeroCopy> for Widget {
    fn from(copy: HomeHeroCopy) -> Widget {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        Column {
            children: vec![
                Text::new("RUST APPLICATION PLATFORM")
                    .size(tokens.typography.font_size_sm)
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.primary)
                    .into(),
                Text::new("One model.\nEvery surface.")
                    .size(if copy.compact { 50.0 } else { 76.0 })
                    .line_height(if copy.compact { 52.0 } else { 75.0 })
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.heading)
                    .max_width(650.0)
                    .semantics_identifier("site-home-hero-title")
                    .into(),
                Text::new("Build, test, package, and release production apps in Rust—from native windows and mobile devices to the web, terminal, and server.")
                    .size(if copy.compact { 17.0 } else { 20.0 })
                    .line_height(if copy.compact { 28.0 } else { 32.0 })
                    .color(tokens.colors.text_secondary)
                    .max_width(650.0)
                    .into(),
                Row {
                    children: vec![
                        Cta::new("Start building  →", "/docs/learn/quickstart/", true).into(),
                        Cta::new("Explore crates", "/crates/", false).into(),
                    ],
                    gap: Some(tokens.spacing.m),
                    wrap: FlexWrap::Wrap,
                    ..Default::default()
                }.into(),
            ],
            gap: Some(tokens.spacing.l),
            ..Default::default()
        }.into()
    }
}

#[derive(Clone, Debug)]
pub(super) struct PlatformAtlas;

impl From<PlatformAtlas> for Widget {
    fn from(_atlas: PlatformAtlas) -> Widget {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        Container::new(Column {
            children: vec![
                Row {
                    children: vec![
                        AtlasTarget::new("Android", "A").into(),
                        AtlasTarget::new("Web", "◎").into(),
                        AtlasTarget::new("iOS", "●").into(),
                    ],
                    gap: Some(tokens.spacing.m),
                    justify_content: JustifyContent::SpaceBetween,
                    ..Default::default()
                }
                .into(),
                Container::new(Image::asset("/img/fission-mark.svg").size(92.0, 108.0))
                    .padding_lengths(Length::all(Length::points(tokens.spacing.xl)))
                    .bg(tokens.colors.primary)
                    .border_radius(tokens.radii.xl)
                    .into(),
                Row {
                    children: vec![
                        AtlasTarget::new("Linux", "L").into(),
                        AtlasTarget::new("macOS", "M").into(),
                        AtlasTarget::new("Windows", "⊞").into(),
                    ],
                    gap: Some(tokens.spacing.m),
                    justify_content: JustifyContent::SpaceBetween,
                    ..Default::default()
                }
                .into(),
            ],
            gap: Some(tokens.spacing.l),
            align_items: AlignItems::Center,
            semantics: Some(super::home_widgets::site_semantics("site-platform-atlas")),
            ..Default::default()
        })
        .width_length(Length::percent(100.0))
        .padding_lengths(Length::all(Length::points(tokens.spacing.xl)))
        .bg(tokens.colors.primary_subtle)
        .border(tokens.colors.border, 1.0)
        .border_radius(tokens.radii.xl)
        .min_height_length(Length::points(410.0))
        .into()
    }
}

#[derive(Clone, Debug)]
struct AtlasTarget {
    label: &'static str,
    glyph: &'static str,
}

impl AtlasTarget {
    fn new(label: &'static str, glyph: &'static str) -> Self {
        Self { label, glyph }
    }
}

impl From<AtlasTarget> for Widget {
    fn from(target: AtlasTarget) -> Widget {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        Container::new(Column {
            children: vec![
                Text::new(target.glyph)
                    .size(20.0)
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.primary)
                    .into(),
                Text::new(target.label)
                    .size(11.0)
                    .color(tokens.colors.text_muted)
                    .into(),
            ],
            gap: Some(tokens.spacing.xs),
            align_items: AlignItems::Center,
            semantics: Some(super::home_widgets::site_semantics(format!(
                "site-atlas-target:{}",
                target.label.to_lowercase()
            ))),
            ..Default::default()
        })
        .width_length(Length::points(104.0))
        .padding_lengths(Length::all(Length::points(tokens.spacing.m)))
        .bg(tokens.colors.surface)
        .border_radius(tokens.radii.large)
        .into()
    }
}
#[derive(Clone, Debug)]
pub(super) struct ProofStrip;

impl From<ProofStrip> for Widget {
    fn from(_component: ProofStrip) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        SemanticColumn::new(
            "site-home-signals",
            vec![
                SectionHeader::new(
                    "One framework. Nine targets.",
                    "Build the product once. Let every surface feel at home.",
                    "Fission keeps the product model shared while each target owns the details that make it belong: windows, input, packaging, accessibility, lifecycle, and distribution.",
                )
                .into(),
                Row {
                    children: vec![
                        LinkCard::new(
                            "Build",
                            "One product model",
                            "State, reducers, widgets, services, and design systems stay together in Rust.",
                            "Learn the model →",
                            "/docs/learn/overview/",
                        )
                        .into(),
                        LinkCard::new(
                            "Render",
                            "A real UI pipeline",
                            "Deterministic layout, semantics, input, paint, and GPU rendering stay inspectable.",
                            "See the pipeline →",
                            "/docs/learn/rendering-pipeline/",
                        )
                        .into(),
                        LinkCard::new(
                            "Reach",
                            "Every surface",
                            "Native desktop, mobile, Web, Terminal, Static site, and SSR are first-class targets.",
                            "Browse targets →",
                            "/product/cross-platform-apps/",
                        )
                        .into(),
                        LinkCard::new(
                            "Ship",
                            "A complete release path",
                            "Test, package, sign, publish, roll out, and keep the receipts from one toolchain.",
                            "See the release path →",
                            "/product/production-lifecycle/",
                        )
                        .into(),
                    ],
                    gap: Some(tokens.spacing.m),
                    wrap: FlexWrap::Wrap,
                    justify_content: JustifyContent::Center,
                    ..Default::default()
                }
                .into(),
            ],
            Some(tokens.spacing.xl),
            AlignItems::Center,
        )
        .into()
    }
}
#[derive(Clone, Debug)]
pub(super) struct LifecycleSection;

impl From<LifecycleSection> for Widget {
    fn from(_component: LifecycleSection) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        ShellSection::new(
            Column {
                children: vec![
                    Row {
                        children: vec![
                            Column {
                                children: vec![
                                    Text::new("Application lifecycle")
                                        .size(tokens.typography.font_size_sm)
                                        .weight(tokens.typography.font_weight_bold)
                                        .color(tokens.colors.secondary)
                                        .into(),
                                    Text::new("From first run to store rollout.")
                                        .size(tokens.typography.heading2_size)
                                        .family(tokens.typography.font_family_serif.clone())
                                        .line_height(tokens.typography.heading2_size * tokens.typography.line_height_heading)
                                        .weight(tokens.typography.font_weight_bold)
                                        .color(tokens.colors.heading)
                                        .into(),
                                ],
                                gap: Some(tokens.spacing.m),
                                flex_grow: 1.0,
                                ..Default::default()
                            }
                            .into(),
                            Text::new("The docs now follow the path teams actually take: setup, develop, test, debug, package, sign, release, distribute, and keep receipts for automation.")
                                .size(tokens.typography.body_large_size)
                                .line_height(tokens.typography.body_large_size * tokens.typography.line_height_relaxed)
                                .color(tokens.colors.text_secondary)
                                .flex_grow(1.0)
                                .into(),
                        ],
                        gap: Some(tokens.spacing.xl),
                        wrap: FlexWrap::Wrap,
                        align_items: AlignItems::Start,
                        ..Default::default()
                    }
                    .into(),
                    Row {
                        children: vec![
                            LifecycleStep::new("01", "Start", "init, project shape, targets").into(),
                            LifecycleStep::new("02", "Develop", "run, devices, logs, shells").into(),
                            LifecycleStep::new("03", "Debug", "tests, screenshots, inspectors").into(),
                            LifecycleStep::new("04", "Package", "artifacts, signing, preflight").into(),
                            LifecycleStep::new("05", "Release", "stores, hosts, rollouts, receipts").into(),
                        ],
                        gap: Some(tokens.spacing.s),
                        wrap: FlexWrap::Wrap,
                        justify_content: JustifyContent::SpaceBetween,
                        ..Default::default()
                    }
                    .into(),
                    Row {
                        children: vec![
                            Cta::new("Open lifecycle docs", "/docs/release-and-distribute/overview/", true).into(),
                            Cta::new("Read product page", "/product/production-lifecycle/", false).into(),
                        ],
                        gap: Some(tokens.spacing.s),
                        wrap: FlexWrap::Wrap,
                        ..Default::default()
                    }
                    .into(),
                ],
                gap: Some(tokens.spacing.l),
                ..Default::default()
            }
            .into(),
        )
        .into()
    }
}
#[derive(Clone, Debug)]
pub(super) struct ArchitectureSection;

impl From<ArchitectureSection> for Widget {
    fn from(_component: ArchitectureSection) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        ShellSection::new(
            Column {
                children: vec![
                    Row {
                        children: vec![
                            BoundaryPanel::new(
                                "Shared across every target",
                                "State, reducers, layout rules, semantics, rendering stages, and testable runtime behavior.",
                                &["State and reducers", "Layout rules", "Semantics tree", "Input routing", "Rendering stages", "Testable runtime behavior"],
                            )
                            .into(),
                            BoundaryPanel::new(
                                "Owned by each shell",
                                "Native windows, browser canvas, package shape, lifecycle hooks, and host-specific integration.",
                                &["Native windows", "Browser canvas", "Package shape", "Lifecycle hooks", "OS integration", "Capability brokering"],
                            )
                            .into(),
                        ],
                        gap: Some(tokens.spacing.l),
                        wrap: FlexWrap::Wrap,
                        align_items: AlignItems::Stretch,
                        ..Default::default()
                    }
                    .into(),
                    Row {
                        children: vec![
                            Text::new("Pipeline")
                                .size(tokens.typography.font_size_xs)
                                .weight(tokens.typography.font_weight_bold)
                                .color(tokens.colors.text_muted)
                                .into(),
                            Text::new("Build -> InternalLower -> Layout -> Paint -> Render")
                                .size(tokens.typography.font_size_sm)
                                .family(tokens.typography.font_family_mono.clone())
                                .color(tokens.colors.text_primary)
                                .into(),
                            Text::new("Same pipeline on every host.")
                                .size(tokens.typography.font_size_sm)
                                .color(tokens.colors.text_muted)
                                .into(),
                        ],
                        gap: Some(tokens.spacing.l),
                        wrap: FlexWrap::Wrap,
                        justify_content: JustifyContent::SpaceBetween,
                        ..Default::default()
                    }
                    .into(),
                ],
                gap: Some(tokens.spacing.l),
                ..Default::default()
            }
            .into(),
        )
        .into()
    }
}
#[derive(Clone, Debug)]
pub(super) struct ModelSection;

impl From<ModelSection> for Widget {
    fn from(_component: ModelSection) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        SemanticRow::new(
            "site-home-model",
            vec![
                Column {
                    children: vec![
                        Text::new("Why the model stays stable")
                            .size(tokens.typography.font_size_sm)
                            .weight(tokens.typography.font_weight_bold)
                            .color(tokens.colors.secondary)
                            .into(),
                        Text::new("The important boundaries stay visible.")
                            .size(tokens.typography.heading2_size)
                            .family(tokens.typography.font_family_serif.clone())
                            .line_height(
                                tokens.typography.heading2_size
                                    * tokens.typography.line_height_heading,
                            )
                            .weight(tokens.typography.font_weight_bold)
                            .color(tokens.colors.heading)
                            .into(),
                        Text::new("Fission is strict about where state changes happen, where host work starts, and how rendering is produced.")
                            .size(tokens.typography.body_large_size)
                            .line_height(tokens.typography.body_large_size * tokens.typography.line_height_relaxed)
                            .color(tokens.colors.text_secondary)
                            .into(),
                        ReducerCard.into(),
                        Row {
                            children: vec![
                                Cta::new("Read the model", "/docs/learn/runtime-model/", true)
                                    .into(),
                                Cta::new("Browse reference", "/reference/overview/overview/", false)
                                    .into(),
                            ],
                            gap: Some(tokens.spacing.s),
                            wrap: FlexWrap::Wrap,
                            ..Default::default()
                        }
                        .into(),
                    ],
                    gap: Some(tokens.spacing.l),
                    flex_grow: 1.0,
                    ..Default::default()
                }
                .into(),
                Row {
                    children: vec![
                        LinkCard::new("01", "Plain Rust data stays in charge.", "Product truth is not hidden inside widgets or host callbacks.", "State", "/docs/learn/runtime-model/").into(),
                        LinkCard::new("02", "Every durable change has a named cause.", "Typed actions and reducers keep behavior reviewable and testable.", "Reducers", "/docs/learn/runtime-model/").into(),
                        LinkCard::new("03", "Outside work has an explicit path.", "Files, timers, authentication, and services do not leak through rendering.", "Host work", "/docs/guides/resources-and-async/").into(),
                        LinkCard::new("04", "Layout and paint stay inspectable.", "Tests and diagnostics can inspect structure, semantics, and paint order directly.", "Render", "/docs/learn/rendering-pipeline/").into(),
                    ],
                    gap: Some(tokens.spacing.m),
                    wrap: FlexWrap::Wrap,
                    flex_grow: 1.0,
                    ..Default::default()
                }
                .into(),
            ],
            Some(tokens.spacing.xl),
            FlexWrap::Wrap,
            AlignItems::Stretch,
            JustifyContent::SpaceBetween,
        )
        .into()
    }
}
#[derive(Clone, Debug)]
pub(super) struct TargetsSection;

impl From<TargetsSection> for Widget {
    fn from(_component: TargetsSection) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        SemanticColumn::new(
            "site-home-targets",
            vec![
                SectionHeader::new(
                    "Targets",
                    "macOS, Windows, Linux, Web, Android, iOS, Terminal, Static site, and SSR are first-class targets.",
                    "Start on the host that answers your next product question fastest, then validate on every real target your users will touch.",
                )
                .into(),
                Column {
                    children: vec![
                        TargetRowCard::new("macOS", "First-class", "Desktop shell", "fission run --target macos", "Native windows, rendering, input, diagnostics, package readiness, and macOS release paths.", "/product/cross-platform-apps/", "macOS path ->").into(),
                        TargetRowCard::new("Windows", "First-class", "Desktop shell", "fission run --target windows", "Native windows, rendering, input, diagnostics, package readiness, and Windows release paths.", "/product/cross-platform-apps/", "Windows path ->").into(),
                        TargetRowCard::new("Linux", "First-class", "Desktop shell", "fission run --target linux", "Native windows, rendering, input, diagnostics, package readiness, and Linux package paths.", "/product/cross-platform-apps/", "Linux path ->").into(),
                        TargetRowCard::new("Web", "First-class", "Web shell", "fission run --target web", "Browser delivery with the same shared app model and WebAssembly packaging workflow.", "/product/cross-platform-apps/", "Web path ->").into(),
                        TargetRowCard::new("Android", "First-class", "Mobile shell", "fission run --target android", "Generated Android host, emulator/device workflow, APK/AAB readiness, and Play distribution.", "/product/cross-platform-apps/", "Android path ->").into(),
                        TargetRowCard::new("iOS", "First-class", "Mobile shell", "fission run --target ios", "Generated iOS host, simulator/device workflow, IPA readiness, and App Store distribution.", "/product/cross-platform-apps/", "iOS path ->").into(),
                        TargetRowCard::new("Terminal", "First-class", "Terminal shell", "fission ui", "Interactive terminal apps built from normal Fission widgets, reducers, screens, and routes.", "/product/terminal-apps/", "Terminal path ->").into(),
                        TargetRowCard::new("Static site", "First-class", "Site shell", "fission site build", "SEO-friendly Static site output from Fission widgets, Markdown content, search, metadata, and assets.", "/product/static-sites/", "Static site path ->").into(),
                        TargetRowCard::new("SSR", "First-class", "Server shell", "fission server serve", "Request-time Fission HTML with jobs, sessions, signed actions, cache policy, workers, and islands.", "/product/server-rendered-sites/", "SSR path ->").into(),
                    ],
                    gap: Some(tokens.spacing.s),
                    ..Default::default()
                }
                .into(),
            ],
            Some(tokens.spacing.xl),
            AlignItems::Center,
        )
        .into()
    }
}
#[derive(Clone, Debug)]
pub(super) struct ChartsSection;

impl From<ChartsSection> for Widget {
    fn from(_component: ChartsSection) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        SemanticColumn::new(
            "site-home-charts",
            vec![
                Row {
                    children: vec![
                        Column {
                            children: vec![
                                Text::new("Visual proof")
                                    .size(tokens.typography.font_size_sm)
                                    .weight(tokens.typography.font_weight_bold)
                                    .color(tokens.colors.secondary)
                                    .into(),
                                Text::new("A UI framework should be able to show, not merely tell.")
                                    .size(tokens.typography.heading2_size)
                                    .family(tokens.typography.font_family_serif.clone())
                                    .line_height(tokens.typography.heading2_size * tokens.typography.line_height_heading)
                                    .weight(tokens.typography.font_weight_bold)
                                    .color(tokens.colors.heading)
                                    .into(),
                            ],
                            gap: Some(tokens.spacing.m),
                            flex_grow: 1.0,
                            ..Default::default()
                        }
                        .into(),
                        Column {
                            children: vec![
                                Text::new("Fission Charts exercises the same renderer, layout system, themes, interaction model, and screenshot tooling as the rest of the framework—across analytical dashboards, live data, maps, networks, finance, and 3D-ready scenes.")
                                    .size(tokens.typography.body_large_size)
                                    .line_height(tokens.typography.body_large_size * tokens.typography.line_height_relaxed)
                                    .color(tokens.colors.text_secondary)
                                    .into(),
                                Row {
                                    children: vec![
                                        Cta::new("Explore charts", "/reference/charts/overview/", true).into(),
                                        Cta::new("View the catalogue", "/docs/charts/catalog/", false).into(),
                                    ],
                                    gap: Some(tokens.spacing.s),
                                    wrap: FlexWrap::Wrap,
                                    ..Default::default()
                                }
                                .into(),
                            ],
                            gap: Some(tokens.spacing.m),
                            flex_grow: 1.0,
                            ..Default::default()
                        }
                        .into(),
                    ],
                    gap: Some(tokens.spacing.xl),
                    wrap: FlexWrap::Wrap,
                    align_items: AlignItems::Start,
                    ..Default::default()
                }
                .into(),
                Row {
                    children: vec![
                        ChartImageCard::new("Gradient area line", "/img/charts/line-gradient-area.png").into(),
                        ChartImageCard::new("Quarter calendar heatmap", "/img/charts/calendar-user-activity.png").into(),
                        ChartImageCard::new("3D wave surface", "/img/charts/surface3d-wave.png").with_badge("3D / GL").into(),
                    ],
                    gap: Some(tokens.spacing.m),
                    wrap: FlexWrap::Wrap,
                    justify_content: JustifyContent::Center,
                    ..Default::default()
                }
                .into(),
            ],
            Some(tokens.spacing.xl),
            AlignItems::Stretch,
        )
        .into()
    }
}
#[derive(Clone, Debug)]
pub(super) struct ExamplesSection;

impl From<ExamplesSection> for Widget {
    fn from(_component: ExamplesSection) -> Self {
        let (_ctx, _view) = fission::build::current::<DocsState>();
        CenteredSection::new(
            "Examples",
            "Examples across the platform, not only the widget layer.",
            "Start with the smallest app, then inspect the examples that prove targets, charts, Static site, SSR, Terminal tooling, and release workflow.",
            vec![
                ExampleCard::new("Starter", "Counter", "cargo run -p counter", "The smallest complete Fission app loop: plain state, two reducers, a widget tree, and buttons bound with the public prelude macros.", "typed actions and reducers", "single-file starter app", "/docs/cookbook/build-a-counter/", "/reference/core/state-system/").into(),
                ExampleCard::new("Site", "Documentation", "fission site build --project-dir documentation", "This website is a Fission static site: custom homepage widgets, Markdown content routes, generated search, metadata, sidebars, and GitHub Pages output.", "Static site shell", "content routes and custom widgets", "/docs/guides/static-sites/", "/product/static-sites/").into(),
                ExampleCard::new("Server", "Pokemon card store", "fission server serve --project-dir examples/pokemon-card-store", "The server-rendered store demonstrates request-time routes, sessions, signed actions, server jobs, cache policy, generated workers, and focused islands.", "server shell", "dynamic Fission HTML", "/docs/guides/server-sites/", "/product/server-rendered-sites/").into(),
                ExampleCard::new("Terminal", "Fission command UI", "fission ui --project-dir .", "The CLI includes a terminal Fission app with screens, routes, reducers, dialogs, command sessions, logs, settings, density, and theme switching.", "terminal shell", "non-blocking command workflow", "/docs/guides/terminal-user-interfaces/", "/product/terminal-apps/").into(),
            ],
        )
        .into()
    }
}
#[derive(Clone, Debug)]
pub(super) struct FinalCta;

impl From<FinalCta> for Widget {
    fn from(_component: FinalCta) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        Container::new(
            Column {
                children: vec![
                    Pill::new("Next").into(),
                    Text::new("Start with one screen. Ship it everywhere.")
                        .size(tokens.typography.heading1_size)
                        .family(tokens.typography.font_family_serif.clone())
                        .line_height(
                            tokens.typography.heading1_size * tokens.typography.line_height_heading,
                        )
                        .weight(tokens.typography.font_weight_bold)
                        .color(tokens.colors.heading)
                        .text_align(TextAlign::Center)
                        .into(),
                    Text::new("Build your first Fission app, inspect the architecture, or explore the crates extending the framework.")
                        .size(tokens.typography.body_large_size)
                        .line_height(tokens.typography.body_large_size * tokens.typography.line_height_relaxed)
                        .color(tokens.colors.text_secondary)
                        .text_align(TextAlign::Center)
                        .into(),
                    Row {
                        children: vec![
                            Cta::new("Start building  →", "/docs/learn/quickstart/", true).into(),
                            Cta::new("Explore crates", "/crates/", false).into(),
                            NavLink::new("Read the architecture →", "/docs/learn/runtime-model/").into(),
                        ],
                        gap: Some(tokens.spacing.m),
                        wrap: FlexWrap::Wrap,
                        justify_content: JustifyContent::Center,
                        ..Default::default()
                    }
                    .into(),
                ],
                gap: Some(tokens.spacing.l),
                align_items: AlignItems::Center,
                semantics: Some(super::home_widgets::site_semantics("site-home-final-cta")),
                ..Default::default()
            }
        )
        .padding_all(tokens.spacing.xxxxl)
        .bg_fill(Fill::LinearGradient {
            start: (0.0, 0.0),
            end: (1.0, 1.0),
            stops: vec![
                (0.0, tokens.colors.surface_sunken),
                (1.0, tokens.colors.background),
            ],
        })
        .into()
    }
}

#[derive(Clone, Debug)]
struct BoundaryPanel {
    kicker: &'static str,
    title: &'static str,
    items: &'static [&'static str],
}

impl BoundaryPanel {
    fn new(kicker: &'static str, title: &'static str, items: &'static [&'static str]) -> Self {
        Self {
            kicker,
            title,
            items,
        }
    }
}

impl From<BoundaryPanel> for Widget {
    fn from(panel: BoundaryPanel) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        Container::new(Column {
            children: vec![
                Text::new(panel.kicker)
                    .size(tokens.typography.font_size_xs)
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.text_muted)
                    .into(),
                Text::new(panel.title)
                    .size(tokens.typography.heading_size)
                    .family(tokens.typography.font_family_serif.clone())
                    .line_height(
                        tokens.typography.heading_size * tokens.typography.line_height_heading,
                    )
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.heading)
                    .into(),
                Row {
                    children: panel
                        .items
                        .iter()
                        .map(|item| {
                            Text::new(*item)
                                .size(tokens.typography.font_size_sm)
                                .color(tokens.colors.text_secondary)
                                .into()
                        })
                        .collect(),
                    gap: Some(tokens.spacing.m),
                    wrap: FlexWrap::Wrap,
                    ..Default::default()
                }
                .into(),
            ],
            gap: Some(tokens.spacing.l),
            ..Default::default()
        })
        .padding_all(tokens.spacing.xl)
        .bg_fill(Fill::Solid(tokens.colors.surface))
        .border(tokens.colors.border, 1.0)
        .border_radius(tokens.radii.xxl)
        .flex_grow(1.0)
        .into()
    }
}

#[derive(Clone, Debug)]
struct LifecycleStep {
    number: &'static str,
    title: &'static str,
    body: &'static str,
}

impl LifecycleStep {
    fn new(number: &'static str, title: &'static str, body: &'static str) -> Self {
        Self {
            number,
            title,
            body,
        }
    }
}

impl From<LifecycleStep> for Widget {
    fn from(step: LifecycleStep) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        Container::new(Column {
            children: vec![
                Text::new(step.number)
                    .size(tokens.typography.font_size_xs)
                    .family(tokens.typography.font_family_mono.clone())
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.primary)
                    .into(),
                Text::new(step.title)
                    .size(tokens.typography.font_size_lg)
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.heading)
                    .into(),
                Text::new(step.body)
                    .size(tokens.typography.font_size_sm)
                    .line_height(
                        tokens.typography.font_size_sm * tokens.typography.line_height_normal,
                    )
                    .color(tokens.colors.text_secondary)
                    .into(),
            ],
            gap: Some(tokens.spacing.s),
            ..Default::default()
        })
        .padding_all(tokens.spacing.m)
        .bg_fill(Fill::Solid(tokens.colors.surface_raised))
        .border(tokens.colors.border, 1.0)
        .border_radius(tokens.radii.large)
        .width(tokens.spacing.xxxxl * 1.85)
        .flex_shrink(1.0)
        .into()
    }
}

#[derive(Clone, Debug)]
struct ReducerCard;

impl From<ReducerCard> for Widget {
    fn from(_card: ReducerCard) -> Self {
        MarkdownViewer {
            markdown: "```rust\nfn reduce(state: &mut GlobalState, action: Action) {\n    match action {\n        Action::Inc => state.count += 1,\n        Action::Reset => state.count = 0,\n    }\n}\n```"
                .to_string(),
            show_scrollbar: false,
        }
        .into()
    }
}
