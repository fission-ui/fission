use super::home_nav::HomePageNav;
use super::home_widgets::{
    content_width, page_fill, Card, CenteredSection, Chip, CodeCard, Cta, ExternalNavLink,
    LinkCard, NavLink, Pill, SectionHeader, SemanticColumn, SemanticRow,
};
use super::state::DocsState;
use fission::op::{AlignItems, Fill, FlexWrap, JustifyContent};
use fission::prelude::*;

#[derive(Clone, Debug)]
pub(crate) struct ExampleShowcasePage;

impl ExampleShowcasePage {
    pub(crate) fn new() -> Self {
        Self
    }
}

impl From<ExampleShowcasePage> for Widget {
    fn from(_page: ExampleShowcasePage) -> Widget {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;

        Container::new(Column {
            children: vec![
                HomePageNav.into(),
                Row {
                    children: vec![Container::new(SemanticColumn::new(
                        "site-example-showcase",
                        vec![
                            ShowcaseHero.into(),
                            ShowcaseGallery.into(),
                            MoreExamples.into(),
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
            gap: Some(tokens.spacing.none),
            flex_grow: 1.0,
            ..Default::default()
        })
        .min_height(tokens.spacing.xxxxl * 9.0)
        .bg_fill(page_fill(tokens))
        .into()
    }
}

#[derive(Clone, Copy, Debug)]
struct ShowcaseHero;

impl From<ShowcaseHero> for Widget {
    fn from(_hero: ShowcaseHero) -> Widget {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;

        Container::new(SemanticColumn::new(
            "site-example-showcase-hero",
            vec![
                Pill::new("Example atlas").into(),
                Text::new("Explore Fission through working products.")
                    .size(tokens.typography.display_md_size)
                    .family(tokens.typography.font_family_serif.clone())
                    .line_height(
                        tokens.typography.display_md_size * tokens.typography.line_height_display,
                    )
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.heading)
                    .max_width(tokens.spacing.xxxxl * 8.0)
                    .flex_shrink(1.0)
                    .into(),
                Text::new("Start with a complete app, inspect the source, then branch into the gallery or target that answers your next engineering question.")
                    .size(tokens.typography.body_large_size)
                    .line_height(
                        tokens.typography.body_large_size * tokens.typography.line_height_relaxed,
                    )
                    .color(tokens.colors.text_secondary)
                    .max_width(tokens.spacing.xxxxl * 8.0)
                    .flex_shrink(1.0)
                    .into(),
                SemanticRow::new(
                    "site-example-showcase-hero-actions",
                    vec![
                        Cta::new(
                            "Choose your first example",
                            "/docs/learn/examples-and-targets/",
                            true,
                        )
                        .into(),
                        ExternalNavLink::new(
                            "Browse all source ->",
                            "https://github.com/fission-ui/fission/tree/main/examples",
                        )
                        .into(),
                    ],
                    Some(tokens.spacing.m),
                    FlexWrap::Wrap,
                    AlignItems::Center,
                    JustifyContent::Start,
                )
                .into(),
                Row {
                    children: [
                        "macOS", "Windows", "Linux", "Web", "Android", "iOS", "Terminal",
                        "SSR",
                    ]
                    .into_iter()
                    .map(|label| Chip::new(label).into())
                    .collect(),
                    gap: Some(tokens.spacing.s),
                    wrap: FlexWrap::Wrap,
                    ..Default::default()
                }
                .into(),
            ],
            Some(tokens.spacing.l),
            AlignItems::Start,
        ))
        .padding([tokens.spacing.xxl, 0.0, tokens.spacing.xxl, 0.0])
        .into()
    }
}

#[derive(Clone, Copy, Debug)]
struct ExampleCopy {
    kind: &'static str,
    title: &'static str,
    image: &'static str,
    alt: &'static str,
    body: &'static str,
    command: &'static str,
    targets: &'static [&'static str],
    guide: &'static str,
    source: &'static str,
}

const FEATURED: &[ExampleCopy] = &[
    ExampleCopy {
        kind: "Product app",
        title: "Inbox",
        image: "/img/examples/inbox-initial.png",
        alt: "Fission Inbox with message list and reading pane",
        body: "Responsive panes, selection, search, compose flows, and realistic mail-product structure.",
        command: "cargo run -p inbox",
        targets: &["macOS", "Windows", "Linux"],
        guide: "/docs/guides/layout-and-widgets/",
        source: "https://github.com/fission-ui/fission/tree/main/examples/inbox",
    },
    ExampleCopy {
        kind: "Product app",
        title: "Fission Editor",
        image: "/img/examples/editor-terminal.png",
        alt: "Fission Editor with file tree, source editor, and integrated terminal",
        body: "A product-shaped editor with files, syntax, language services, terminal work, and LiveTest.",
        command: "cargo run -p fission-editor",
        targets: &["macOS", "Windows", "Linux"],
        guide: "/docs/guides/resources-and-async/",
        source: "https://github.com/fission-ui/fission/tree/main/examples/editor",
    },
    ExampleCopy {
        kind: "Gallery",
        title: "Widget Gallery",
        image: "/img/examples/widget-gallery.png",
        alt: "Fission Widget Gallery showing standard controls",
        body: "Tour the standard widget library, layout primitives, states, and interaction behavior.",
        command: "cargo run -p widget-gallery",
        targets: &["macOS", "Windows", "Linux"],
        guide: "/docs/guides/layout-and-widgets/",
        source: "https://github.com/fission-ui/fission/tree/main/examples/widget-gallery",
    },
    ExampleCopy {
        kind: "Workbench",
        title: "Text Lab",
        image: "/img/examples/text-lab.png",
        alt: "Fission Text Lab with text editing and diagnostic controls",
        body: "Probe text shaping, selection, input, focus, menus, modal flows, and diagnostics.",
        command: "cargo run -p text-lab",
        targets: &["macOS", "Windows", "Linux"],
        guide: "/docs/guides/input-events-text-and-env/",
        source: "https://github.com/fission-ui/fission/tree/main/examples/text-lab",
    },
    ExampleCopy {
        kind: "Platform",
        title: "Terminal UI",
        image: "/img/examples/terminal-ui.png",
        alt: "Fission terminal interface with navigation and command output",
        body: "Use the same app model for keyboard-first screens, dialogs, command sessions, and logs.",
        command: "cargo run -p terminal",
        targets: &["Terminal"],
        guide: "/docs/guides/terminal-user-interfaces/",
        source: "https://github.com/fission-ui/fission/tree/main/examples/terminal",
    },
];

#[derive(Clone, Copy, Debug)]
struct ShowcaseGallery;

impl From<ShowcaseGallery> for Widget {
    fn from(_gallery: ShowcaseGallery) -> Widget {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;

        SemanticColumn::new(
            "site-example-showcase-gallery",
            vec![
                SectionHeader::new(
                    "Live examples",
                    "Product-shaped proofs, not isolated screenshots.",
                    "Every featured example is checked into the repository with a direct run command, source, and a guide to the ideas it demonstrates.",
                )
                .into(),
                Row {
                    children: FEATURED
                        .iter()
                        .copied()
                        .map(|example| ShowcaseCard { example }.into())
                        .collect(),
                    gap: Some(tokens.spacing.m),
                    wrap: FlexWrap::Wrap,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Stretch,
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

#[derive(Clone, Copy, Debug)]
struct ShowcaseCard {
    example: ExampleCopy,
}

impl From<ShowcaseCard> for Widget {
    fn from(card: ShowcaseCard) -> Widget {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        let width = tokens.spacing.xxxxl * 3.5;
        let image_width = width - tokens.spacing.xl * 2.0;

        Card::new(
            vec![
                Container::new(
                    Image::asset(card.example.image)
                        .size(image_width, image_width * 0.75)
                        .semantic_label(card.example.alt),
                )
                .padding_all(tokens.spacing.xs)
                .bg_fill(Fill::Solid(tokens.colors.surface))
                .border(tokens.colors.border, 1.0)
                .border_radius(tokens.radii.large)
                .into(),
                Text::new(card.example.kind)
                    .size(tokens.typography.font_size_xs)
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.primary)
                    .into(),
                Text::new(card.example.title)
                    .size(tokens.typography.heading_size)
                    .family(tokens.typography.font_family_serif.clone())
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.heading)
                    .into(),
                Text::new(card.example.body)
                    .size(tokens.typography.body_medium_size)
                    .line_height(
                        tokens.typography.body_medium_size * tokens.typography.line_height_normal,
                    )
                    .color(tokens.colors.text_secondary)
                    .flex_shrink(1.0)
                    .into(),
                Row {
                    children: card
                        .example
                        .targets
                        .iter()
                        .map(|target| Chip::new(*target).into())
                        .collect(),
                    gap: Some(tokens.spacing.xs),
                    wrap: FlexWrap::Wrap,
                    ..Default::default()
                }
                .into(),
                CodeCard::new("Run", card.example.command).into(),
                SemanticRow::new(
                    format!("site-example-links:{}", card.example.title),
                    vec![
                        NavLink::new("Read the guide ->", card.example.guide).into(),
                        ExternalNavLink::new("Source ->", card.example.source).into(),
                    ],
                    Some(tokens.spacing.m),
                    FlexWrap::Wrap,
                    AlignItems::Center,
                    JustifyContent::SpaceBetween,
                )
                .into(),
            ],
            width,
        )
        .into()
    }
}

#[derive(Clone, Copy, Debug)]
struct MoreExamples;

impl From<MoreExamples> for Widget {
    fn from(_more: MoreExamples) -> Widget {
        CenteredSection::new(
            "Choose by question",
            "The rest of the repository covers the full platform atlas.",
            "Pick the smallest example that proves the next behavior you need, then move to a real target host when the host becomes part of the question.",
            vec![
                LinkCard::new("Start", "Counter", "State, reducers, actions, and a complete app loop in one small project.", "Build the counter ->", "/docs/cookbook/build-a-counter/").into(),
                LinkCard::new("Motion", "Animation Gallery", "Compose, scrub, inspect, and test explicit animation tracks.", "Explore animation ->", "/docs/guides/media-animation-portals-and-3d/").into(),
                LinkCard::new("Data", "Chart Gallery", "Browse chart families, interaction patterns, datasets, and native rendering.", "Browse chart families ->", "/docs/charts/catalog/").into(),
                LinkCard::new("Devices", "Field Inspector", "Inspect platform capabilities across desktop, browser, Android, and iOS hosts.", "Platform capabilities ->", "/docs/guides/platform-capabilities/").into(),
                LinkCard::new("SSR", "Pokemon Card Store", "Follow sessions, jobs, signed actions, route caching, workers, and islands.", "Study the server app ->", "/docs/guides/server-sites/").into(),
            ],
        )
        .into()
    }
}
