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
            title: "One foundation. Less repeated work.",
            lead: "Tangible benefits for developers, teams and organisations.",
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
                    "Plain Rust",
                    "Plain Rust in. Native apps out.",
                    "State is a struct, updates are typed reducers and the interface is a value built from them. When the state changes, Fission rebuilds the view and updates only what changed.",
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
            eyebrow: "How it works",
            title: "Different targets. One way of working.",
            lead: "Write your app once as plain Rust, then run it on every surface with the fission command.",
            align: HeadingAlign::Center,
            tinted: false,
            content: vec![targets.into()],
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
            title: "See what it builds",
            lead: "Every screenshot is a checked-in example you can run with cargo run.",
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
            eyebrow: "Production",
            title: "Built for real delivery",
            lead: "What it takes to ship and maintain high-quality applications on every target.",
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
                    "Start in a minute",
                    "If you have Rust installed, four commands give you a running app. New to Rust? The quickstart walks through installing the toolchain first.",
                    false,
                ),
                arrow_link("Open the quickstart  →", "/docs/learn/quickstart/"),
                arrow_link("Browse the examples  →", "/docs/learn/examples-and-targets/"),
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
            title: "Ready to build without limits?",
            body: "Install the fission command and have an app running in a minute.",
            primary: ("Get started  →", "/docs/learn/quickstart/"),
            secondary: Some(("Read the docs", "/docs/")),
        }
        .into()
    }
}
