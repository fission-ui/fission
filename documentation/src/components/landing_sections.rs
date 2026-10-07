//! The home page sections below the hero. They are composed from the page kit: open feature
//! columns, text beside real code or screenshots, and a tinted closing call to action.

use super::home_widgets::site_semantics;
use super::page_kit::{
    arrow_link, code_block, feature_columns, heading_block, screenshot, split, CallToAction,
    Feature, HeadingAlign, Section,
};
use super::state::DocsState;
use fission::op::{AlignItems, TextAlign};
use fission::prelude::*;

const WIDGET_SNIPPET: &str = "#[fission_reducer(Increment)]
fn on_increment(state: &mut CounterState) {
    state.count += 1;
}

impl From<CounterApp> for Widget {
    fn from(_: CounterApp) -> Self {
        let (ctx, view) = fission::build::current::<CounterState>();
        Column {
            children: vec![
                Text::new(format!(\"Count: {}\", view.state().count)).into(),
                Button {
                    on_press: Some(with_reducer!(ctx, Increment, on_increment)),
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

const START_COMMANDS: &str = "cargo install cargo-fission
fission init my-app
cd my-app
fission run

# the same app on other targets
fission add-target web android ios
fission run --target web";

const FOUNDATION: [Feature; 3] = [
    Feature {
        icon: material::content::bolt::regular,
        title: "Move faster",
        body: "Share application logic, state and interface across every target instead of rebuilding them for each platform.",
    },
    Feature {
        icon: material::image::palette::regular,
        title: "Stay consistent",
        body: "One widget set and one set of design tokens keep behaviour and look aligned as the product grows.",
    },
    Feature {
        icon: material::maps::layers::regular,
        title: "Own the stack",
        body: "Plain Rust types, portable primitives and explicit escape hatches when a platform needs something special.",
    },
];

const DELIVERY: [Feature; 4] = [
    Feature {
        icon: material::communication::hub::regular,
        title: "Shared state and UI",
        body: "One codebase with the same behaviour on every target.",
    },
    Feature {
        icon: material::device::devices::regular,
        title: "Native integration",
        body: "Notifications, biometrics, camera and deep links where the platform allows.",
    },
    Feature {
        icon: material::action::accessibility_new::regular,
        title: "Accessible by default",
        body: "Semantics, keyboard navigation and focus handling are built in.",
    },
    Feature {
        icon: material::action::rocket_launch::regular,
        title: "Tested and packaged",
        body: "Drive real apps in tests, then package and publish with one command.",
    },
];

#[derive(Clone, Debug)]
pub(super) struct FoundationSection;

impl From<FoundationSection> for Widget {
    fn from(_section: FoundationSection) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        Section {
            identifier: "site-home-foundation",
            anchor: Some("why"),
            eyebrow: "Why Fission",
            title: "A Rust app framework built for shipping.",
            lead: "Share the work that should be shared, keep platform-specific control where it matters, and take the same product from development to release.",
            align: HeadingAlign::Center,
            tinted: false,
            content: vec![feature_columns(tokens, "site-kit-columns", &FOUNDATION)],
        }
        .into()
    }
}

#[derive(Clone, Debug)]
pub(super) struct WriteOnceSection;

impl From<WriteOnceSection> for Widget {
    fn from(_section: WriteOnceSection) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        let text = Column {
            children: vec![
                heading_block(
                    tokens,
                    None,
                    "Rust UI development",
                    "Build the interface and app logic in Rust.",
                    "Define state with Rust types, update it with typed reducers and compose the interface from retained widgets. Fission updates the rendered app when state changes without requiring a JavaScript frontend or a separate UI language.",
                    false,
                ),
                arrow_link("Follow the quickstart  →", "/docs/learn/quickstart/"),
                arrow_link("How the runtime works  →", "/docs/learn/runtime-model/"),
            ],
            gap: Some(tokens.spacing.l),
            ..Default::default()
        };
        Section {
            identifier: "site-home-write-once",
            anchor: None,
            eyebrow: "",
            title: "",
            lead: "",
            align: HeadingAlign::Start,
            tinted: true,
            content: vec![split(
                tokens,
                "site-kit-split",
                text.into(),
                code_block(tokens, "site-home-code:counter", WIDGET_SNIPPET),
            )],
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
        let targets = Row {
            children: TARGETS
                .iter()
                .map(|target| {
                    Column {
                        children: vec![
                            Icon::svg((target.icon)())
                                .size(tokens.spacing.xl)
                                .color(tokens.colors.primary)
                                .into(),
                            Text::new(target.label)
                                .size(tokens.typography.font_size_sm)
                                .weight(tokens.typography.font_weight_medium)
                                .color(tokens.colors.text_primary)
                                .text_align(TextAlign::Center)
                                .into(),
                        ],
                        gap: Some(tokens.spacing.s),
                        align_items: AlignItems::Center,
                        ..Default::default()
                    }
                    .into()
                })
                .collect(),
            semantics: Some(site_semantics("site-kit-targets")),
            ..Default::default()
        };
        Section {
            identifier: "site-home-targets",
            anchor: Some("how"),
            eyebrow: "Supported platforms",
            title: "One Rust codebase for desktop, mobile and web.",
            lead: "Build for macOS, Windows, Linux, Android, iOS and Web with shared Rust UI and application code. The same framework also supports terminal apps, static sites and server rendering.",
            align: HeadingAlign::Center,
            tinted: false,
            content: vec![
                targets.into(),
                arrow_link(
                    "Explore cross-platform Rust app development  →",
                    "/product/cross-platform-apps/",
                ),
            ],
        }
        .into()
    }
}

#[derive(Clone, Debug)]
pub(super) struct GallerySection;

impl From<GallerySection> for Widget {
    fn from(_section: GallerySection) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        let shots = Row {
            children: vec![
                screenshot(
                    tokens,
                    "/img/examples/editor.png",
                    368.0,
                    230.0,
                    "A code editor with a file tree, tabs and a terminal",
                ),
                screenshot(
                    tokens,
                    "/img/examples/chart-gallery.png",
                    368.0,
                    230.0,
                    "Line, bar, map, hierarchy and 3D charts",
                ),
                screenshot(
                    tokens,
                    "/img/examples/widget-gallery.png",
                    368.0,
                    230.0,
                    "Every built-in widget, live",
                ),
            ],
            semantics: Some(site_semantics("site-kit-gallery")),
            ..Default::default()
        };
        Section {
            identifier: "site-home-gallery",
            anchor: Some("examples"),
            eyebrow: "Examples",
            title: "See real Fission apps and Rust UI examples.",
            lead: "Every screenshot comes from a checked-in example you can run, inspect and adapt for your own application.",
            align: HeadingAlign::Center,
            tinted: true,
            content: vec![shots.into()],
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
        Section {
            identifier: "site-home-delivery",
            anchor: None,
            eyebrow: "From UI to release",
            title: "More than a Rust GUI toolkit.",
            lead: "Fission combines the UI framework with accessibility, platform integration, testing and packaging so teams can finish and ship the application.",
            align: HeadingAlign::Center,
            tinted: false,
            content: vec![feature_columns(tokens, "site-kit-columns", &DELIVERY)],
        }
        .into()
    }
}

#[derive(Clone, Debug)]
pub(super) struct StartSection;

impl From<StartSection> for Widget {
    fn from(_section: StartSection) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        let text = Column {
            children: vec![
                heading_block(
                    tokens,
                    Some("start"),
                    "Get started",
                    "Build your first Rust app in minutes.",
                    "If you have Rust installed, four commands create and run a Fission app. The quickstart also covers the toolchain and adding Web, Android and iOS targets.",
                    false,
                ),
                arrow_link(
                    "Follow the Rust app quickstart  →",
                    "/docs/learn/quickstart/",
                ),
                arrow_link(
                    "Browse runnable Fission examples  →",
                    "/docs/learn/examples-and-targets/",
                ),
            ],
            gap: Some(tokens.spacing.l),
            ..Default::default()
        };
        Section {
            identifier: "site-home-start",
            anchor: None,
            eyebrow: "",
            title: "",
            lead: "",
            align: HeadingAlign::Start,
            tinted: false,
            content: vec![split(
                tokens,
                "site-kit-split",
                text.into(),
                code_block(tokens, "site-home-code:start", START_COMMANDS),
            )],
        }
        .into()
    }
}

#[derive(Clone, Debug)]
pub(super) struct CtaBand;

impl From<CtaBand> for Widget {
    fn from(_band: CtaBand) -> Self {
        CallToAction {
            title: "Ready to build an app in Rust?",
            body: "Install the Fission CLI and run your first desktop app, then add Web, Android or iOS when you need them.",
            primary: ("Build your first Rust app  →", "/docs/learn/quickstart/"),
            secondary: Some(("Explore the Fission docs", "/docs/")),
        }
        .into()
    }
}
