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
                Pill::new("Production-ready Rust user interface").build(ctx, view),
                Text::new("Build desktop, web, Android, and iOS apps in Rust.")
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
                Text::new("Fission is a cross-platform user interface framework with one shared runtime, explicit state, explicit side effects, and a GPU-backed rendering pipeline.")
                    .size(tokens.typography.font_size_lg)
                    .line_height(tokens.typography.font_size_lg * tokens.typography.line_height_relaxed)
                    .color(tokens.colors.text_secondary)
                    .width(prose_width(tokens))
                    .max_width(prose_width(tokens))
                    .text_align(TextAlign::Center)
                    .flex_shrink(1.0)
                    .into_node(),
                Text::new("You write app state as plain Rust data, update it with reducers, and let Fission keep layout, input, time, rendering, and platform boundaries consistent across every target.")
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
                        Cta::new("Start with Quickstart ->", "/docs/learn/quickstart/", true)
                            .build(ctx, view),
                        Cta::new("Read Learn overview", "/docs/learn/overview/", false)
                            .build(ctx, view),
                        NavLink::new("Browse Reference ->", "/reference/overview/overview/")
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
                        CodeCard::new("Run a real app", "cargo run -p counter").build(ctx, view),
                        CodeCard::new("Create your own project", "fission init my-app")
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
                        StatusText::new("Rust 1.77+").build(ctx, view),
                        StatusText::new("MIT licensed").build(ctx, view),
                        StatusText::new("v0.1.0 alpha").build(ctx, view),
                        StatusText::new("Renders on Vello + wgpu").build(ctx, view),
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
                    "A cross-platform Rust framework built for real products.",
                    "Fission keeps state flow, layout, semantics, input routing, and rendering in one runtime, while platform shells handle packaging, windows, browser surfaces, lifecycle, and operating-system integration.",
                )
                .build(ctx, view),
                Row {
                    children: vec![
                        LinkCard::new(
                            "Runtime",
                            "One shared runtime",
                            "State, reducers, layout, semantics, and rendering stay in one app model.",
                            "See the model ->",
                            "/docs/learn/runtime-model/",
                        )
                        .build(ctx, view),
                        LinkCard::new(
                            "Targets",
                            "Four real target families",
                            "Desktop, web, Android, and iOS hosts already exist around the same app code.",
                            "See targets ->",
                            "/docs/learn/examples-and-targets/",
                        )
                        .build(ctx, view),
                        LinkCard::new(
                            "Testing",
                            "Built for verification",
                            "Live tests, diagnostics, semantics, and layout inspection are part of the runtime story.",
                            "See testing ->",
                            "/docs/guides/testing-and-diagnostics/",
                        )
                        .build(ctx, view),
                        LinkCard::new(
                            "CLI",
                            "Target scaffolding included",
                            "Project setup and host generation are already part of the command-line workflow.",
                            "See host setup ->",
                            "/docs/guides/platform-shells-cli-and-testing/",
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
                    "Desktop, web, Android, and iOS stay in the same orbit.",
                    "Start on the host that answers your next product question fastest, then keep the shared model intact.",
                )
                .build(ctx, view),
                Column {
                    children: vec![
                        TargetRowCard::new("Desktop", "Supported", "macOS - Linux - Windows", "cargo run -p counter", "Fast local loop for reducers, overlays, layout, and diagnostics.", "/docs/learn/examples-and-targets/", "Desktop path ->").build(ctx, view),
                        TargetRowCard::new("Web", "Smoke path", "WASM", "./examples/web-smoke/platforms/web/run-browser.sh", "Browser host path and generated launcher folder around the same app model.", "/docs/guides/platform-shells-cli-and-testing/", "Web path ->").build(ctx, view),
                        TargetRowCard::new("Android", "Smoke path", "Emulator", "./examples/mobile-smoke/platforms/android/run-emulator.sh", "Checked-in emulator path and generated Android host folder.", "/docs/guides/platform-shells-cli-and-testing/", "Android path ->").build(ctx, view),
                        TargetRowCard::new("iOS", "Smoke path", "Simulator", "./examples/mobile-smoke/platforms/ios/run-sim.sh", "Checked-in simulator path and generated iOS host folder.", "/docs/guides/platform-shells-cli-and-testing/", "iOS path ->").build(ctx, view),
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
            "Small loop, real app shell, large custom tool surface.",
            "Start where your evaluation needs the most signal.",
            vec![
                ExampleCard::new("Starter", "Counter", "cargo run -p counter", "The smallest complete Fission app loop: plain state, two reducers, a widget tree, and buttons bound with the public prelude macros.", "typed actions and reducers", "single-file starter app", "/docs/cookbook/build-a-counter/", "/reference/core/state-system/").build(ctx, view),
                ExampleCard::new("Product shell", "Inbox", "cargo run -p inbox", "A product-like mail app that exercises portals, theme switching, locale switching, routing, and host capabilities in one shell.", "translation bundles and locale sync", "OPEN_URL host capability flow", "/docs/guides/theming-and-i18n/", "/reference/core/environment-input-and-ime/").build(ctx, view),
                ExampleCard::new("Custom surface", "Fission Editor", "cargo run -p fission-editor -- .", "The deepest example in the repo: custom editing surface, jobs, timers, portals, terminal panel, and extensive live tests.", "custom render node path", "resource-driven jobs and timers", "/docs/guides/resources-and-async/", "/reference/widgets/media/").build(ctx, view),
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
                    Text::new("Run an app, inspect a host, then go deeper where you need detail.")
                        .size(tokens.typography.heading1_size)
                        .family(tokens.typography.font_family_serif.clone())
                        .line_height(
                            tokens.typography.heading1_size * tokens.typography.line_height_heading,
                        )
                        .weight(tokens.typography.font_weight_bold)
                        .color(tokens.colors.heading)
                        .text_align(TextAlign::Center)
                        .into_node(),
                    Text::new("The shared runtime is sitting right there. The next product question is one cargo command away.")
                        .size(tokens.typography.body_large_size)
                        .line_height(tokens.typography.body_large_size * tokens.typography.line_height_relaxed)
                        .color(tokens.colors.text_secondary)
                        .text_align(TextAlign::Center)
                        .into_node(),
                    Row {
                        children: vec![
                            Cta::new("Run examples", "/docs/learn/examples-and-targets/", true).build(ctx, view),
                            Cta::new("Inspect hosts", "/docs/guides/platform-shells-cli-and-testing/", false).build(ctx, view),
                            NavLink::new("Review testing ->", "/docs/guides/testing-and-diagnostics/").build(ctx, view),
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
