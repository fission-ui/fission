use super::home_nav::HomePageNav;
use super::home_widgets::{
    content_width, page_fill, semantic_column, semantic_row, Cta, NavLink, Pill,
};
use super::state::DocsState;
use fission::op::{AlignItems, Fill, FlexWrap, JustifyContent};
use fission::prelude::*;

#[derive(Clone, Copy, Debug)]
pub(crate) enum MarketingPageKind {
    Overview,
    CrossPlatformApps,
    TerminalApps,
    StaticSites,
    ProductionLifecycle,
    DeveloperTools,
    DesignSystems,
    Charts,
}

#[derive(Clone, Debug)]
pub(crate) struct ProductMarketingPage {
    kind: MarketingPageKind,
}

impl ProductMarketingPage {
    pub(crate) fn new(kind: MarketingPageKind) -> Self {
        Self { kind }
    }
}

#[derive(Clone, Copy)]
struct PageCopy {
    eyebrow: &'static str,
    title: &'static str,
    body: &'static str,
    primary_label: &'static str,
    primary_href: &'static str,
    secondary_label: &'static str,
    secondary_href: &'static str,
    proof_label: &'static str,
    proof_body: &'static str,
    features: &'static [FeatureCopy],
    workflow: &'static [StepCopy],
}

#[derive(Clone, Copy)]
struct FeatureCopy {
    label: &'static str,
    title: &'static str,
    body: &'static str,
}

#[derive(Clone, Copy)]
struct StepCopy {
    label: &'static str,
    body: &'static str,
}

const OVERVIEW_FEATURES: &[FeatureCopy] = &[
    FeatureCopy { label: "Runtime", title: "One app model", body: "State, actions, reducers, selectors, widgets, resources, jobs, services, capabilities, and design systems stay in shared Rust." },
    FeatureCopy { label: "Targets", title: "Every major surface", body: "Desktop, web, Android, iOS, terminal UI, and static HTML site targets are treated as product outputs, not side projects." },
    FeatureCopy { label: "Lifecycle", title: "Tooling after build", body: "Readiness checks, packages, signing, release content, app stores, static hosting, GitHub Releases, rollouts, and receipts are part of the platform." },
];

const OVERVIEW_STEPS: &[StepCopy] = &[
    StepCopy {
        label: "Start",
        body: "Create a project, understand the app shape, and add the targets you need.",
    },
    StepCopy {
        label: "Develop",
        body: "Run on devices, attach logs, use the right shell, and keep product behavior shared.",
    },
    StepCopy {
        label: "Ship",
        body: "Check readiness, package, sign, publish, manage tracks, and keep receipts for CI.",
    },
];

const CROSS_FEATURES: &[FeatureCopy] = &[
    FeatureCopy { label: "Desktop", title: "Native app loop", body: "Windows, macOS, and Linux provide the fast local loop for product UI, diagnostics, and desktop packaging paths." },
    FeatureCopy { label: "Mobile", title: "Real host projects", body: "Android and iOS hosts keep the shared model intact while validating touch, lifecycle, safe areas, keyboards, package outputs, and stores." },
    FeatureCopy { label: "Web", title: "Browser delivery", body: "The web shell compiles the app for browser delivery without moving product behavior into a separate JavaScript application." },
];

const CROSS_STEPS: &[StepCopy] = &[
    StepCopy { label: "Choose the fastest loop", body: "Use desktop when it answers the current product question fastest." },
    StepCopy { label: "Switch to the host", body: "Use browser, emulator, simulator, or device runs when the host is part of the behavior." },
    StepCopy { label: "Package intentionally", body: "Move from target run to target package without losing the app model." },
];

const TERMINAL_FEATURES: &[FeatureCopy] = &[
    FeatureCopy { label: "Real Fission app", title: "Screens and reducers", body: "Terminal apps use the same state, route, reducer, screen, and component structure as graphical Fission apps." },
    FeatureCopy { label: "Command workflows", title: "Non-blocking tools", body: "Long-running checks, builds, logs, and release commands can run without freezing the interface." },
    FeatureCopy { label: "Verification", title: "Terminal-compatible output", body: "The terminal shell verifies lowered output against terminal capabilities instead of pretending every graphical widget works in cells." },
];

const TERMINAL_STEPS: &[StepCopy] = &[
    StepCopy { label: "Navigate", body: "Use keyboard and supported pointer input for screens, dialogs, settings, and command flows." },
    StepCopy { label: "Observe", body: "Keep bounded scrollback, command sessions, logs, and status visible." },
    StepCopy { label: "Automate", body: "Use the same CLI workflows behind a friendlier interactive shell." },
];

const STATIC_FEATURES: &[FeatureCopy] = &[
    FeatureCopy { label: "Custom pages", title: "Designed landing pages", body: "Homepages and product pages are normal Fission widgets rendered to static HTML at build time." },
    FeatureCopy { label: "Content routes", title: "Markdown at scale", body: "Documentation and reference pages come from content folders, front matter, explicit sidebars, generated headings, and templates." },
    FeatureCopy { label: "SEO", title: "Static output", body: "The site shell emits ordinary HTML, CSS, metadata, search indexes, assets, favicon links, and structured data support." },
];

const STATIC_STEPS: &[StepCopy] = &[
    StepCopy {
        label: "Design",
        body: "Build bespoke landing pages with Fission widget components.",
    },
    StepCopy {
        label: "Author",
        body: "Write docs and reference content in Markdown or MDX under content folders.",
    },
    StepCopy {
        label: "Publish",
        body: "Generate static output for GitHub Pages or another static host.",
    },
];

const LIFECYCLE_FEATURES: &[FeatureCopy] = &[
    FeatureCopy { label: "Readiness", title: "Know what is missing", body: "Preflight checks report SDKs, package tools, signing inputs, credentials, store metadata, and artifact shape before release day." },
    FeatureCopy { label: "Artifacts", title: "Manifests and receipts", body: "Package outputs carry hashes, sizes, MIME types, targets, formats, validation state, and distribution receipts." },
    FeatureCopy { label: "Distribution", title: "Stores and hosts", body: "GitHub Releases, static hosting, object storage, Google Play, App Store Connect, and Microsoft Store paths fit one workflow." },
];

const LIFECYCLE_STEPS: &[StepCopy] = &[
    StepCopy { label: "Package", body: "Produce installable or uploadable artifacts for the selected target." },
    StepCopy { label: "Validate", body: "Check release metadata, screenshots, signing, credentials, and provider requirements." },
    StepCopy { label: "Distribute", body: "Publish to stores, static hosts, package flights, testers, tracks, and rollout channels." },
];

const DEVTOOLS_FEATURES: &[FeatureCopy] = &[
    FeatureCopy { label: "Inspector", title: "See the app model", body: "Inspect widget output, Core IR, layout boxes, semantics, focus, hit testing, and paint order." },
    FeatureCopy { label: "Diagnostics", title: "Follow behavior", body: "Trace actions, reducers, state snapshots, logs, resource calls, command output, and target diagnostics." },
    FeatureCopy { label: "IDE workflow", title: "Bring tools to the editor", body: "Plugins should expose targets, tasks, diagnostics, inspector links, docs, and release checks where developers already work." },
];

const DEVTOOLS_STEPS: &[StepCopy] = &[
    StepCopy {
        label: "Inspect",
        body: "Understand what widgets produced and how layout resolved.",
    },
    StepCopy {
        label: "Diagnose",
        body: "Correlate actions, resources, logs, device output, and frame timing.",
    },
    StepCopy {
        label: "Improve",
        body:
            "Use profiler and test output to fix performance and correctness issues before release.",
    },
];

const DESIGN_FEATURES: &[FeatureCopy] = &[
    FeatureCopy { label: "DSP JSON", title: "Bring your system", body: "Read design system package JSON at build time and generate typed Rust theme structures." },
    FeatureCopy { label: "Components", title: "Variants and states", body: "Apply sizes, variants, hover, active, focus, disabled, error, shadows, borders, typography, icon sizing, and dark/light behavior." },
    FeatureCopy { label: "Charts", title: "Visualization palette", body: "Use generated data-visualization palettes so charts match the product design system." },
];

const DESIGN_STEPS: &[StepCopy] = &[
    StepCopy {
        label: "Generate",
        body: "Convert design system JSON into typed theme code during the build.",
    },
    StepCopy {
        label: "Select",
        body: "Set the active theme through Env from user or platform preference.",
    },
    StepCopy {
        label: "Apply",
        body: "Let widgets, charts, shells, and product surfaces consume the same tokens.",
    },
];

const CHART_FEATURES: &[FeatureCopy] = &[
    FeatureCopy { label: "Breadth", title: "Large chart catalog", body: "Line, bar, area, pie, scatter, heatmap, financial, relationship, map, component, dynamic, and 3D-oriented families." },
    FeatureCopy { label: "Product fit", title: "Dashboards and analytics", body: "Build monitoring, finance, operations, reporting, planning, and decision-support surfaces without leaving Fission." },
    FeatureCopy { label: "Design", title: "Theme-aware visuals", body: "Charts consume the design system palette, typography, backgrounds, dark mode, and interaction rules." },
];

const CHART_STEPS: &[StepCopy] = &[
    StepCopy {
        label: "Browse",
        body: "Use the catalog to choose a family and variant.",
    },
    StepCopy {
        label: "Configure",
        body:
            "Bind series, datasets, axes, legends, tooltips, interaction, and animation settings.",
    },
    StepCopy {
        label: "Ship",
        body: "Document and test charts with generated screenshots and gallery examples.",
    },
];

impl MarketingPageKind {
    fn copy(self) -> PageCopy {
        match self {
            MarketingPageKind::Overview => PageCopy {
                eyebrow: "Fission platform",
                title: "A Rust application platform for the full product lifecycle.",
                body: "Build the interface, run it on real targets, test and debug it, package artifacts, prepare release content, publish through stores and hosts, and keep receipts for automation.",
                primary_label: "Start docs",
                primary_href: "/docs/intro/",
                secondary_label: "See release workflow",
                secondary_href: "/docs/release-and-distribute/overview/",
                proof_label: "One product model",
                proof_body: "Fission keeps product behavior in shared Rust while shells and tooling handle the host and lifecycle edges.",
                features: OVERVIEW_FEATURES,
                workflow: OVERVIEW_STEPS,
            },
            MarketingPageKind::CrossPlatformApps => PageCopy {
                eyebrow: "Cross-platform apps",
                title: "One Rust app model across desktop, mobile, and web.",
                body: "Fission keeps state, reducers, widgets, resources, jobs, services, design systems, and charts shared while shells host the product on each platform.",
                primary_label: "Read shell guide",
                primary_href: "/docs/guides/platform-shells-cli-and-testing/",
                secondary_label: "Browse targets",
                secondary_href: "/docs/learn/examples-and-targets/",
                proof_label: "Real targets",
                proof_body: "Desktop, web, Android, and iOS are positioned as production targets with host-specific validation where the platform boundary matters.",
                features: CROSS_FEATURES,
                workflow: CROSS_STEPS,
            },
            MarketingPageKind::TerminalApps => PageCopy {
                eyebrow: "Terminal UI",
                title: "Build terminal apps without leaving Fission.",
                body: "Terminal UI is for production command tools, setup flows, diagnostics, admin panels, and developer workflows that need an interactive shell surface.",
                primary_label: "Build a terminal app",
                primary_href: "/docs/guides/terminal-user-interfaces/",
                secondary_label: "Try fission ui",
                secondary_href: "/reference/cli/overview/",
                proof_label: "Built into the CLI",
                proof_body: "The Fission CLI UI is implemented as a Fission terminal app with routes, screens, reducers, dialogs, settings, and command sessions.",
                features: TERMINAL_FEATURES,
                workflow: TERMINAL_STEPS,
            },
            MarketingPageKind::StaticSites => PageCopy {
                eyebrow: "Static sites",
                title: "Generate SEO-friendly static sites from Fission widgets and content.",
                body: "Use custom widget routes for marketing pages and Markdown content routes for documentation, reference, blogs, and changelogs.",
                primary_label: "Read static site guide",
                primary_href: "/docs/guides/static-sites/",
                secondary_label: "View this site structure",
                secondary_href: "/docs/release-and-distribute/overview/",
                proof_label: "This site is the example",
                proof_body: "The Fission documentation site is generated by the Fission static site shell, including custom pages, content routes, sidebars, search, and metadata.",
                features: STATIC_FEATURES,
                workflow: STATIC_STEPS,
            },
            MarketingPageKind::ProductionLifecycle => PageCopy {
                eyebrow: "Production lifecycle",
                title: "Package, sign, release, distribute, and track the output.",
                body: "Fission treats post-build work as a platform feature: readiness checks, artifact manifests, release content, credentials, stores, static hosts, tracks, rollouts, and receipts.",
                primary_label: "Open release docs",
                primary_href: "/docs/release-and-distribute/overview/",
                secondary_label: "Lifecycle details",
                secondary_href: "/docs/release-and-distribute/post-build-lifecycle/",
                proof_label: "Release work is product work",
                proof_body: "The CLI gives packaging and distribution the same project model as development instead of leaving each app to invent release scripts.",
                features: LIFECYCLE_FEATURES,
                workflow: LIFECYCLE_STEPS,
            },
            MarketingPageKind::DeveloperTools => PageCopy {
                eyebrow: "Developer tools",
                title: "Make the Fission runtime observable while you build.",
                body: "The developer tools direction is inspection, diagnostics, profiling, screenshots, device workflow, and IDE integration around the same explicit app model.",
                primary_label: "Read testing docs",
                primary_href: "/docs/test-and-debug/overview/",
                secondary_label: "Open reference",
                secondary_href: "/reference/core/testing-and-diagnostics/",
                proof_label: "Debug the architecture you ship",
                proof_body: "Tools should expose state, actions, reducers, layout, semantics, resources, logs, and target output without adding hidden behavior paths.",
                features: DEVTOOLS_FEATURES,
                workflow: DEVTOOLS_STEPS,
            },
            MarketingPageKind::DesignSystems => PageCopy {
                eyebrow: "Design systems",
                title: "Bring a real design system into Rust UI code.",
                body: "Fission reads design system package JSON at build time and generates typed theme code for widgets, charts, shells, and product surfaces.",
                primary_label: "Read design guide",
                primary_href: "/docs/guides/design-system/",
                secondary_label: "Theme docs",
                secondary_href: "/docs/guides/theming-and-i18n/",
                proof_label: "No JSON hot path",
                proof_body: "Design system JSON is converted during the build, then app code selects typed themes through Env at runtime.",
                features: DESIGN_FEATURES,
                workflow: DESIGN_STEPS,
            },
            MarketingPageKind::Charts => PageCopy {
                eyebrow: "Charts",
                title: "Beautiful data visualization as a first-class product surface.",
                body: "Fission Charts is the native charting layer for dashboards, analytics, finance, maps, networks, dynamic data, and 3D-ready visuals.",
                primary_label: "Browse catalog",
                primary_href: "/docs/charts/catalog/",
                secondary_label: "Chart reference",
                secondary_href: "/reference/charts/overview/",
                proof_label: "Built for dashboards",
                proof_body: "The chart catalog is broad because production apps need reporting, monitoring, planning, financial, and decision-support surfaces.",
                features: CHART_FEATURES,
                workflow: CHART_STEPS,
            },
        }
    }
}

impl Widget<DocsState> for ProductMarketingPage {
    fn build(&self, ctx: &mut BuildCtx<DocsState>, view: &View<DocsState>) -> Node {
        let tokens = &view.env.theme.tokens;
        let copy = self.kind.copy();
        Container::new(
            Column {
                children: vec![
                    HomePageNav.build(ctx, view),
                    Row {
                        children: vec![Container::new(semantic_column(
                            "site-product-page",
                            vec![
                                marketing_hero(ctx, view, self.kind, copy),
                                product_nav_strip(ctx, view),
                                feature_showcase(view, copy),
                                workflow_showcase(view, copy),
                                proof_band(ctx, view, copy),
                            ],
                            Some(tokens.spacing.xxxl),
                            AlignItems::Stretch,
                        ))
                        .max_width(content_width(tokens))
                        .flex_grow(1.0)
                        .flex_shrink(1.0)
                        .padding([0.0, 0.0, tokens.spacing.xxl, tokens.spacing.xxxxl])
                        .into_node()],
                        justify_content: JustifyContent::Center,
                        ..Default::default()
                    }
                    .into_node(),
                ],
                gap: Some(0.0),
                flex_grow: 1.0,
                ..Default::default()
            }
            .into_node(),
        )
        .min_height(tokens.spacing.xxxxl * 9.0)
        .bg_fill(page_fill(tokens))
        .into_node()
    }
}

fn marketing_hero(
    ctx: &mut BuildCtx<DocsState>,
    view: &View<DocsState>,
    kind: MarketingPageKind,
    copy: PageCopy,
) -> Node {
    let tokens = &view.env.theme.tokens;
    Container::new(semantic_row(
        "site-product-hero",
        vec![
            Container::new(
                Column {
                    children: vec![
                        Pill::new(copy.eyebrow).build(ctx, view),
                        Text::new(copy.title)
                            .size(tokens.typography.display_md_size)
                            .family(tokens.typography.font_family_serif.clone())
                            .line_height(
                                tokens.typography.display_md_size
                                    * tokens.typography.line_height_display,
                            )
                            .weight(tokens.typography.font_weight_bold)
                            .color(tokens.colors.heading)
                            .max_width(tokens.spacing.xxxxl * 5.4)
                            .flex_shrink(1.0)
                            .semantics_identifier("site-product-hero-title")
                            .into_node(),
                        Text::new(copy.body)
                            .size(tokens.typography.font_size_lg)
                            .line_height(
                                tokens.typography.font_size_lg
                                    * tokens.typography.line_height_relaxed,
                            )
                            .color(tokens.colors.text_secondary)
                            .max_width(tokens.spacing.xxxxl * 5.2)
                            .flex_shrink(1.0)
                            .semantics_identifier("site-product-hero-body")
                            .into_node(),
                        semantic_row(
                            "site-product-hero-ctas",
                            vec![
                                Cta::new(copy.primary_label, copy.primary_href, true)
                                    .build(ctx, view),
                                Cta::new(copy.secondary_label, copy.secondary_href, false)
                                    .build(ctx, view),
                            ],
                            Some(tokens.spacing.m),
                            FlexWrap::Wrap,
                            AlignItems::Center,
                            JustifyContent::Start,
                        ),
                    ],
                    gap: Some(tokens.spacing.l),
                    ..Default::default()
                }
                .into_node(),
            )
            .width(tokens.spacing.xxxxl * 5.45)
            .flex_shrink(1.0)
            .into_node(),
            product_visual(view, kind),
        ],
        Some(tokens.spacing.xxl),
        FlexWrap::Wrap,
        AlignItems::Center,
        JustifyContent::SpaceBetween,
    ))
    .padding_all(tokens.spacing.xxl)
    .bg_fill(Fill::LinearGradient {
        start: (0.0, 0.0),
        end: (1.0, 1.0),
        stops: vec![
            (0.0, tokens.colors.surface.with_alpha(245)),
            (0.52, tokens.colors.surface_sunken.with_alpha(238)),
            (1.0, tokens.colors.primary_subtle.with_alpha(180)),
        ],
    })
    .border(tokens.colors.border, 1.0)
    .border_radius(tokens.radii.xxl)
    .into_node()
}

fn product_nav_strip(ctx: &mut BuildCtx<DocsState>, view: &View<DocsState>) -> Node {
    let tokens = &view.env.theme.tokens;
    Container::new(
        Row {
            children: vec![
                strip_link(ctx, view, "Platform", "/product/overview/"),
                strip_link(ctx, view, "Apps", "/product/cross-platform-apps/"),
                strip_link(ctx, view, "Terminal", "/product/terminal-apps/"),
                strip_link(ctx, view, "Static sites", "/product/static-sites/"),
                strip_link(ctx, view, "Lifecycle", "/product/production-lifecycle/"),
                strip_link(ctx, view, "Dev tools", "/product/developer-tools/"),
                strip_link(ctx, view, "Design", "/product/design-systems/"),
                strip_link(ctx, view, "Charts", "/product/charts/"),
            ],
            gap: Some(tokens.spacing.s),
            wrap: FlexWrap::Wrap,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..Default::default()
        }
        .into_node(),
    )
    .padding_all(tokens.spacing.m)
    .bg_fill(Fill::Solid(tokens.colors.surface.with_alpha(232)))
    .border(tokens.colors.border, 1.0)
    .border_radius(tokens.radii.full)
    .into_node()
}

fn strip_link(
    ctx: &mut BuildCtx<DocsState>,
    view: &View<DocsState>,
    label: &'static str,
    href: &'static str,
) -> Node {
    let tokens = &view.env.theme.tokens;
    Container::new(NavLink::new(label, href).build(ctx, view))
        .padding([
            tokens.spacing.m,
            tokens.spacing.m,
            tokens.spacing.s,
            tokens.spacing.s,
        ])
        .bg_fill(Fill::Solid(tokens.colors.surface_raised))
        .border(tokens.colors.border, 1.0)
        .border_radius(tokens.radii.full)
        .into_node()
}

fn feature_showcase(view: &View<DocsState>, copy: PageCopy) -> Node {
    let tokens = &view.env.theme.tokens;
    semantic_row(
        "site-product-feature-showcase",
        vec![
            Column {
                children: vec![
                    Text::new("What teams get")
                        .size(tokens.typography.font_size_sm)
                        .weight(tokens.typography.font_weight_bold)
                        .color(tokens.colors.secondary)
                        .into_node(),
                    Text::new("A product surface, not a loose collection of pages.")
                        .size(tokens.typography.heading2_size)
                        .family(tokens.typography.font_family_serif.clone())
                        .line_height(
                            tokens.typography.heading2_size * tokens.typography.line_height_heading,
                        )
                        .weight(tokens.typography.font_weight_bold)
                        .color(tokens.colors.heading)
                        .into_node(),
                    Text::new("Each part of the platform has a clear job, and each job links back to the same Rust app model.")
                        .size(tokens.typography.body_large_size)
                        .line_height(
                            tokens.typography.body_large_size
                                * tokens.typography.line_height_relaxed,
                        )
                        .color(tokens.colors.text_secondary)
                        .into_node(),
                ],
                gap: Some(tokens.spacing.m),
                flex_grow: 1.0,
                ..Default::default()
            }
            .into_node(),
            Row {
                children: copy
                    .features
                    .iter()
                    .map(|feature| feature_card(view, *feature))
                    .collect(),
                gap: Some(tokens.spacing.m),
                wrap: FlexWrap::Wrap,
                align_items: AlignItems::Stretch,
                justify_content: JustifyContent::End,
                flex_grow: 1.0,
                ..Default::default()
            }
            .into_node(),
        ],
        Some(tokens.spacing.xxl),
        FlexWrap::Wrap,
        AlignItems::Stretch,
        JustifyContent::SpaceBetween,
    )
}

fn feature_card(view: &View<DocsState>, feature: FeatureCopy) -> Node {
    let tokens = &view.env.theme.tokens;
    Container::new(
        Column {
            children: vec![
                Text::new(feature.label)
                    .size(tokens.typography.font_size_xs)
                    .family(tokens.typography.font_family_mono.clone())
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.primary)
                    .into_node(),
                Text::new(feature.title)
                    .size(tokens.typography.heading_size)
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.heading)
                    .into_node(),
                Text::new(feature.body)
                    .size(tokens.typography.body_medium_size)
                    .line_height(
                        tokens.typography.body_medium_size * tokens.typography.line_height_relaxed,
                    )
                    .color(tokens.colors.text_secondary)
                    .flex_shrink(1.0)
                    .into_node(),
            ],
            gap: Some(tokens.spacing.m),
            ..Default::default()
        }
        .into_node(),
    )
    .width(tokens.spacing.xxxxl * 3.1)
    .min_height(tokens.spacing.xxxxl * 2.05)
    .flex_shrink(1.0)
    .padding_all(tokens.spacing.l)
    .bg_fill(Fill::Solid(tokens.colors.surface_raised))
    .border(tokens.colors.border, 1.0)
    .border_radius(tokens.radii.xl)
    .into_node()
}

fn workflow_showcase(view: &View<DocsState>, copy: PageCopy) -> Node {
    let tokens = &view.env.theme.tokens;
    Container::new(
        Column {
            children: vec![
                Row {
                    children: vec![
                        Text::new("Workflow")
                            .size(tokens.typography.font_size_sm)
                            .weight(tokens.typography.font_weight_bold)
                            .color(tokens.colors.primary)
                            .into_node(),
                        Text::new("The path stays explicit from first run to release.")
                            .size(tokens.typography.heading_size)
                            .weight(tokens.typography.font_weight_bold)
                            .color(tokens.colors.heading)
                            .into_node(),
                    ],
                    gap: Some(tokens.spacing.l),
                    wrap: FlexWrap::Wrap,
                    align_items: AlignItems::Center,
                    ..Default::default()
                }
                .into_node(),
                Row {
                    children: copy
                        .workflow
                        .iter()
                        .enumerate()
                        .map(|(index, step)| workflow_step(view, index + 1, *step))
                        .collect(),
                    gap: Some(tokens.spacing.m),
                    wrap: FlexWrap::Wrap,
                    align_items: AlignItems::Stretch,
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
    .padding_all(tokens.spacing.xl)
    .bg_fill(Fill::Solid(tokens.colors.surface))
    .border(tokens.colors.border, 1.0)
    .border_radius(tokens.radii.xxl)
    .into_node()
}

fn workflow_step(view: &View<DocsState>, index: usize, step: StepCopy) -> Node {
    let tokens = &view.env.theme.tokens;
    Container::new(
        Column {
            children: vec![
                Text::new(format!("{:02}", index))
                    .size(tokens.typography.font_size_xs)
                    .family(tokens.typography.font_family_mono.clone())
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.primary)
                    .into_node(),
                Text::new(step.label)
                    .size(tokens.typography.font_size_lg)
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.heading)
                    .into_node(),
                Text::new(step.body)
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
    .width(tokens.spacing.xxxxl * 3.05)
    .min_height(tokens.spacing.xxxxl * 1.35)
    .flex_shrink(1.0)
    .padding_all(tokens.spacing.l)
    .bg_fill(Fill::Solid(tokens.colors.surface_raised))
    .border(tokens.colors.border, 1.0)
    .border_radius(tokens.radii.large)
    .into_node()
}

fn proof_band(ctx: &mut BuildCtx<DocsState>, view: &View<DocsState>, copy: PageCopy) -> Node {
    let tokens = &view.env.theme.tokens;
    Container::new(semantic_row(
        "site-product-proof",
        vec![
            Column {
                children: vec![
                    Text::new(copy.proof_label)
                        .size(tokens.typography.heading1_size)
                        .family(tokens.typography.font_family_serif.clone())
                        .line_height(
                            tokens.typography.heading1_size * tokens.typography.line_height_heading,
                        )
                        .weight(tokens.typography.font_weight_bold)
                        .color(tokens.colors.heading)
                        .into_node(),
                    Text::new(copy.proof_body)
                        .size(tokens.typography.body_large_size)
                        .line_height(
                            tokens.typography.body_large_size
                                * tokens.typography.line_height_relaxed,
                        )
                        .color(tokens.colors.text_secondary)
                        .into_node(),
                ],
                gap: Some(tokens.spacing.m),
                flex_grow: 1.0,
                ..Default::default()
            }
            .into_node(),
            Cta::new("Open documentation", "/docs/intro/", true).build(ctx, view),
        ],
        Some(tokens.spacing.xl),
        FlexWrap::Wrap,
        AlignItems::Center,
        JustifyContent::SpaceBetween,
    ))
    .padding_all(tokens.spacing.xl)
    .bg_fill(Fill::LinearGradient {
        start: (0.0, 0.0),
        end: (1.0, 1.0),
        stops: vec![
            (0.0, tokens.colors.primary_subtle.with_alpha(200)),
            (1.0, tokens.colors.surface_sunken),
        ],
    })
    .border(tokens.colors.border, 1.0)
    .border_radius(tokens.radii.xxl)
    .into_node()
}

fn product_visual(view: &View<DocsState>, kind: MarketingPageKind) -> Node {
    let tokens = &view.env.theme.tokens;
    let children = match kind {
        MarketingPageKind::Charts => chart_visual(view),
        MarketingPageKind::TerminalApps => terminal_visual(view),
        MarketingPageKind::StaticSites => site_visual(view),
        MarketingPageKind::ProductionLifecycle => lifecycle_visual(view),
        MarketingPageKind::DeveloperTools => devtools_visual(view),
        MarketingPageKind::DesignSystems => design_visual(view),
        MarketingPageKind::CrossPlatformApps => target_visual(view),
        MarketingPageKind::Overview => platform_visual(view),
    };
    Container::new(children)
        .width(tokens.spacing.xxxxl * 4.35)
        .flex_shrink(1.0)
        .padding_all(tokens.spacing.l)
        .bg_fill(Fill::Solid(tokens.colors.surface_raised.with_alpha(246)))
        .border(tokens.colors.border, 1.0)
        .border_radius(tokens.radii.xxl)
        .into_node()
}

fn platform_visual(view: &View<DocsState>) -> Node {
    visual_stack(
        view,
        "Platform map",
        &[
            ("App model", "State / reducers / widgets"),
            ("Targets", "Desktop / Web / Mobile / TUI / Static"),
            ("Lifecycle", "Package / sign / release / receipts"),
        ],
    )
}

fn target_visual(view: &View<DocsState>) -> Node {
    visual_stack(
        view,
        "Target matrix",
        &[
            ("Desktop", "Windows  macOS  Linux"),
            ("Mobile", "Android  iOS"),
            ("Web", "WASM browser shell"),
            ("Specialized", "Terminal UI  Static HTML"),
        ],
    )
}

fn terminal_visual(view: &View<DocsState>) -> Node {
    visual_stack(
        view,
        "fission ui",
        &[
            ("Dashboard", "doctor running..."),
            ("Logs", "47 checks passed"),
            ("Settings", "theme: dark  density: compact"),
            ("Command", "non-blocking session attached"),
        ],
    )
}

fn site_visual(view: &View<DocsState>) -> Node {
    visual_stack(
        view,
        "Static site build",
        &[
            ("Custom route", "/product/overview/"),
            ("Content route", "/docs/learn/quickstart/"),
            ("Generated", "HTML  CSS  search  sitemap"),
        ],
    )
}

fn lifecycle_visual(view: &View<DocsState>) -> Node {
    visual_stack(
        view,
        "Release pipeline",
        &[
            ("Preflight", "SDKs  signing  credentials"),
            ("Package", "artifact-manifest.json"),
            ("Publish", "stores  hosts  releases"),
            ("Receipt", "CI-readable output"),
        ],
    )
}

fn devtools_visual(view: &View<DocsState>) -> Node {
    visual_stack(
        view,
        "Inspector surface",
        &[
            ("Widget tree", "routes / screens / components"),
            ("Core IR", "layout / semantics / paint"),
            ("Runtime", "actions / reducers / resources"),
        ],
    )
}

fn design_visual(view: &View<DocsState>) -> Node {
    visual_stack(
        view,
        "Design system",
        &[
            ("DSP JSON", "tokens and components"),
            ("Codegen", "typed Rust theme"),
            ("Runtime", "Env selects active theme"),
            ("Surfaces", "widgets and charts"),
        ],
    )
}

fn chart_visual(view: &View<DocsState>) -> Node {
    let tokens = &view.env.theme.tokens;
    Column {
        children: vec![
            visual_header(view, "Chart surfaces"),
            Row {
                children: vec![
                    chart_thumb(view, "/img/charts/line-gradient-area.png"),
                    chart_thumb(view, "/img/charts/bar-horizontal.png"),
                ],
                gap: Some(tokens.spacing.s),
                wrap: FlexWrap::Wrap,
                ..Default::default()
            }
            .into_node(),
            Row {
                children: vec![
                    chart_thumb(view, "/img/charts/sankey-energy.png"),
                    chart_thumb(view, "/img/charts/surface3d-wave.png"),
                ],
                gap: Some(tokens.spacing.s),
                wrap: FlexWrap::Wrap,
                ..Default::default()
            }
            .into_node(),
        ],
        gap: Some(tokens.spacing.m),
        ..Default::default()
    }
    .into_node()
}

fn visual_stack(
    view: &View<DocsState>,
    title: &'static str,
    rows: &[(&'static str, &'static str)],
) -> Node {
    let tokens = &view.env.theme.tokens;
    Column {
        children: std::iter::once(visual_header(view, title))
            .chain(
                rows.iter()
                    .map(|(label, body)| visual_row(view, label, body)),
            )
            .collect(),
        gap: Some(tokens.spacing.m),
        ..Default::default()
    }
    .into_node()
}

fn visual_header(view: &View<DocsState>, title: &'static str) -> Node {
    let tokens = &view.env.theme.tokens;
    Row {
        children: vec![
            dot(tokens.colors.error),
            dot(tokens.colors.warning),
            dot(tokens.colors.success),
            Text::new(title)
                .size(tokens.typography.font_size_sm)
                .family(tokens.typography.font_family_mono.clone())
                .color(tokens.colors.text_secondary)
                .into_node(),
        ],
        gap: Some(tokens.spacing.s),
        align_items: AlignItems::Center,
        ..Default::default()
    }
    .into_node()
}

fn visual_row(view: &View<DocsState>, label: &'static str, body: &'static str) -> Node {
    let tokens = &view.env.theme.tokens;
    Container::new(
        Row {
            children: vec![
                Text::new(label)
                    .size(tokens.typography.font_size_sm)
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.heading)
                    .into_node(),
                Text::new(body)
                    .size(tokens.typography.font_size_sm)
                    .family(tokens.typography.font_family_mono.clone())
                    .color(tokens.colors.text_secondary)
                    .into_node(),
            ],
            gap: Some(tokens.spacing.m),
            wrap: FlexWrap::Wrap,
            justify_content: JustifyContent::SpaceBetween,
            ..Default::default()
        }
        .into_node(),
    )
    .padding_all(tokens.spacing.m)
    .bg_fill(Fill::Solid(tokens.colors.surface))
    .border(tokens.colors.border, 1.0)
    .border_radius(tokens.radii.large)
    .into_node()
}

fn chart_thumb(view: &View<DocsState>, src: &'static str) -> Node {
    let tokens = &view.env.theme.tokens;
    Container::new(
        Image {
            source: src.to_string(),
            width: Some(tokens.spacing.xxxxl * 1.85),
            height: Some(tokens.spacing.xxxxl * 1.05),
            ..Default::default()
        }
        .into_node(),
    )
    .padding_all(tokens.spacing.xs)
    .bg_fill(Fill::Solid(tokens.colors.on_surface.with_alpha(245)))
    .border_radius(tokens.radii.large)
    .into_node()
}

fn dot(color: Color) -> Node {
    Container::new(Text::new(" ").into_node())
        .width(9.0)
        .height(9.0)
        .bg_fill(Fill::Solid(color))
        .border_radius(99.0)
        .into_node()
}
