use fission::core::ui::{
    BadgeTone, ContextMenu, ContextMenuEntry, ContextMenuItem, ContextMenuRegion, Image,
};
use fission::op::{BoxAlignment, ImageFit};
use fission::prelude::*;
use fission::widgets::divider::Orientation;
use fission::widgets::{
    AspectRatio, Avatar, AvatarGroup, AvatarGroupItem, Badge, Code, Divider, HStack, Kbd,
    MarkdownContent, Stat, Tag, VStack, Wrap,
};

const AVATAR_SIZE: f32 = 36.0;
const AVATAR_GROUP_VISIBLE: usize = 3;
const BORDER_WIDTH: f32 = 1.0;
const DIVIDER_DASH: f32 = 4.0;
const WIDE_RATIO_WIDTH: f32 = 320.0;
const SQUARE_RATIO_WIDTH: f32 = 160.0;
const IMAGE_WIDTH: f32 = 240.0;
const IMAGE_HEIGHT: f32 = 160.0;
const TEAM: &[(&str, &str)] = &[
    ("ada", "Ada Lovelace"),
    ("grace", "Grace Hopper"),
    ("alan", "Alan Turing"),
    ("katherine", "Katherine Johnson"),
    ("linus", "Linus Torvalds"),
];
const BADGE_TONES: &[(BadgeTone, &str)] = &[
    (BadgeTone::Brand, "New"),
    (BadgeTone::Gray, "Draft"),
    (BadgeTone::Success, "Paid"),
    (BadgeTone::Warning, "Pending"),
    (BadgeTone::Error, "Overdue"),
    (BadgeTone::Blue, "Info"),
    (BadgeTone::Orange, "Beta"),
];
const MARKDOWN_SAMPLE: &str = "# Release notes\n\n\
Version **2.4** makes sharing faster and fixes a few rough edges.\n\n\
## What's new\n\n\
- Share a page with a link\n\
- Pick a *date range* when filtering\n\
- Rename documents in place\n\n\
> Tip: press `Ctrl+K` to search from anywhere.\n\n\
Read the [full changelog](https://example.com/changelog) for details.\n\n\
```rust\nlet total = items.iter().sum::<u32>();\n```\n";

pub(crate) fn avatar() -> Vec<Widget> {
    let (_, view) = fission::build::current::<()>();
    let tokens = &view.env().theme.tokens;
    widgets![
        HStack {
            spacing: Some(tokens.spacing.s),
            children: widgets![
                Avatar {
                    name: Some("John Doe".into()),
                    src: None,
                    size: Some(AVATAR_SIZE),
                },
                Avatar {
                    name: Some("Ada Lovelace".into()),
                    src: None,
                    size: Some(AVATAR_SIZE),
                },
            ],
        },
        // A group overlaps its people and counts the ones it cannot show.
        HStack {
            spacing: None,
            children: widgets![AvatarGroup {
                id: Some(WidgetId::explicit("gallery.avatar.group")),
                avatars: TEAM
                    .iter()
                    .map(|(id, name)| AvatarGroupItem::new(
                        WidgetId::explicit(&format!("gallery.avatar.{id}")),
                        Avatar {
                            name: Some((*name).into()),
                            ..Default::default()
                        },
                    ))
                    .collect(),
                max_visible: Some(AVATAR_GROUP_VISIBLE),
                size: Some(AVATAR_SIZE),
                semantics_label: Some("Project members".into()),
                ..Default::default()
            }],
        },
    ]
}

pub(crate) fn badge() -> Vec<Widget> {
    let (_, view) = fission::build::current::<()>();
    let tokens = &view.env().theme.tokens;
    widgets![Wrap {
        direction: FlexDirection::Row,
        spacing: Some(tokens.spacing.s),
        run_spacing: Some(tokens.spacing.s),
        children: BADGE_TONES
            .iter()
            .map(|(tone, label)| {
                Badge {
                    text: (*label).into(),
                    tone: *tone,
                    ..Default::default()
                }
                .into()
            })
            .collect(),
    }]
}

pub(crate) fn divider() -> Vec<Widget> {
    let (_, view) = fission::build::current::<()>();
    let tokens = &view.env().theme.tokens;
    let label = |text: &str| -> Widget { Text::new(text).color(tokens.colors.text_primary).into() };
    let vertical = || -> Widget {
        Container::new(Divider {
            orientation: Orientation::Vertical,
            ..Default::default()
        })
        .height(tokens.sizing.icon_md)
        .into()
    };
    widgets![
        VStack {
            spacing: Some(tokens.spacing.s),
            children: vec![
                label("Account"),
                Divider::default().into(),
                label("Notifications"),
            ],
        },
        HStack {
            spacing: None,
            children: widgets![HStack {
                spacing: Some(tokens.spacing.m),
                children: vec![
                    label("Edit"),
                    vertical(),
                    label("Share"),
                    vertical(),
                    label("Delete"),
                ],
            }],
        },
        Divider {
            dash_pattern: Some(vec![DIVIDER_DASH, DIVIDER_DASH]),
            ..Default::default()
        },
    ]
}

fn ratio_tile(width: f32, ratio: f32, label: &str) -> Widget {
    let (_, view) = fission::build::current::<()>();
    let tokens = &view.env().theme.tokens;
    Container::new(AspectRatio {
        ratio,
        child: Container::new(Text::new(label).color(tokens.colors.text_primary))
            .width_length(Length::percent(100.0))
            .height_length(Length::percent(100.0))
            .align_child(BoxAlignment::Center)
            .bg(tokens.colors.primary_subtle)
            .border_radius(tokens.radii.medium)
            .into(),
    })
    .width(width)
    .into()
}

pub(crate) fn aspect_ratio() -> Vec<Widget> {
    let (_, view) = fission::build::current::<()>();
    let tokens = &view.env().theme.tokens;
    widgets![Row {
        gap: Some(tokens.spacing.m),
        align_items: fission::op::AlignItems::Start,
        children: vec![
            ratio_tile(WIDE_RATIO_WIDTH, 16.0 / 9.0, "16 : 9"),
            ratio_tile(SQUARE_RATIO_WIDTH, 1.0, "1 : 1"),
        ],
        ..Default::default()
    }]
}

fn hex(color: Color) -> String {
    format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b)
}

pub(crate) fn image() -> Vec<Widget> {
    let (_, view) = fission::build::current::<()>();
    let tokens = &view.env().theme.tokens;
    let colors = &tokens.colors;
    // An inline drawing, so the page works offline and follows the theme's colours.
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 240 160">
<rect width="240" height="160" fill="{sky}"/>
<circle cx="182" cy="46" r="20" fill="{sun}"/>
<path d="M0 160 L78 62 L130 124 L166 88 L240 160 Z" fill="{hill}"/>
</svg>"#,
        sky = hex(colors.primary_subtle),
        sun = hex(colors.warning),
        hill = hex(colors.primary),
    );
    let framed = |fit: ImageFit, label: &str| -> Widget {
        VStack {
            spacing: Some(tokens.spacing.xs),
            children: widgets![
                Container::new(
                    Image::svg_text(svg.clone())
                        .size(IMAGE_WIDTH, IMAGE_HEIGHT)
                        .fit(fit)
                        .semantic_label("Mountains under the sun"),
                )
                .size(IMAGE_WIDTH, IMAGE_HEIGHT)
                .bg(colors.surface)
                .border(colors.border, tokens.sizing.border_hairline)
                .border_radius(tokens.radii.medium),
                Text::new(label).color(colors.text_secondary),
            ],
        }
        .into()
    };
    widgets![Wrap {
        direction: FlexDirection::Row,
        spacing: Some(tokens.spacing.m),
        run_spacing: Some(tokens.spacing.m),
        children: vec![
            framed(ImageFit::Cover, "Cover"),
            framed(ImageFit::Contain, "Contain"),
        ],
    }]
}

pub(crate) fn markdown() -> Vec<Widget> {
    widgets![MarkdownContent::new(MARKDOWN_SAMPLE)]
}

pub(crate) fn tag() -> Vec<Widget> {
    let (_, view) = fission::build::current::<()>();
    let tokens = &view.env().theme.tokens;
    widgets![HStack {
        spacing: Some(tokens.spacing.s),
        children: widgets![
            Tag {
                label: "Rust".into(),
                on_close: None,
                on_press: None,
                selected: false,
            },
            Tag {
                label: "Design".into(),
                on_close: None,
                on_press: None,
                selected: true,
            },
        ],
    }]
}

pub(crate) fn code() -> Vec<Widget> {
    let (_, view) = fission::build::current::<()>();
    let tokens = &view.env().theme.tokens;
    widgets![HStack {
        spacing: Some(tokens.spacing.s),
        children: widgets![
            Code {
                text: "let x = 42;".into(),
            },
            Kbd {
                text: "Ctrl+C".into(),
            },
        ],
    }]
}

pub(crate) fn stat() -> Vec<Widget> {
    widgets![HStack {
        spacing: None,
        children: widgets![Stat {
            label: "Total Users".into(),
            value: "1,234".into(),
            help_text: Some("+12% this month".into()),
        }],
    }]
}

pub(crate) fn context_menu() -> Vec<Widget> {
    let (_, view) = fission::build::current::<()>();
    let tokens = &view.env().theme.tokens;
    let typography = &tokens.typography;
    widgets![
        Container::new(Text {
            id: Some(WidgetId::explicit("gallery.selectable.text")),
            content: TextContent::Literal(
                "Selectable Text: drag across this sentence, then use Ctrl/Cmd+C or right-click for Copy and Select All.".into(),
            ),
            selectable: true,
            wrap: true,
            ..Default::default()
        })
        .width_length(Length::percent(100.0)),
        Container::new(RichText {
            id: Some(WidgetId::explicit("gallery.selectable.rich_text")),
            runs: vec![
                RichTextRun::new("Selectable RichText: "),
                RichTextRun::new("mixed style text can be selected too.")
                    .color(tokens.colors.text_link)
                    .weight(typography.font_weight_bold),
            ],
            selectable: true,
            wrap: true,
            ..Default::default()
        })
        .width_length(Length::percent(100.0)),
        ContextMenuRegion::new(
            Container::new(
                Text::new("Right-click this custom region for a widget-backed context menu.")
                    .wrap(true),
            )
            .padding_lengths(Length::all(Length::points(tokens.spacing.s)))
            .border(tokens.colors.border, BORDER_WIDTH)
            .border_radius(tokens.radii.large),
            ContextMenu::with_items([ContextMenuEntry::Item(ContextMenuItem::new(
                "custom-help",
                HStack {
                    spacing: Some(tokens.spacing.xs),
                    children: widgets![
                        Badge {
                            text: "Tip".into(),
                            ..Default::default()
                        },
                        Text::new(TextContent::KeyWithFallback {
                            key: "gallery.context_menu.copy_help".into(),
                            fallback: "Menu items can be arbitrary widgets".into(),
                        }),
                    ],
                },
            ))]),
        )
        .id(WidgetId::explicit("gallery.custom.context_menu")),
    ]
}
