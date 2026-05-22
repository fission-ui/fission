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

impl Widget<DocsState> for HomePageHero {
    fn build(&self, ctx: &mut BuildCtx<DocsState>, view: &View<DocsState>) -> Node {
        let tokens = &view.env.theme.tokens;
        semantic_column(
            "site-home-hero",
            vec![
                Pill::new("Rust application platform").build(ctx, view),
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
                    .into_node(),
                Text::new("Fission is a full application platform for desktop, mobile, web, terminal, and static site targets, with one shared app model and lifecycle tooling around it.")
                    .size(tokens.typography.font_size_lg)
                    .line_height(tokens.typography.font_size_lg * tokens.typography.line_height_relaxed)
                    .color(tokens.colors.text_secondary)
                    .width(prose_width(tokens))
                    .max_width(prose_width(tokens))
                    .text_align(TextAlign::Center)
                    .flex_shrink(1.0)
                    .into_node(),
                Text::new("Write product state as plain Rust data, render with widgets, run through target shells, then use the CLI for devices, tests, preflight checks, packages, signing, release content, and distribution.")
                    .size(tokens.typography.body_large_size)
                    .line_height(tokens.typography.body_large_size * tokens.typography.line_height_relaxed)
                    .color(tokens.colors.text_muted)
                    .width(prose_width(tokens))
                    .max_width(prose_width(tokens))
                    .text_align(TextAlign::Center)
                    .flex_shrink(1.0)
                    .into_node(),
                Row {
                    children: vec![
                        Cta::new("Start building ->", "/docs/learn/quickstart/", true)
                            .build(ctx, view),
                        Cta::new("Explore platform", "/product/overview/", false)
                            .build(ctx, view),
                        NavLink::new("Release workflow ->", "/docs/release-and-distribute/overview/")
                            .build(ctx, view),
                    ],
                    gap: Some(tokens.spacing.m),
                    wrap: FlexWrap::Wrap,
                    justify_content: JustifyContent::Center,
                    ..Default::default()
                }
                .into_node(),
                Row {
                    children: vec![
                        CodeCard::new("Create an app", "fission init my-app").build(ctx, view),
                        CodeCard::new("Run on a target", "fission run --project-dir my-app")
                            .build(ctx, view),
                        CodeCard::new("Check release readiness", "fission readiness release --target windows --format msix --provider microsoft-store").build(ctx, view),
                    ],
                    gap: Some(tokens.spacing.m),
                    wrap: FlexWrap::Wrap,
                    justify_content: JustifyContent::Center,
                    ..Default::default()
                }
                .into_node(),
                Row {
                    children: vec![
                        StatusText::new("Desktop").build(ctx, view),
                        StatusText::new("Web/WASM").build(ctx, view),
                        StatusText::new("Android + iOS").build(ctx, view),
                        StatusText::new("Terminal UI").build(ctx, view),
                        StatusText::new("Static HTML").build(ctx, view),
                    ],
                    gap: Some(tokens.spacing.l),
                    wrap: FlexWrap::Wrap,
                    justify_content: JustifyContent::Center,
                    ..Default::default()
                }
                .into_node(),
            ],
            Some(tokens.spacing.l),
            AlignItems::Center,
        )
    }
}

#[derive(Clone, Debug)]
pub(super) struct ProofStrip;

impl Widget<DocsState> for ProofStrip {
    fn build(&self, ctx: &mut BuildCtx<DocsState>, view: &View<DocsState>) -> Node {
        let tokens = &view.env.theme.tokens;
        semantic_column(
            "site-home-signals",
            vec![
                SectionHeader::new(
                    "What Fission is",
                    "One platform for the whole application lifecycle.",
                    "Fission combines a Rust UI runtime, target shells, developer workflow, package readiness, release content, and distribution tooling so teams do not have to invent a platform around the framework.",
                )
                .build(ctx, view),
                Row {
                    children: vec![
                        LinkCard::new(
                            "Build",
                            "Shared product model",
                            "State, reducers, selectors, widgets, design systems, charts, commands, jobs, and services stay in Rust.",
                            "Learn the model ->",
                            "/docs/learn/overview/",
                        )
                        .build(ctx, view),
                        LinkCard::new(
                            "Run",
                            "Real target shells",
                            "Desktop, web, mobile, terminal, and static site shells host the same app model.",
                            "See targets ->",
                            "/product/cross-platform-apps/",
                        )
                        .build(ctx, view),
                        LinkCard::new(
                            "Verify",
                            "Tests and diagnostics",
                            "Unit, widget, shell, screenshot, device, readiness, and future inspector tools are part of the platform story.",
                            "Debug path ->",
                            "/docs/test-and-debug/overview/",
                        )
                        .build(ctx, view),
                        LinkCard::new(
                            "Ship",
                            "Post-build lifecycle",
                            "Package, sign, publish, manage testers, rollouts, tracks, static hosts, app stores, and release receipts.",
                            "Release path ->",
                            "/product/production-lifecycle/",
                        )
                        .build(ctx, view),
                    ],
                    gap: Some(tokens.spacing.m),
                    wrap: FlexWrap::Wrap,
                    justify_content: JustifyContent::Center,
                    ..Default::default()
                }
                .into_node(),
            ],
            Some(tokens.spacing.xl),
            AlignItems::Center,
        )
    }
}

#[derive(Clone, Debug)]
pub(super) struct LifecycleSection;

impl Widget<DocsState> for LifecycleSection {
    fn build(&self, ctx: &mut BuildCtx<DocsState>, view: &View<DocsState>) -> Node {
        let tokens = &view.env.theme.tokens;
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
                                        .into_node(),
                                    Text::new("From first run to store rollout.")
                                        .size(tokens.typography.heading2_size)
                                        .family(tokens.typography.font_family_serif.clone())
                                        .line_height(tokens.typography.heading2_size * tokens.typography.line_height_heading)
                                        .weight(tokens.typography.font_weight_bold)
                                        .color(tokens.colors.heading)
                                        .into_node(),
                                ],
                                gap: Some(tokens.spacing.m),
                                flex_grow: 1.0,
                                ..Default::default()
                            }
                            .into_node(),
                            Text::new("The docs now follow the path teams actually take: setup, develop, test, debug, package, sign, release, distribute, and keep receipts for automation.")
                                .size(tokens.typography.body_large_size)
                                .line_height(tokens.typography.body_large_size * tokens.typography.line_height_relaxed)
                                .color(tokens.colors.text_secondary)
                                .flex_grow(1.0)
                                .into_node(),
                        ],
                        gap: Some(tokens.spacing.xl),
                        wrap: FlexWrap::Wrap,
                        align_items: AlignItems::Start,
                        ..Default::default()
                    }
                    .into_node(),
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
                    .into_node(),
                    Row {
                        children: vec![
                            Cta::new("Open lifecycle docs", "/docs/release-and-distribute/overview/", true).build(ctx, view),
                            Cta::new("Read product page", "/product/production-lifecycle/", false).build(ctx, view),
                        ],
                        gap: Some(tokens.spacing.s),
                        wrap: FlexWrap::Wrap,
                        ..Default::default()
                    }
                    .into_node(),
                ],
                gap: Some(tokens.spacing.l),
                ..Default::default()
            }
            .into_node(),
        )
        .build(ctx, view)
    }
}

#[derive(Clone, Debug)]
pub(super) struct ArchitectureSection;

impl Widget<DocsState> for ArchitectureSection {
    fn build(&self, ctx: &mut BuildCtx<DocsState>, view: &View<DocsState>) -> Node {
        let tokens = &view.env.theme.tokens;
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
                    .into_node(),
                    Row {
                        children: vec![
                            Text::new("Pipeline")
                                .size(tokens.typography.font_size_xs)
                                .weight(tokens.typography.font_weight_bold)
                                .color(tokens.colors.text_muted)
                                .into_node(),
                            Text::new("Build -> Lower -> Layout -> Paint -> Render")
                                .size(tokens.typography.font_size_sm)
                                .family(tokens.typography.font_family_mono.clone())
                                .color(tokens.colors.text_primary)
                                .into_node(),
                            Text::new("Same pipeline on every host.")
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
                ],
                gap: Some(tokens.spacing.l),
                ..Default::default()
            }
            .into_node(),
        )
        .build(ctx, view)
    }
}

#[derive(Clone, Debug)]
pub(super) struct ModelSection;

impl Widget<DocsState> for ModelSection {
    fn build(&self, ctx: &mut BuildCtx<DocsState>, view: &View<DocsState>) -> Node {
        let tokens = &view.env.theme.tokens;
        semantic_row(
            "site-home-model",
            vec![
                Column {
                    children: vec![
                        Text::new("Why the model stays stable")
                            .size(tokens.typography.font_size_sm)
                            .weight(tokens.typography.font_weight_bold)
                            .color(tokens.colors.secondary)
                            .into_node(),
                        Text::new("The important boundaries stay visible.")
                            .size(tokens.typography.heading2_size)
                            .family(tokens.typography.font_family_serif.clone())
                            .line_height(
                                tokens.typography.heading2_size
                                    * tokens.typography.line_height_heading,
                            )
                            .weight(tokens.typography.font_weight_bold)
                            .color(tokens.colors.heading)
                            .into_node(),
                        Text::new("Fission is strict about where state changes happen, where host work starts, and how rendering is produced.")
                            .size(tokens.typography.body_large_size)
                            .line_height(tokens.typography.body_large_size * tokens.typography.line_height_relaxed)
                            .color(tokens.colors.text_secondary)
                            .into_node(),
                        reducer_card(tokens),
                        Row {
                            children: vec![
                                Cta::new("Read the model", "/docs/learn/runtime-model/", true)
                                    .build(ctx, view),
                                Cta::new("Browse reference", "/reference/overview/overview/", false)
                                    .build(ctx, view),
                            ],
                            gap: Some(tokens.spacing.s),
                            wrap: FlexWrap::Wrap,
                            ..Default::default()
                        }
                        .into_node(),
                    ],
                    gap: Some(tokens.spacing.l),
                    flex_grow: 1.0,
                    ..Default::default()
                }
                .into_node(),
                Row {
                    children: vec![
                        LinkCard::new("01", "Plain Rust data stays in charge.", "Product truth is not hidden inside widgets or host callbacks.", "State", "/docs/learn/runtime-model/").build(ctx, view),
                        LinkCard::new("02", "Every durable change has a named cause.", "Typed actions and reducers keep behavior reviewable and testable.", "Reducers", "/docs/learn/runtime-model/").build(ctx, view),
                        LinkCard::new("03", "Outside work has an explicit path.", "Files, timers, authentication, and services do not leak through rendering.", "Host work", "/docs/guides/resources-and-async/").build(ctx, view),
                        LinkCard::new("04", "Layout and paint stay inspectable.", "Tests and diagnostics can inspect structure, semantics, and paint order directly.", "Render", "/docs/learn/rendering-pipeline/").build(ctx, view),
                    ],
                    gap: Some(tokens.spacing.m),
                    wrap: FlexWrap::Wrap,
                    flex_grow: 1.0,
                    ..Default::default()
                }
                .into_node(),
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

impl Widget<DocsState> for TargetsSection {
    fn build(&self, ctx: &mut BuildCtx<DocsState>, view: &View<DocsState>) -> Node {
        let tokens = &view.env.theme.tokens;
        semantic_column(
            "site-home-targets",
            vec![
                SectionHeader::new(
                    "Targets",
                    "Desktop, mobile, web, terminal, and static HTML are first-class outputs.",
                    "Start on the host that answers your next product question fastest, then validate on every real target your users will touch.",
                )
                .build(ctx, view),
                Column {
                    children: vec![
                        TargetRowCard::new("Desktop", "First-class", "macOS - Linux - Windows", "fission run --target desktop", "Native windows, rendering, input, diagnostics, package readiness, and desktop release paths.", "/product/cross-platform-apps/", "Desktop path ->").build(ctx, view),
                        TargetRowCard::new("Web", "First-class", "WASM", "fission run --target web", "Browser delivery with the same shared app model and web/static packaging workflow.", "/product/cross-platform-apps/", "Web path ->").build(ctx, view),
                        TargetRowCard::new("Mobile", "First-class", "Android - iOS", "fission devices", "Generated mobile hosts, emulator/simulator workflow, APK/AAB/IPA readiness, and store publishing.", "/product/cross-platform-apps/", "Mobile path ->").build(ctx, view),
                        TargetRowCard::new("Terminal UI", "First-class", "Windows - macOS - Linux", "fission ui", "Interactive terminal apps built from normal Fission widgets, reducers, screens, and routes.", "/product/terminal-apps/", "Terminal path ->").build(ctx, view),
                        TargetRowCard::new("Static HTML", "First-class", "Sites - Docs - Marketing", "fission site build", "SEO-friendly static HTML from Fission widgets, Markdown content, search, metadata, and assets.", "/product/static-sites/", "Site path ->").build(ctx, view),
                    ],
                    gap: Some(tokens.spacing.s),
                    ..Default::default()
                }
                .into_node(),
            ],
            Some(tokens.spacing.xl),
            AlignItems::Center,
        )
    }
}

#[derive(Clone, Debug)]
pub(super) struct ChartsSection;

impl Widget<DocsState> for ChartsSection {
    fn build(&self, ctx: &mut BuildCtx<DocsState>, view: &View<DocsState>) -> Node {
        let tokens = &view.env.theme.tokens;
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
                                    .into_node(),
                                Text::new("Dashboards, analytics, finance, maps, networks, and 3D-ready visuals.")
                                    .size(tokens.typography.heading2_size)
                                    .family(tokens.typography.font_family_serif.clone())
                                    .line_height(tokens.typography.heading2_size * tokens.typography.line_height_heading)
                                    .weight(tokens.typography.font_weight_bold)
                                    .color(tokens.colors.heading)
                                    .into_node(),
                            ],
                            gap: Some(tokens.spacing.m),
                            flex_grow: 1.0,
                            ..Default::default()
                        }
                        .into_node(),
                        Column {
                            children: vec![
                                Text::new("Fission Charts is the native charting layer for Fission apps, with more than 400 renderer-backed variants covering line, bar, area, pie, scatter, heatmap, financial, relationship, map, component, dynamic, and 3D chart work - without leaving the Rust UI model.")
                                    .size(tokens.typography.body_large_size)
                                    .line_height(tokens.typography.body_large_size * tokens.typography.line_height_relaxed)
                                    .color(tokens.colors.text_secondary)
                                    .into_node(),
                                Row {
                                    children: vec![
                                        Cta::new("Explore Charts", "/reference/charts/overview/", true).build(ctx, view),
                                        Cta::new("Open catalog", "/docs/charts/catalog/", false).build(ctx, view),
                                    ],
                                    gap: Some(tokens.spacing.s),
                                    wrap: FlexWrap::Wrap,
                                    ..Default::default()
                                }
                                .into_node(),
                            ],
                            gap: Some(tokens.spacing.m),
                            flex_grow: 1.0,
                            ..Default::default()
                        }
                        .into_node(),
                    ],
                    gap: Some(tokens.spacing.xl),
                    wrap: FlexWrap::Wrap,
                    align_items: AlignItems::Start,
                    ..Default::default()
                }
                .into_node(),
                Row {
                    children: vec![
                        ChartImageCard::new("Gradient area line", "/img/charts/line-gradient-area.png").build(ctx, view),
                        ChartImageCard::new("Ranked bar", "/img/charts/bar-horizontal.png").build(ctx, view),
                        ChartImageCard::new("Quarter calendar heatmap", "/img/charts/calendar-user-activity.png").build(ctx, view),
                        ChartImageCard::new("Energy sankey", "/img/charts/sankey-energy.png").build(ctx, view),
                        ChartImageCard::new("3D wave surface", "/img/charts/surface3d-wave.png").with_badge("3D / GL").build(ctx, view),
                    ],
                    gap: Some(tokens.spacing.m),
                    wrap: FlexWrap::Wrap,
                    justify_content: JustifyContent::Center,
                    ..Default::default()
                }
                .into_node(),
                Row {
                    children: [
                        "Line", "Bar", "Area", "Pie", "Scatter", "Heatmap", "Financial",
                        "Relationship", "Map", "Component", "Dynamic", "3D",
                    ]
                    .iter()
                    .map(|label| Chip::new(label).build(ctx, view))
                    .collect(),
                    gap: Some(tokens.spacing.s),
                    wrap: FlexWrap::Wrap,
                    justify_content: JustifyContent::Center,
                    ..Default::default()
                }
                .into_node(),
            ],
            Some(tokens.spacing.xl),
            AlignItems::Stretch,
        )
    }
}

#[derive(Clone, Debug)]
pub(super) struct ExamplesSection;

impl Widget<DocsState> for ExamplesSection {
    fn build(&self, ctx: &mut BuildCtx<DocsState>, view: &View<DocsState>) -> Node {
        CenteredSection::new(
            "Examples",
            "Examples across the platform, not only the widget layer.",
            "Start with the smallest app, then inspect the examples that prove targets, charts, static sites, terminal tooling, and release workflow.",
            vec![
                ExampleCard::new("Starter", "Counter", "cargo run -p counter", "The smallest complete Fission app loop: plain state, two reducers, a widget tree, and buttons bound with the public prelude macros.", "typed actions and reducers", "single-file starter app", "/docs/cookbook/build-a-counter/", "/reference/core/state-system/").build(ctx, view),
                ExampleCard::new("Site", "Documentation", "fission site build --project-dir documentation", "This website is a Fission static site: custom homepage widgets, Markdown content routes, generated search, metadata, sidebars, and GitHub Pages output.", "static HTML shell", "content routes and custom widgets", "/docs/guides/static-sites/", "/product/static-sites/").build(ctx, view),
                ExampleCard::new("Terminal", "Fission CLI UI", "fission ui --project-dir .", "The CLI includes a terminal Fission app with screens, routes, reducers, dialogs, command sessions, logs, settings, density, and theme switching.", "terminal shell", "non-blocking command workflow", "/docs/guides/terminal-user-interfaces/", "/product/terminal-apps/").build(ctx, view),
            ],
        )
        .build(ctx, view)
    }
}

#[derive(Clone, Debug)]
pub(super) struct FinalCta;

impl Widget<DocsState> for FinalCta {
    fn build(&self, ctx: &mut BuildCtx<DocsState>, view: &View<DocsState>) -> Node {
        let tokens = &view.env.theme.tokens;
        Container::new(
            Column {
                children: vec![
                    Pill::new("Next").build(ctx, view),
                    Text::new("Pick a lifecycle stage and keep moving.")
                        .size(tokens.typography.heading1_size)
                        .family(tokens.typography.font_family_serif.clone())
                        .line_height(
                            tokens.typography.heading1_size * tokens.typography.line_height_heading,
                        )
                        .weight(tokens.typography.font_weight_bold)
                        .color(tokens.colors.heading)
                        .text_align(TextAlign::Center)
                        .into_node(),
                    Text::new("Start with the app model, add the targets you need, then use Fission's tooling to verify, package, and release the product.")
                        .size(tokens.typography.body_large_size)
                        .line_height(tokens.typography.body_large_size * tokens.typography.line_height_relaxed)
                        .color(tokens.colors.text_secondary)
                        .text_align(TextAlign::Center)
                        .into_node(),
                    Row {
                        children: vec![
                            Cta::new("Start docs", "/docs/intro/", true).build(ctx, view),
                            Cta::new("Product overview", "/product/overview/", false).build(ctx, view),
                            NavLink::new("Reference ->", "/reference/overview/overview/").build(ctx, view),
                        ],
                        gap: Some(tokens.spacing.m),
                        wrap: FlexWrap::Wrap,
                        justify_content: JustifyContent::Center,
                        ..Default::default()
                    }
                    .into_node(),
                ],
                gap: Some(tokens.spacing.l),
                align_items: AlignItems::Center,
                ..Default::default()
            }
            .into_node(),
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
        .into_node()
    }
}

fn boundary_panel(
    _ctx: &mut BuildCtx<DocsState>,
    view: &View<DocsState>,
    kicker: &'static str,
    title: &'static str,
    items: &[&'static str],
) -> Node {
    let tokens = &view.env.theme.tokens;
    Container::new(
        Column {
            children: vec![
                Text::new(kicker)
                    .size(tokens.typography.font_size_xs)
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.text_muted)
                    .into_node(),
                Text::new(title)
                    .size(tokens.typography.heading_size)
                    .family(tokens.typography.font_family_serif.clone())
                    .line_height(
                        tokens.typography.heading_size * tokens.typography.line_height_heading,
                    )
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.heading)
                    .into_node(),
                Row {
                    children: items
                        .iter()
                        .map(|item| {
                            Text::new(*item)
                                .size(tokens.typography.font_size_sm)
                                .color(tokens.colors.text_secondary)
                                .into_node()
                        })
                        .collect(),
                    gap: Some(tokens.spacing.m),
                    wrap: FlexWrap::Wrap,
                    ..Default::default()
                }
                .into_node(),
            ],
            gap: Some(tokens.spacing.l),
            ..Default::default()
        }
        .into_node(),
    )
    .padding_all(tokens.spacing.xl)
    .bg_fill(Fill::Solid(tokens.colors.surface))
    .border(tokens.colors.border, 1.0)
    .border_radius(tokens.radii.xxl)
    .flex_grow(1.0)
    .into_node()
}

fn lifecycle_step(
    tokens: &Tokens,
    number: &'static str,
    title: &'static str,
    body: &'static str,
) -> Node {
    Container::new(
        Column {
            children: vec![
                Text::new(number)
                    .size(tokens.typography.font_size_xs)
                    .family(tokens.typography.font_family_mono.clone())
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.primary)
                    .into_node(),
                Text::new(title)
                    .size(tokens.typography.font_size_lg)
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.heading)
                    .into_node(),
                Text::new(body)
                    .size(tokens.typography.font_size_sm)
                    .line_height(
                        tokens.typography.font_size_sm * tokens.typography.line_height_normal,
                    )
                    .color(tokens.colors.text_secondary)
                    .into_node(),
            ],
            gap: Some(tokens.spacing.s),
            ..Default::default()
        }
        .into_node(),
    )
    .padding_all(tokens.spacing.m)
    .bg_fill(Fill::Solid(tokens.colors.surface_raised))
    .border(tokens.colors.border, 1.0)
    .border_radius(tokens.radii.large)
    .width(tokens.spacing.xxxxl * 1.85)
    .flex_shrink(1.0)
    .into_node()
}

fn reducer_card(tokens: &Tokens) -> Node {
    Container::new(
        Text::new("fn reduce(state: &mut AppState, action: Action) {\n  match action {\n    Action::Inc => state.count += 1,\n    Action::Reset => state.count = 0,\n  }\n}")
            .size(tokens.typography.font_size_sm)
            .family(tokens.typography.font_family_mono.clone())
            .line_height(tokens.typography.font_size_sm * tokens.typography.line_height_relaxed)
            .color(tokens.colors.text_primary)
            .into_node(),
    )
    .padding_all(tokens.spacing.l)
    .bg_fill(Fill::Solid(tokens.colors.surface_raised))
    .border(tokens.colors.border, 1.0)
    .border_radius(tokens.radii.xl)
    .into_node()
}
