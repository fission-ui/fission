use fission::core::ui::{ContextMenu, ContextMenuEntry, ContextMenuItem, ContextMenuRegion};
use fission::prelude::*;
use fission::widgets::{Avatar, Badge, Code, HStack, Kbd, Stat, Tag};

const AVATAR_SIZE: f32 = 36.0;
const BORDER_WIDTH: f32 = 1.0;

pub(crate) fn avatar() -> Vec<Widget> {
    let (_, view) = fission::build::current::<()>();
    let tokens = &view.env().theme.tokens;
    widgets![HStack {
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
    }]
}

pub(crate) fn badge() -> Vec<Widget> {
    let (_, view) = fission::build::current::<()>();
    let tokens = &view.env().theme.tokens;
    widgets![HStack {
        spacing: Some(tokens.spacing.s),
        children: widgets![Badge {
            text: "New".into(),
            ..Default::default()
        }],
    }]
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
