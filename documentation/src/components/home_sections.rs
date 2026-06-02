use super::home_widgets::{
    hero_text_width, prose_width, semantic_column, semantic_row, CenteredSection, ChartImageCard,
    Chip, CodeCard, Cta, ExampleCard, LinkCard, NavLink, Pill, SectionHeader, ShellSection,
    StatusText, TargetRowCard,
};
use super::state::DocsState;
use fission::op::{AlignItems, Fill, FlexWrap, JustifyContent, TextAlign};
use fission::prelude::*;

#[derive(Clone, Debug)]
pub(super) struct HomePageHero;

impl From<HomePageHero> for Widget {
    fn from(_component: HomePageHero) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        semantic_column(
            "site-home-hero",
            vec![
                Pill::new("Rust application platform").into(),
                Text::new("Build, test, package, and release production apps in Rust.")
                    .size(tokens.typography.display_md_size)
                    .family(tokens.typography.font_family_serif.clone())
                    .line_height(
                        tokens.typography.display_md_size * tokens.typography.line_height_display,
                    )
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.heading)
                    .width(hero_text_width(tokens))
                    .max_width(hero_text_width(tokens))
                    .text_align(TextAlign::Center)
                    .semantics_identifier("site-home-hero-title")
                    .flex_shrink(1.0)
                    .into(),
                Text::new("Fission is a full application platform for desktop, mobile, web, terminal, static site, and server-rendered site targets, with one shared app model and lifecycle tooling around it.")
                    .size(tokens.typography.font_size_lg)
                    .line_height(tokens.typography.font_size_lg * tokens.typography.line_height_relaxed)
                    .color(tokens.colors.text_secondary)
                    .width(prose_width(tokens))
                    .max_width(prose_width(tokens))
                    .text_align(TextAlign::Center)
                    .flex_shrink(1.0)
                    .into(),
                Text::new("Write product state as plain Rust data, render with widgets, run through target shells, then use the CLI for devices, tests, preflight checks, packages, signing, release content, and distribution.")
                    .size(tokens.typography.body_large_size)
                    .line_height(tokens.typography.body_large_size * tokens.typography.line_height_relaxed)
                    .color(tokens.colors.text_muted)
                    .width(prose_width(tokens))
                    .max_width(prose_width(tokens))
                    .text_align(TextAlign::Center)
                    .flex_shrink(1.0)
                    .into(),
                Row {
                    children: vec![
                        Cta::new("Start building ->", "/docs/learn/quickstart/", true)
                            .into(),
                        Cta::new("Explore platform", "/product/overview/", false)
                            .into(),
                        NavLink::new("Release workflow ->", "/docs/release-and-distribute/overview/")
                            .into(),
                    ],
                    gap: Some(tokens.spacing.m),
                    wrap: FlexWrap::Wrap,
                    justify_content: JustifyContent::Center,
                    ..Default::default()
                }
                .into(),
                Row {
                    children: vec![
                        CodeCard::new("Create an app", "fission init my-app").into(),
                        CodeCard::new("Run on a target", "fission run --project-dir my-app")
                            .into(),
                        CodeCard::new("Check release readiness", "fission readiness release --target windows --format msix --provider microsoft-store").into(),
                    ],
                    gap: Some(tokens.spacing.m),
                    wrap: FlexWrap::Wrap,
                    justify_content: JustifyContent::Center,
                    ..Default::default()
                }
                .into(),
                Row {
                    children: vec![
                        StatusText::new("Desktop").into(),
                        StatusText::new("Web/WASM").into(),
                        StatusText::new("Android + iOS").into(),
                        StatusText::new("Terminal UI").into(),
                        StatusText::new("Static HTML").into(),
                        StatusText::new("Server HTML").into(),
                    ],
                    gap: Some(tokens.spacing.l),
                    wrap: FlexWrap::Wrap,
                    justify_content: JustifyContent::Center,
                    ..Default::default()
                }
                .into(),
            ],
            Some(tokens.spacing.l),
            AlignItems::Center,
        )
    }
}
#[derive(Clone, Debug)]
pub(super) struct ProofStrip;

impl From<ProofStrip> for Widget {
    fn from(_component: ProofStrip) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        semantic_column(
            "site-home-signals",
            vec![
                SectionHeader::new(
                    "What Fission is",
                    "One platform for the whole application lifecycle.",
                    "Fission combines a Rust UI runtime, target shells, developer workflow, package readiness, release content, and distribution tooling so teams do not have to invent a platform around the framework.",
                )
                .into(),
                Row {
                    children: vec![
                        LinkCard::new(
                            "Build",
                            "Shared product model",
                            "State, reducers, selectors, widgets, design systems, charts, commands, jobs, and services stay in Rust.",
                            "Learn the model ->",
                            "/docs/learn/overview/",
                        )
                        .into(),
                        LinkCard::new(
                            "Run",
                            "Real target shells",
                            "Desktop, web, mobile, terminal, static site, and server-rendered site shells host the same app model.",
                            "See targets ->",
                            "/product/cross-platform-apps/",
                        )
                        .into(),
                        LinkCard::new(
                            "Verify",
                            "Tests and diagnostics",
                            "Unit, widget, shell, screenshot, device, readiness, and future inspector tools are part of the platform story.",
                            "Debug path ->",
                            "/docs/test-and-debug/overview/",
                        )
                        .into(),
                        LinkCard::new(
                            "Ship",
                            "Post-build lifecycle",
                            "Package, sign, publish, manage testers, rollouts, tracks, static hosts, app stores, and release receipts.",
                            "Release path ->",
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
                            lifecycle_step(tokens, "01", "Start", "init, project shape, targets"),
                            lifecycle_step(tokens, "02", "Develop", "run, devices, logs, shells"),
                            lifecycle_step(tokens, "03", "Debug", "tests, screenshots, inspectors"),
                            lifecycle_step(tokens, "04", "Package", "artifacts, signing, preflight"),
                            lifecycle_step(tokens, "05", "Release", "stores, hosts, rollouts, receipts"),
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
        let (ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        ShellSection::new(
            Column {
                children: vec![
                    Row {
                        children: vec![
                            boundary_panel(
                                ctx,
                                view,
                                "Shared across every target",
                                "State, reducers, layout rules, semantics, rendering stages, and testable runtime behavior.",
                                &["State and reducers", "Layout rules", "Semantics tree", "Input routing", "Rendering stages", "Testable runtime behavior"],
                            ),
                            boundary_panel(
                                ctx,
                                view,
                                "Owned by each shell",
                                "Windows, browser surfaces, package shape, lifecycle hooks, and host-specific integration.",
                                &["Windows and surfaces", "Browser canvas", "Package shape", "Lifecycle hooks", "OS integration", "Capability brokering"],
                            ),
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
        semantic_row(
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
                        reducer_card(tokens),
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
    }
}
#[derive(Clone, Debug)]
pub(super) struct TargetsSection;

impl From<TargetsSection> for Widget {
    fn from(_component: TargetsSection) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        semantic_column(
            "site-home-targets",
            vec![
                SectionHeader::new(
                    "Targets",
                    "Desktop, mobile, web, terminal, static HTML, and server-rendered HTML are first-class outputs.",
                    "Start on the host that answers your next product question fastest, then validate on every real target your users will touch.",
                )
                .into(),
                Column {
                    children: vec![
                        TargetRowCard::new("Desktop", "First-class", "macOS - Linux - Windows", "fission run --target desktop", "Native windows, rendering, input, diagnostics, package readiness, and desktop release paths.", "/product/cross-platform-apps/", "Desktop path ->").into(),
                        TargetRowCard::new("Web", "First-class", "WASM", "fission run --target web", "Browser delivery with the same shared app model and web/static packaging workflow.", "/product/cross-platform-apps/", "Web path ->").into(),
                        TargetRowCard::new("Mobile", "First-class", "Android - iOS", "fission devices", "Generated mobile hosts, emulator/simulator workflow, APK/AAB/IPA readiness, and store publishing.", "/product/cross-platform-apps/", "Mobile path ->").into(),
                        TargetRowCard::new("Terminal UI", "First-class", "Windows - macOS - Linux", "fission ui", "Interactive terminal apps built from normal Fission widgets, reducers, screens, and routes.", "/product/terminal-apps/", "Terminal path ->").into(),
                        TargetRowCard::new("Static HTML", "First-class", "Sites - Docs - Marketing", "fission site build", "SEO-friendly static HTML from Fission widgets, Markdown content, search, metadata, and assets.", "/product/static-sites/", "Site path ->").into(),
                        TargetRowCard::new("Server HTML", "First-class", "Dynamic sites", "fission server serve", "Request-time Fission HTML with jobs, sessions, signed actions, cache policy, workers, and islands.", "/product/server-rendered-sites/", "Server path ->").into(),
                    ],
                    gap: Some(tokens.spacing.s),
                    ..Default::default()
                }
                .into(),
            ],
            Some(tokens.spacing.xl),
            AlignItems::Center,
        )
    }
}
#[derive(Clone, Debug)]
pub(super) struct ChartsSection;

impl From<ChartsSection> for Widget {
    fn from(_component: ChartsSection) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        semantic_column(
            "site-home-charts",
            vec![
                Row {
                    children: vec![
                        Column {
                            children: vec![
                                Text::new("Beautiful charts")
                                    .size(tokens.typography.font_size_sm)
                                    .weight(tokens.typography.font_weight_bold)
                                    .color(tokens.colors.secondary)
                                    .into(),
                                Text::new("Dashboards, analytics, finance, maps, networks, and 3D-ready visuals.")
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
                                Text::new("Fission Charts is the native charting layer for Fission apps, with more than 400 renderer-backed variants covering line, bar, area, pie, scatter, heatmap, financial, relationship, map, component, dynamic, and 3D chart work - without leaving the Rust UI model.")
                                    .size(tokens.typography.body_large_size)
                                    .line_height(tokens.typography.body_large_size * tokens.typography.line_height_relaxed)
                                    .color(tokens.colors.text_secondary)
                                    .into(),
                                Row {
                                    children: vec![
                                        Cta::new("Explore Charts", "/reference/charts/overview/", true).into(),
                                        Cta::new("Open catalog", "/docs/charts/catalog/", false).into(),
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
                        ChartImageCard::new("Ranked bar", "/img/charts/bar-horizontal.png").into(),
                        ChartImageCard::new("Quarter calendar heatmap", "/img/charts/calendar-user-activity.png").into(),
                        ChartImageCard::new("Energy sankey", "/img/charts/sankey-energy.png").into(),
                        ChartImageCard::new("3D wave surface", "/img/charts/surface3d-wave.png").with_badge("3D / GL").into(),
                    ],
                    gap: Some(tokens.spacing.m),
                    wrap: FlexWrap::Wrap,
                    justify_content: JustifyContent::Center,
                    ..Default::default()
                }
                .into(),
                Row {
                    children: [
                        "Line", "Bar", "Area", "Pie", "Scatter", "Heatmap", "Financial",
                        "Relationship", "Map", "Component", "Dynamic", "3D",
                    ]
                    .iter()
                    .map(|label| Chip::new(label).into())
                    .collect(),
                    gap: Some(tokens.spacing.s),
                    wrap: FlexWrap::Wrap,
                    justify_content: JustifyContent::Center,
                    ..Default::default()
                }
                .into(),
            ],
            Some(tokens.spacing.xl),
            AlignItems::Stretch,
        )
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
            "Start with the smallest app, then inspect the examples that prove targets, charts, static sites, server-rendered sites, terminal tooling, and release workflow.",
            vec![
                ExampleCard::new("Starter", "Counter", "cargo run -p counter", "The smallest complete Fission app loop: plain state, two reducers, a widget tree, and buttons bound with the public prelude macros.", "typed actions and reducers", "single-file starter app", "/docs/cookbook/build-a-counter/", "/reference/core/state-system/").into(),
                ExampleCard::new("Site", "Documentation", "fission site build --project-dir documentation", "This website is a Fission static site: custom homepage widgets, Markdown content routes, generated search, metadata, sidebars, and GitHub Pages output.", "static HTML shell", "content routes and custom widgets", "/docs/guides/static-sites/", "/product/static-sites/").into(),
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
                    Text::new("Pick a lifecycle stage and keep moving.")
                        .size(tokens.typography.heading1_size)
                        .family(tokens.typography.font_family_serif.clone())
                        .line_height(
                            tokens.typography.heading1_size * tokens.typography.line_height_heading,
                        )
                        .weight(tokens.typography.font_weight_bold)
                        .color(tokens.colors.heading)
                        .text_align(TextAlign::Center)
                        .into(),
                    Text::new("Start with the app model, add the targets you need, then use Fission's tooling to verify, package, and release the product.")
                        .size(tokens.typography.body_large_size)
                        .line_height(tokens.typography.body_large_size * tokens.typography.line_height_relaxed)
                        .color(tokens.colors.text_secondary)
                        .text_align(TextAlign::Center)
                        .into(),
                    Row {
                        children: vec![
                            Cta::new("Start docs", "/docs/intro/", true).into(),
                            Cta::new("Product overview", "/product/overview/", false).into(),
                            NavLink::new("Reference ->", "/reference/overview/overview/").into(),
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
fn boundary_panel(
    _ctx: BuildCtxHandle<DocsState>,
    view: ViewHandle<DocsState>,
    kicker: &'static str,
    title: &'static str,
    items: &[&'static str],
) -> Widget {
    let tokens = &view.env().theme.tokens;
    Container::new(Column {
        children: vec![
            Text::new(kicker)
                .size(tokens.typography.font_size_xs)
                .weight(tokens.typography.font_weight_bold)
                .color(tokens.colors.text_muted)
                .into(),
            Text::new(title)
                .size(tokens.typography.heading_size)
                .family(tokens.typography.font_family_serif.clone())
                .line_height(tokens.typography.heading_size * tokens.typography.line_height_heading)
                .weight(tokens.typography.font_weight_bold)
                .color(tokens.colors.heading)
                .into(),
            Row {
                children: items
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

fn lifecycle_step(
    tokens: &Tokens,
    number: &'static str,
    title: &'static str,
    body: &'static str,
) -> Widget {
    Container::new(Column {
        children: vec![
            Text::new(number)
                .size(tokens.typography.font_size_xs)
                .family(tokens.typography.font_family_mono.clone())
                .weight(tokens.typography.font_weight_bold)
                .color(tokens.colors.primary)
                .into(),
            Text::new(title)
                .size(tokens.typography.font_size_lg)
                .weight(tokens.typography.font_weight_bold)
                .color(tokens.colors.heading)
                .into(),
            Text::new(body)
                .size(tokens.typography.font_size_sm)
                .line_height(tokens.typography.font_size_sm * tokens.typography.line_height_normal)
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

fn reducer_card(tokens: &Tokens) -> Widget {
    Container::new(
        Text::new("fn reduce(state: &mut GlobalState, action: Action) {\n  match action {\n    Action::Inc => state.count += 1,\n    Action::Reset => state.count = 0,\n  }\n}")
            .size(tokens.typography.font_size_sm)
            .family(tokens.typography.font_family_mono.clone())
            .line_height(tokens.typography.font_size_sm * tokens.typography.line_height_relaxed)
            .color(tokens.colors.text_primary)
    )
    .padding_all(tokens.spacing.l)
    .bg_fill(Fill::Solid(tokens.colors.surface_raised))
    .border(tokens.colors.border, 1.0)
    .border_radius(tokens.radii.xl)
    .into()
}
