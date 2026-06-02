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
    ServerSites,
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
    proof_cta_label: &'static str,
    proof_cta_href: &'static str,
    feature_label: &'static str,
    feature_title: &'static str,
    feature_body: &'static str,
    features: &'static [FeatureCopy],
    details_label: &'static str,
    details_title: &'static str,
    details_body: &'static str,
    details: &'static [DetailCopy],
    workflow_label: &'static str,
    workflow_title: &'static str,
    workflow: &'static [StepCopy],
}

#[derive(Clone, Copy)]
struct FeatureCopy {
    label: &'static str,
    title: &'static str,
    body: &'static str,
}

#[derive(Clone, Copy)]
struct DetailCopy {
    label: &'static str,
    title: &'static str,
    body: &'static str,
    href: &'static str,
    link_label: &'static str,
}

#[derive(Clone, Copy)]
struct StepCopy {
    label: &'static str,
    body: &'static str,
}

const OVERVIEW_FEATURES: &[FeatureCopy] = &[
    FeatureCopy { label: "Runtime", title: "One app model", body: "State, actions, reducers, selectors, widgets, resources, jobs, services, capabilities, and design systems stay in shared Rust." },
    FeatureCopy { label: "Targets", title: "Every major surface", body: "Desktop, web, Android, iOS, terminal UI, static HTML, and server-rendered site targets are treated as product outputs, not side projects." },
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

const OVERVIEW_DETAILS: &[DetailCopy] = &[
    DetailCopy { label: "Shared runtime", title: "One application shape", body: "A product keeps one model for state, actions, reducers, components, resources, jobs, services, capabilities, and themes. Targets host that model; they do not fork it.", href: "/docs/learn/runtime-model/", link_label: "Runtime model" },
    DetailCopy { label: "Target breadth", title: "More than windowed apps", body: "Fission treats desktop, browser, mobile, terminal, static HTML, and server-rendered HTML as first-class delivery surfaces for the same product architecture.", href: "/reference/platform/targets/", link_label: "Targets reference" },
    DetailCopy { label: "Lifecycle tooling", title: "The work after build is covered", body: "The platform story includes readiness checks, packaging, signing, release content, distribution providers, receipts, and CI-friendly automation.", href: "/docs/release-and-distribute/overview/", link_label: "Release docs" },
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

const CROSS_DETAILS: &[DetailCopy] = &[
    DetailCopy { label: "Desktop", title: "Use the fastest local host", body: "Windows, macOS, and Linux keep the iteration loop short while still exercising layout, rendering, input, capabilities, and packaging expectations.", href: "/docs/build-and-package/desktop-packages/", link_label: "Desktop packages" },
    DetailCopy { label: "Mobile", title: "Validate mobile as mobile", body: "Android and iOS runs cover touch input, safe areas, keyboard behavior, app metadata, permissions, icons, launch screens, and store-bound package formats.", href: "/docs/build-and-package/mobile-packages/", link_label: "Mobile packages" },
    DetailCopy { label: "Browser", title: "Ship the app to the web", body: "The web target keeps application behavior in Rust while the browser host owns WebAssembly loading, renderer selection, assets, and web-specific diagnostics.", href: "/docs/build-and-package/web-packages/", link_label: "Web packages" },
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

const TERMINAL_DETAILS: &[DetailCopy] = &[
    DetailCopy { label: "Information architecture", title: "Screens before command lists", body: "A good terminal app should present tasks, status, confirmation dialogs, settings, logs, and scrollback progressively instead of dumping every command upfront.", href: "/docs/guides/terminal-user-interfaces/", link_label: "TUI guide" },
    DetailCopy { label: "Interaction", title: "Keyboard first, pointer aware", body: "The shell supports keyboard navigation and pointer input where terminals expose it, while keeping the interface usable when only keys are available.", href: "/reference/widgets/terminal-view/", link_label: "Terminal widgets" },
    DetailCopy { label: "Packaging", title: "Treat terminal apps as products", body: "Terminal apps can be packaged as command-line tools, installers, or release assets, with configuration and docs alongside graphical targets.", href: "/docs/build-and-package/terminal-packages/", link_label: "Package terminal apps" },
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

const STATIC_DETAILS: &[DetailCopy] = &[
    DetailCopy { label: "Custom pages", title: "Marketing pages are widgets", body: "Homepages and product pages are normal Fission component trees, so layout, theme, links, imagery, and footer structure stay in Rust instead of a separate template language.", href: "/docs/guides/static-site-custom-pages/", link_label: "Custom pages" },
    DetailCopy { label: "Content routes", title: "Markdown scales the docs", body: "Folders under `content/` become route trees with front matter, sidebars, tables, code blocks, in-page links, metadata, generated blog landing pages, and search data.", href: "/docs/guides/static-site-content-routes/", link_label: "Content routes" },
    DetailCopy { label: "Publishing", title: "Static output is portable", body: "The build emits ordinary HTML, CSS, assets, sitemap, robots, favicon, search index, and optional page elements for GitHub Pages or other static hosts.", href: "/docs/build-and-package/static-site-packages/", link_label: "Package static sites" },
];

const SERVER_FEATURES: &[FeatureCopy] = &[
    FeatureCopy { label: "Request-time routes", title: "Dynamic HTML", body: "Render pages when they need sessions, request data, private state, or cache revalidation instead of forcing every route to be static." },
    FeatureCopy { label: "Server work", title: "Jobs and signed actions", body: "Drain typed server jobs before responding and handle user intent through signed action posts that reducers can validate and retry safely." },
    FeatureCopy { label: "Focused browser code", title: "Workers and islands", body: "Attach progressive workers or small WASM islands where a page needs browser-side behavior without turning the whole site into one large app." },
];

const SERVER_STEPS: &[StepCopy] = &[
    StepCopy {
        label: "Model",
        body: "Choose which routes are public, revalidated, request-rendered, or session-private.",
    },
    StepCopy {
        label: "Render",
        body: "Use Fission widgets for page structure while jobs and services prepare data.",
    },
    StepCopy {
        label: "Deploy",
        body: "Package the server as a Docker image with generated assets and security checks.",
    },
];

const SERVER_DETAILS: &[DetailCopy] = &[
    DetailCopy { label: "Route modes", title: "Static, cached, or private", body: "A server site can combine long-lived pages, time-to-live revalidation, request-rendered pages, and session-private pages in one product.", href: "/docs/guides/server-sites/", link_label: "Server guide" },
    DetailCopy { label: "User intent", title: "Signed actions, reducers, and jobs", body: "Forms and browser actions post signed payloads back to the server, reducers update state, and jobs/services prepare data before HTML is returned.", href: "/docs/guides/server-site-caching-jobs-actions/", link_label: "Actions and jobs" },
    DetailCopy { label: "Deployment", title: "Server assets are part of the package", body: "Workers, islands, static assets, server binaries, Docker image metadata, and security settings are generated and checked together.", href: "/docs/build-and-package/server-site-packages/", link_label: "Package server sites" },
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

const LIFECYCLE_DETAILS: &[DetailCopy] = &[
    DetailCopy { label: "Preflight", title: "Find missing release inputs early", body: "Project checks verify SDKs, package tools, signing references, release metadata, icons, screenshots, credentials, and provider-specific requirements.", href: "/docs/release-and-distribute/release-content/", link_label: "Release content" },
    DetailCopy { label: "Artifacts", title: "Every output gets a manifest", body: "Packages record target, format, path, hashes, size, MIME type, signing state, cache policy, upload intent, and distribution readiness.", href: "/docs/build-and-package/overview/", link_label: "Packaging overview" },
    DetailCopy { label: "Providers", title: "Publish through the right channel", body: "Release flows cover app stores, GitHub Releases, static hosts, Docker registries, and object storage without turning each app into a custom script pile.", href: "/docs/release-and-distribute/overview/", link_label: "Distribution overview" },
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

const DEVTOOLS_DETAILS: &[DetailCopy] = &[
    DetailCopy { label: "Inspector", title: "See what the runtime sees", body: "The developer-tooling direction is to expose component output, layout boxes, semantics, paint order, focus state, hit testing, and route state.", href: "/docs/test-and-debug/overview/", link_label: "Test and debug" },
    DetailCopy { label: "Traces", title: "Follow actions through reducers", body: "A useful tool should connect user input, action dispatch, reducer execution, state snapshots, resource calls, logs, and rendered output.", href: "/reference/core/testing-and-diagnostics/", link_label: "Diagnostics reference" },
    DetailCopy { label: "Editors", title: "Bring project context to the IDE", body: "Plugins and agent tools should surface docs, targets, screenshots, layout inspection, test runners, and release checks where developers already work.", href: "/docs/develop/workflow/", link_label: "Developer workflow" },
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

const DESIGN_DETAILS: &[DetailCopy] = &[
    DetailCopy { label: "Build-time codegen", title: "JSON becomes typed Rust", body: "Design System Package JSON is read during the build, generating Rust structures so widgets do not parse JSON on the rendering hot path.", href: "/docs/guides/design-system/", link_label: "Design guide" },
    DetailCopy { label: "Runtime selection", title: "Env chooses the active theme", body: "Applications can select light, dark, brand, density, or locale-aware themes in the environment callback while widgets consume stable tokens.", href: "/docs/guides/theming-and-i18n/", link_label: "Theme and locale" },
    DetailCopy { label: "Component states", title: "Variants are product rules", body: "Buttons, inputs, charts, surfaces, typography, shadows, borders, icon sizes, and interaction states should all come from the same design model.", href: "/reference/core/theming-and-i18n/", link_label: "Theme reference" },
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

const CHART_DETAILS: &[DetailCopy] = &[
    DetailCopy { label: "Catalog", title: "Choose a family deliberately", body: "The reference catalog groups chart families and variants so dashboards can use the right visual form for comparison, trend, distribution, hierarchy, geography, or topology.", href: "/docs/charts/catalog/", link_label: "Chart catalog" },
    DetailCopy { label: "Datasets", title: "Model data before visuals", body: "Series and datasets define how data enters the chart, including static sets, streaming updates, and larger histories that should not be loaded all at once.", href: "/docs/charts/series-and-datasets/", link_label: "Series and datasets" },
    DetailCopy { label: "Interaction", title: "Tooltips, selection, and motion belong to the chart", body: "Production charts need theme-aware animation, interaction rules, legends, toolboxes, hit testing, and screenshots that prove the output.", href: "/docs/charts/data-and-interaction/", link_label: "Chart interaction" },
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
                proof_cta_label: "Start with the platform model",
                proof_cta_href: "/docs/intro/",
                feature_label: "Platform shape",
                feature_title: "The product architecture stays together.",
                feature_body: "Fission is organized around one explicit app model and the host/lifecycle tools needed to take that model from first run to release.",
                features: OVERVIEW_FEATURES,
                details_label: "How it fits",
                details_title: "A framework boundary for the whole application lifecycle.",
                details_body: "The overview page connects the major surfaces. Use the deeper product pages when you need the details for a specific output or workflow.",
                details: OVERVIEW_DETAILS,
                workflow_label: "Lifecycle path",
                workflow_title: "From project setup to release receipts.",
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
                proof_cta_label: "Read target expectations",
                proof_cta_href: "/reference/platform/targets/",
                feature_label: "Host coverage",
                feature_title: "Each app target has a concrete responsibility.",
                feature_body: "Cross-platform does not mean pretending every host is the same. Fission keeps product behavior shared while validating the parts that belong to each platform.",
                features: CROSS_FEATURES,
                details_label: "Target model",
                details_title: "Use the fastest loop until the host itself matters.",
                details_body: "Desktop, web, Android, and iOS should all be tested as real outputs. The right target depends on the behavior you are proving.",
                details: CROSS_DETAILS,
                workflow_label: "Run loop",
                workflow_title: "Move between hosts without rewriting product code.",
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
                proof_body: "The Fission command UI is implemented as a Fission terminal app with routes, screens, reducers, dialogs, settings, and command sessions.",
                proof_cta_label: "Open terminal guide",
                proof_cta_href: "/docs/guides/terminal-user-interfaces/",
                feature_label: "Terminal product design",
                feature_title: "A terminal app is still an application.",
                feature_body: "The terminal shell is for structured workflows with screens, state, confirmation, progress, logs, settings, and non-blocking commands.",
                features: TERMINAL_FEATURES,
                details_label: "Terminal UX",
                details_title: "Progressive disclosure matters even in cells.",
                details_body: "Terminal UI should be compact and task-led, but it should still have the same product discipline as a graphical app.",
                details: TERMINAL_DETAILS,
                workflow_label: "Interaction path",
                workflow_title: "Navigate, run work, inspect logs, and change settings.",
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
                proof_cta_label: "Build a content site",
                proof_cta_href: "/docs/guides/static-sites/",
                feature_label: "Static site system",
                feature_title: "Static output without abandoning Fission widgets.",
                feature_body: "Use Fission components where design matters and Markdown content routes where documentation, references, blogs, and changelogs need to scale.",
                features: STATIC_FEATURES,
                details_label: "Site architecture",
                details_title: "Custom routes and content routes solve different jobs.",
                details_body: "A production static site needs designed pages, navigable content, metadata, assets, search, generated CSS, and publishing output.",
                details: STATIC_DETAILS,
                workflow_label: "Publishing path",
                workflow_title: "Design pages, author content, generate portable HTML.",
                workflow: STATIC_STEPS,
            },
            MarketingPageKind::ServerSites => PageCopy {
                eyebrow: "Server-rendered sites",
                title: "Render dynamic web products with the same Fission app model.",
                body: "Use the server shell for ecommerce, dashboards, portals, account pages, and other routes that need request-time data, sessions, signed actions, cache policy, workers, or focused islands.",
                primary_label: "Build a server site",
                primary_href: "/docs/guides/server-sites/",
                secondary_label: "Read server security",
                secondary_href: "/docs/guides/server-site-security/",
                proof_label: "Dynamic pages without a second app model",
                proof_body: "Server-rendered routes still use Fission widgets, jobs, services, reducers, generated assets, and deployment packaging instead of a parallel web stack.",
                proof_cta_label: "Build a server-rendered route",
                proof_cta_href: "/docs/guides/server-sites/",
                feature_label: "Server shell",
                feature_title: "Request-time HTML belongs in the Fission model too.",
                feature_body: "Server sites combine cached pages, private session pages, signed actions, workers, islands, and deployment packaging around the same Rust components.",
                features: SERVER_FEATURES,
                details_label: "Dynamic web model",
                details_title: "Choose the route mode based on the data boundary.",
                details_body: "Some pages should be static, some should be revalidated, and some must be rendered per request or per session. The server shell exists for that split.",
                details: SERVER_DETAILS,
                workflow_label: "Request path",
                workflow_title: "Model routes, prepare data, render HTML, package assets.",
                workflow: SERVER_STEPS,
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
                proof_cta_label: "Plan a release",
                proof_cta_href: "/docs/release-and-distribute/overview/",
                feature_label: "Release operations",
                feature_title: "The framework should help after the binary builds.",
                feature_body: "Packaging, signing, release notes, screenshots, credentials, stores, hosting, rollouts, and receipts should be explicit project workflows.",
                features: LIFECYCLE_FEATURES,
                details_label: "Post-build lifecycle",
                details_title: "Release readiness is checked before upload.",
                details_body: "Fission's release tooling aims to make missing metadata, bad artifacts, absent credentials, and provider-specific gaps visible before they block launch.",
                details: LIFECYCLE_DETAILS,
                workflow_label: "Release path",
                workflow_title: "Package artifacts, validate requirements, publish through providers.",
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
                proof_cta_label: "Open diagnostics docs",
                proof_cta_href: "/docs/test-and-debug/overview/",
                feature_label: "Observability",
                feature_title: "Developer tools should explain real runtime behavior.",
                feature_body: "The debugging story centers on inspected output, action traces, layout facts, resource calls, screenshots, device logs, and IDE integration.",
                features: DEVTOOLS_FEATURES,
                details_label: "Tooling direction",
                details_title: "Make the app legible to humans and tools.",
                details_body: "The goal is not a second programming model. The tools should reveal the Fission model already running in the app.",
                details: DEVTOOLS_DETAILS,
                workflow_label: "Debugging path",
                workflow_title: "Inspect output, trace behavior, fix before release.",
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
                proof_cta_label: "Generate a design system",
                proof_cta_href: "/docs/guides/design-system/",
                feature_label: "Design integration",
                feature_title: "Tokens and component states become typed UI input.",
                feature_body: "Fission treats a design system as code-generated product infrastructure, not a blob of JSON that widgets parse while rendering.",
                features: DESIGN_FEATURES,
                details_label: "Theme system",
                details_title: "Design data flows from package JSON to Env.",
                details_body: "The runtime should consume typed tokens for color, spacing, typography, components, charts, dark mode, and state-specific styling.",
                details: DESIGN_DETAILS,
                workflow_label: "Theme path",
                workflow_title: "Generate code, choose a theme, apply it everywhere.",
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
                proof_cta_label: "Browse chart families",
                proof_cta_href: "/docs/charts/catalog/",
                feature_label: "Visualization",
                feature_title: "Charts are product components, not decorative embeds.",
                feature_body: "The chart layer needs family breadth, dataset modeling, interaction, theming, animation, screenshots, and reference coverage.",
                features: CHART_FEATURES,
                details_label: "Chart architecture",
                details_title: "Start with the data story, then choose the visual form.",
                details_body: "Production charts need the right family, typed data, interaction behavior, theme integration, and tests that prove the rendered output.",
                details: CHART_DETAILS,
                workflow_label: "Chart path",
                workflow_title: "Choose a family, bind data, configure behavior, prove output.",
                workflow: CHART_STEPS,
            },
        }
    }
}

impl From<ProductMarketingPage> for Widget {
    fn from(component: ProductMarketingPage) -> Self {
        let (ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        let copy = component.kind.copy();
        Container::new(Column {
            children: vec![
                HomePageNav.into(),
                Row {
                    children: vec![Container::new(semantic_column(
                        "site-product-page",
                        vec![
                            marketing_hero(ctx, view, component.kind, copy),
                            product_nav_strip(ctx, view),
                            feature_showcase(view, copy),
                            detail_showcase(view, copy),
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
                    .into()],
                    justify_content: JustifyContent::Center,
                    ..Default::default()
                }
                .into(),
            ],
            gap: Some(0.0),
            flex_grow: 1.0,
            ..Default::default()
        })
        .min_height(tokens.spacing.xxxxl * 9.0)
        .bg_fill(page_fill(tokens))
        .into()
    }
}
fn marketing_hero(
    _ctx: BuildCtxHandle<DocsState>,
    view: ViewHandle<DocsState>,
    kind: MarketingPageKind,
    copy: PageCopy,
) -> Widget {
    let tokens = &view.env().theme.tokens;
    Container::new(semantic_row(
        "site-product-hero",
        vec![
            Container::new(Column {
                children: vec![
                    Pill::new(copy.eyebrow).into(),
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
                        .into(),
                    Text::new(copy.body)
                        .size(tokens.typography.font_size_lg)
                        .line_height(
                            tokens.typography.font_size_lg * tokens.typography.line_height_relaxed,
                        )
                        .color(tokens.colors.text_secondary)
                        .max_width(tokens.spacing.xxxxl * 5.2)
                        .flex_shrink(1.0)
                        .semantics_identifier("site-product-hero-body")
                        .into(),
                    semantic_row(
                        "site-product-hero-ctas",
                        vec![
                            Cta::new(copy.primary_label, copy.primary_href, true).into(),
                            Cta::new(copy.secondary_label, copy.secondary_href, false).into(),
                        ],
                        Some(tokens.spacing.m),
                        FlexWrap::Wrap,
                        AlignItems::Center,
                        JustifyContent::Start,
                    ),
                ],
                gap: Some(tokens.spacing.l),
                ..Default::default()
            })
            .width(tokens.spacing.xxxxl * 5.45)
            .flex_shrink(1.0)
            .into(),
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
    .into()
}

fn product_nav_strip(ctx: BuildCtxHandle<DocsState>, view: ViewHandle<DocsState>) -> Widget {
    let tokens = &view.env().theme.tokens;
    Container::new(Row {
        children: vec![
            strip_link(ctx, view, "Platform", "/product/overview/"),
            strip_link(ctx, view, "Apps", "/product/cross-platform-apps/"),
            strip_link(ctx, view, "Terminal", "/product/terminal-apps/"),
            strip_link(ctx, view, "Static sites", "/product/static-sites/"),
            strip_link(ctx, view, "Server sites", "/product/server-rendered-sites/"),
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
    })
    .padding_all(tokens.spacing.m)
    .bg_fill(Fill::Solid(tokens.colors.surface.with_alpha(232)))
    .border(tokens.colors.border, 1.0)
    .border_radius(tokens.radii.full)
    .into()
}

fn strip_link(
    _ctx: BuildCtxHandle<DocsState>,
    view: ViewHandle<DocsState>,
    label: &'static str,
    href: &'static str,
) -> Widget {
    let tokens = &view.env().theme.tokens;
    Container::new(NavLink::new(label, href))
        .padding([
            tokens.spacing.m,
            tokens.spacing.m,
            tokens.spacing.s,
            tokens.spacing.s,
        ])
        .bg_fill(Fill::Solid(tokens.colors.surface_raised))
        .border(tokens.colors.border, 1.0)
        .border_radius(tokens.radii.full)
        .into()
}

fn feature_showcase(view: ViewHandle<DocsState>, copy: PageCopy) -> Widget {
    let tokens = &view.env().theme.tokens;
    semantic_row(
        "site-product-feature-showcase",
        vec![
            Column {
                children: vec![
                    Text::new(copy.feature_label)
                        .size(tokens.typography.font_size_sm)
                        .weight(tokens.typography.font_weight_bold)
                        .color(tokens.colors.secondary)
                        .into(),
                    Text::new(copy.feature_title)
                        .size(tokens.typography.heading2_size)
                        .family(tokens.typography.font_family_serif.clone())
                        .line_height(
                            tokens.typography.heading2_size * tokens.typography.line_height_heading,
                        )
                        .weight(tokens.typography.font_weight_bold)
                        .color(tokens.colors.heading)
                        .into(),
                    Text::new(copy.feature_body)
                        .size(tokens.typography.body_large_size)
                        .line_height(
                            tokens.typography.body_large_size
                                * tokens.typography.line_height_relaxed,
                        )
                        .color(tokens.colors.text_secondary)
                        .into(),
                ],
                gap: Some(tokens.spacing.m),
                flex_grow: 1.0,
                ..Default::default()
            }
            .into(),
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
            .into(),
        ],
        Some(tokens.spacing.xxl),
        FlexWrap::Wrap,
        AlignItems::Stretch,
        JustifyContent::SpaceBetween,
    )
}

fn detail_showcase(view: ViewHandle<DocsState>, copy: PageCopy) -> Widget {
    let tokens = &view.env().theme.tokens;
    Container::new(Column {
        children: vec![
            Column {
                children: vec![
                    Text::new(copy.details_label)
                        .size(tokens.typography.font_size_sm)
                        .weight(tokens.typography.font_weight_bold)
                        .color(tokens.colors.primary)
                        .into(),
                    Text::new(copy.details_title)
                        .size(tokens.typography.heading_size)
                        .family(tokens.typography.font_family_serif.clone())
                        .line_height(
                            tokens.typography.heading_size * tokens.typography.line_height_heading,
                        )
                        .weight(tokens.typography.font_weight_bold)
                        .color(tokens.colors.heading)
                        .into(),
                    Text::new(copy.details_body)
                        .size(tokens.typography.body_large_size)
                        .line_height(
                            tokens.typography.body_large_size
                                * tokens.typography.line_height_relaxed,
                        )
                        .color(tokens.colors.text_secondary)
                        .max_width(tokens.spacing.xxxxl * 6.2)
                        .flex_shrink(1.0)
                        .into(),
                ],
                gap: Some(tokens.spacing.m),
                ..Default::default()
            }
            .into(),
            Row {
                children: copy
                    .details
                    .iter()
                    .map(|detail| detail_card(view, *detail))
                    .collect(),
                gap: Some(tokens.spacing.m),
                wrap: FlexWrap::Wrap,
                align_items: AlignItems::Stretch,
                justify_content: JustifyContent::SpaceBetween,
                ..Default::default()
            }
            .into(),
        ],
        gap: Some(tokens.spacing.l),
        ..Default::default()
    })
    .padding_all(tokens.spacing.xl)
    .bg_fill(Fill::LinearGradient {
        start: (0.0, 0.0),
        end: (1.0, 1.0),
        stops: vec![
            (0.0, tokens.colors.surface.with_alpha(246)),
            (1.0, tokens.colors.surface_sunken.with_alpha(242)),
        ],
    })
    .border(tokens.colors.border, 1.0)
    .border_radius(tokens.radii.xxl)
    .into()
}

fn detail_card(view: ViewHandle<DocsState>, detail: DetailCopy) -> Widget {
    let tokens = &view.env().theme.tokens;
    Container::new(Column {
        children: vec![
            Text::new(detail.label)
                .size(tokens.typography.font_size_xs)
                .family(tokens.typography.font_family_mono.clone())
                .weight(tokens.typography.font_weight_bold)
                .color(tokens.colors.secondary)
                .into(),
            Text::new(detail.title)
                .size(tokens.typography.font_size_lg)
                .weight(tokens.typography.font_weight_bold)
                .color(tokens.colors.heading)
                .into(),
            Text::new(detail.body)
                .size(tokens.typography.body_medium_size)
                .line_height(
                    tokens.typography.body_medium_size * tokens.typography.line_height_relaxed,
                )
                .color(tokens.colors.text_secondary)
                .flex_shrink(1.0)
                .into(),
            NavLink::new(detail.link_label, detail.href).into(),
        ],
        gap: Some(tokens.spacing.m),
        ..Default::default()
    })
    .width(tokens.spacing.xxxxl * 3.25)
    .min_height(tokens.spacing.xxxxl * 2.35)
    .flex_shrink(1.0)
    .padding_all(tokens.spacing.l)
    .bg_fill(Fill::Solid(tokens.colors.surface_raised))
    .border(tokens.colors.border, 1.0)
    .border_radius(tokens.radii.xl)
    .into()
}

fn feature_card(view: ViewHandle<DocsState>, feature: FeatureCopy) -> Widget {
    let tokens = &view.env().theme.tokens;
    Container::new(Column {
        children: vec![
            Text::new(feature.label)
                .size(tokens.typography.font_size_xs)
                .family(tokens.typography.font_family_mono.clone())
                .weight(tokens.typography.font_weight_bold)
                .color(tokens.colors.primary)
                .into(),
            Text::new(feature.title)
                .size(tokens.typography.heading_size)
                .weight(tokens.typography.font_weight_bold)
                .color(tokens.colors.heading)
                .into(),
            Text::new(feature.body)
                .size(tokens.typography.body_medium_size)
                .line_height(
                    tokens.typography.body_medium_size * tokens.typography.line_height_relaxed,
                )
                .color(tokens.colors.text_secondary)
                .flex_shrink(1.0)
                .into(),
        ],
        gap: Some(tokens.spacing.m),
        ..Default::default()
    })
    .width(tokens.spacing.xxxxl * 3.1)
    .min_height(tokens.spacing.xxxxl * 2.05)
    .flex_shrink(1.0)
    .padding_all(tokens.spacing.l)
    .bg_fill(Fill::Solid(tokens.colors.surface_raised))
    .border(tokens.colors.border, 1.0)
    .border_radius(tokens.radii.xl)
    .into()
}

fn workflow_showcase(view: ViewHandle<DocsState>, copy: PageCopy) -> Widget {
    let tokens = &view.env().theme.tokens;
    Container::new(Column {
        children: vec![
            Row {
                children: vec![
                    Text::new(copy.workflow_label)
                        .size(tokens.typography.font_size_sm)
                        .weight(tokens.typography.font_weight_bold)
                        .color(tokens.colors.primary)
                        .into(),
                    Text::new(copy.workflow_title)
                        .size(tokens.typography.heading_size)
                        .weight(tokens.typography.font_weight_bold)
                        .color(tokens.colors.heading)
                        .into(),
                ],
                gap: Some(tokens.spacing.l),
                wrap: FlexWrap::Wrap,
                align_items: AlignItems::Center,
                ..Default::default()
            }
            .into(),
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
            .into(),
        ],
        gap: Some(tokens.spacing.l),
        ..Default::default()
    })
    .padding_all(tokens.spacing.xl)
    .bg_fill(Fill::Solid(tokens.colors.surface))
    .border(tokens.colors.border, 1.0)
    .border_radius(tokens.radii.xxl)
    .into()
}

fn workflow_step(view: ViewHandle<DocsState>, index: usize, step: StepCopy) -> Widget {
    let tokens = &view.env().theme.tokens;
    Container::new(Column {
        children: vec![
            Text::new(format!("{:02}", index))
                .size(tokens.typography.font_size_xs)
                .family(tokens.typography.font_family_mono.clone())
                .weight(tokens.typography.font_weight_bold)
                .color(tokens.colors.primary)
                .into(),
            Text::new(step.label)
                .size(tokens.typography.font_size_lg)
                .weight(tokens.typography.font_weight_bold)
                .color(tokens.colors.heading)
                .into(),
            Text::new(step.body)
                .size(tokens.typography.font_size_sm)
                .line_height(tokens.typography.font_size_sm * tokens.typography.line_height_normal)
                .color(tokens.colors.text_secondary)
                .into(),
        ],
        gap: Some(tokens.spacing.s),
        ..Default::default()
    })
    .width(tokens.spacing.xxxxl * 3.05)
    .min_height(tokens.spacing.xxxxl * 1.35)
    .flex_shrink(1.0)
    .padding_all(tokens.spacing.l)
    .bg_fill(Fill::Solid(tokens.colors.surface_raised))
    .border(tokens.colors.border, 1.0)
    .border_radius(tokens.radii.large)
    .into()
}

fn proof_band(
    _ctx: BuildCtxHandle<DocsState>,
    view: ViewHandle<DocsState>,
    copy: PageCopy,
) -> Widget {
    let tokens = &view.env().theme.tokens;
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
                        .into(),
                    Text::new(copy.proof_body)
                        .size(tokens.typography.body_large_size)
                        .line_height(
                            tokens.typography.body_large_size
                                * tokens.typography.line_height_relaxed,
                        )
                        .color(tokens.colors.text_secondary)
                        .into(),
                ],
                gap: Some(tokens.spacing.m),
                flex_grow: 1.0,
                ..Default::default()
            }
            .into(),
            Cta::new(copy.proof_cta_label, copy.proof_cta_href, true).into(),
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
    .into()
}

fn product_visual(view: ViewHandle<DocsState>, kind: MarketingPageKind) -> Widget {
    let tokens = &view.env().theme.tokens;
    let children = match kind {
        MarketingPageKind::Charts => chart_visual(view),
        MarketingPageKind::TerminalApps => terminal_visual(view),
        MarketingPageKind::StaticSites => site_visual(view),
        MarketingPageKind::ServerSites => server_visual(view),
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
        .into()
}

fn platform_visual(view: ViewHandle<DocsState>) -> Widget {
    visual_stack(
        view,
        "Platform map",
        &[
            ("App model", "State / reducers / widgets"),
            ("Targets", "Desktop / Web / Mobile / TUI / Site / Server"),
            ("Lifecycle", "Package / sign / release / receipts"),
        ],
    )
}

fn target_visual(view: ViewHandle<DocsState>) -> Widget {
    visual_stack(
        view,
        "Target matrix",
        &[
            ("Desktop", "Windows  macOS  Linux"),
            ("Mobile", "Android  iOS"),
            ("Web", "WASM browser shell"),
            ("Specialized", "Terminal UI  Static HTML  Server HTML"),
        ],
    )
}

fn server_visual(view: ViewHandle<DocsState>) -> Widget {
    visual_stack(
        view,
        "Server site",
        &[
            ("Route", "ServerPrivate / Revalidated"),
            ("Data", "jobs  cache  sessions"),
            ("Actions", "signed reducer dispatch"),
            ("Browser", "worker  island  assets"),
        ],
    )
}

fn terminal_visual(view: ViewHandle<DocsState>) -> Widget {
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

fn site_visual(view: ViewHandle<DocsState>) -> Widget {
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

fn lifecycle_visual(view: ViewHandle<DocsState>) -> Widget {
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

fn devtools_visual(view: ViewHandle<DocsState>) -> Widget {
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

fn design_visual(view: ViewHandle<DocsState>) -> Widget {
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

fn chart_visual(view: ViewHandle<DocsState>) -> Widget {
    let tokens = &view.env().theme.tokens;
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
            .into(),
            Row {
                children: vec![
                    chart_thumb(view, "/img/charts/sankey-energy.png"),
                    chart_thumb(view, "/img/charts/surface3d-wave.png"),
                ],
                gap: Some(tokens.spacing.s),
                wrap: FlexWrap::Wrap,
                ..Default::default()
            }
            .into(),
        ],
        gap: Some(tokens.spacing.m),
        ..Default::default()
    }
    .into()
}

fn visual_stack(
    view: ViewHandle<DocsState>,
    title: &'static str,
    rows: &[(&'static str, &'static str)],
) -> Widget {
    let tokens = &view.env().theme.tokens;
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
    .into()
}

fn visual_header(view: ViewHandle<DocsState>, title: &'static str) -> Widget {
    let tokens = &view.env().theme.tokens;
    Row {
        children: vec![
            dot(tokens.colors.error),
            dot(tokens.colors.warning),
            dot(tokens.colors.success),
            Text::new(title)
                .size(tokens.typography.font_size_sm)
                .family(tokens.typography.font_family_mono.clone())
                .color(tokens.colors.text_secondary)
                .into(),
        ],
        gap: Some(tokens.spacing.s),
        align_items: AlignItems::Center,
        ..Default::default()
    }
    .into()
}

fn visual_row(view: ViewHandle<DocsState>, label: &'static str, body: &'static str) -> Widget {
    let tokens = &view.env().theme.tokens;
    Container::new(Row {
        children: vec![
            Text::new(label)
                .size(tokens.typography.font_size_sm)
                .weight(tokens.typography.font_weight_bold)
                .color(tokens.colors.heading)
                .into(),
            Text::new(body)
                .size(tokens.typography.font_size_sm)
                .family(tokens.typography.font_family_mono.clone())
                .color(tokens.colors.text_secondary)
                .into(),
        ],
        gap: Some(tokens.spacing.m),
        wrap: FlexWrap::Wrap,
        justify_content: JustifyContent::SpaceBetween,
        ..Default::default()
    })
    .padding_all(tokens.spacing.m)
    .bg_fill(Fill::Solid(tokens.colors.surface))
    .border(tokens.colors.border, 1.0)
    .border_radius(tokens.radii.large)
    .into()
}

fn chart_thumb(view: ViewHandle<DocsState>, src: &'static str) -> Widget {
    let tokens = &view.env().theme.tokens;
    Container::new(Image::asset(src).size(tokens.spacing.xxxxl * 1.85, tokens.spacing.xxxxl * 1.05))
        .padding_all(tokens.spacing.xs)
        .bg_fill(Fill::Solid(tokens.colors.on_surface.with_alpha(245)))
        .border_radius(tokens.radii.large)
        .into()
}

fn dot(color: Color) -> Widget {
    Container::new(Text::new(" "))
        .width(9.0)
        .height(9.0)
        .bg_fill(Fill::Solid(color))
        .border_radius(99.0)
        .into()
}
