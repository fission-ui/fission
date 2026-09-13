use fission::core::op::ImageFit;
use fission::core::ui::{Container, Text, TextContent, Video, Widget};
use fission::core::WidgetId;
use fission::widgets::{
    AspectRatio, Card, Code, HStack, Image, Kbd, SimpleGrid, Spinner, SpinnerMotion, VStack,
};

/// The thread's attachments while they are scanned.
pub(super) struct Attachments;

impl From<Attachments> for Widget {
    fn from(_: Attachments) -> Self {
        let (_, view) = fission::build::current::<()>();
        let tokens = &view.env().theme.tokens;

        Container::new(VStack {
            spacing: Some(tokens.spacing.s),
            children: vec![
                Text::new(TextContent::Key("email.attachments".into()))
                    .size(tokens.typography.font_size_lg)
                    .into(),
                HStack {
                    spacing: Some(tokens.spacing.s),
                    children: vec![
                        Spinner {
                            id: WidgetId::explicit("attachments_spinner"),
                            label: None,
                            color: None,
                            motion: Some(SpinnerMotion::Default),
                        }
                        .into(),
                        Text::new(TextContent::Key("email.scanning_attachments".into()))
                            .size(tokens.typography.font_size_xs)
                            .color(tokens.colors.text_secondary)
                            .into(),
                    ],
                }
                .into(),
                SimpleGrid {
                    min_child_width: 120.0,
                    gap: Some(tokens.spacing.s),
                    children: vec![
                        AspectRatio {
                            ratio: 4.0 / 3.0,
                            child: Image::network("https://picsum.photos/200/150")
                                .fit(ImageFit::Cover)
                                .into(),
                        }
                        .into(),
                        AspectRatio {
                            ratio: 4.0 / 3.0,
                            child: Image::network("https://picsum.photos/201/150")
                                .fit(ImageFit::Cover)
                                .into(),
                        }
                        .into(),
                        AspectRatio {
                            ratio: 16.0 / 9.0,
                            child: Video::asset("docs/video1.mp4").into(),
                        }
                        .into(),
                    ],
                }
                .into(),
            ],
        })
        .padding_all(tokens.spacing.s)
        .bg(tokens.colors.surface)
        .border(tokens.colors.border, 1.0)
        .border_radius(tokens.radii.medium)
        .into()
    }
}

/// A search tip with an example query and keyboard shortcut.
pub(super) struct PowerTip;

impl From<PowerTip> for Widget {
    fn from(_: PowerTip) -> Self {
        let (_, view) = fission::build::current::<()>();
        let tokens = &view.env().theme.tokens;

        Card {
            child: Container::new(VStack {
                spacing: Some(tokens.spacing.s),
                children: vec![
                    Text::new(TextContent::Key("email.power_tip".into()))
                        .size(tokens.typography.font_size_base)
                        .into(),
                    Code {
                        text: "label:important after:2025/01/01".into(),
                    }
                    .into(),
                    HStack {
                        spacing: Some(tokens.spacing.xs),
                        children: vec![
                            Kbd { text: "g".into() }.into(),
                            Kbd { text: "i".into() }.into(),
                            Text::new(view.tr("email.jump_to_inbox"))
                                .size(tokens.typography.font_size_xs)
                                .into(),
                        ],
                    }
                    .into(),
                ],
            })
            .padding_all(tokens.spacing.s)
            .into(),
            ..Default::default()
        }
        .into()
    }
}
