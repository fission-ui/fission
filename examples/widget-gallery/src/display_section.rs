use crate::gallery_section::GallerySection;
use fission::core::ui::{ContextMenu, ContextMenuEntry, ContextMenuItem, ContextMenuRegion};
use fission::prelude::*;
use fission::widgets::{Avatar, Badge, Code, HStack, Kbd, Stat, Tag, Wrap};

const AVATAR_SIZE: f32 = 36.0;
const BORDER_WIDTH: f32 = 1.0;

pub(crate) struct DisplaySection;

impl From<DisplaySection> for Widget {
    fn from(_section: DisplaySection) -> Self {
        let (_, view) = fission::build::current::<()>();
        let tokens = &view.env().theme.tokens;
        let typography = &tokens.typography;

        GallerySection::new(
            "Display",
            widgets![
                Wrap {
                    direction: FlexDirection::Row,
                    spacing: Some(tokens.spacing.s),
                    children: widgets![
                        Text::new("Hello Fission")
                            .size(typography.body_large_size)
                            .color(tokens.colors.text_primary),
                        Badge {
                            text: "New".into(),
                            ..Default::default()
                        },
                        Tag {
                            label: "Rust".into(),
                            on_close: None,
                        },
                        Avatar {
                            name: Some("John Doe".into()),
                            src: None,
                            size: Some(AVATAR_SIZE),
                        },
                    ],
                },
                HStack {
                    spacing: Some(tokens.spacing.s),
                    children: widgets![
                        Code {
                            text: "let x = 42;".into(),
                        },
                        Kbd {
                            text: "Ctrl+C".into(),
                        },
                    ],
                },
                Stat {
                    label: "Total Users".into(),
                    value: "1,234".into(),
                    help_text: Some("+12% this month".into()),
                },
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
                            .color(tokens.colors.primary)
                            .weight(typography.font_weight_bold),
                    ],
                    selectable: true,
                    wrap: true,
                    ..Default::default()
                })
                .width_length(Length::percent(100.0)),
                ContextMenuRegion::new(
                    Container::new(
                        Text::new(
                            "Right-click this custom region for a widget-backed context menu.",
                        )
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
            ],
        )
        .into()
    }
}
