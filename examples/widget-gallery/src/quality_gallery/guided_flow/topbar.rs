use super::palette::{rgba, BLUE, INK, LINE, MUTED, PALE_BLUE};
use fission::prelude::*;

pub(super) struct GuidedTopbar;
impl From<GuidedTopbar> for Widget {
    fn from(_topbar: GuidedTopbar) -> Self {
        Container::new(Row {
            gap: Some(16.0),
            align_items: fission::op::AlignItems::Center,
            children: widgets![
                Text::new("Forge").size(14.0).line_height(20.0).color(MUTED),
                Text::new("/").size(14.0).weight(600).color(MUTED),
                Text::new("Customer feedback triage")
                    .size(14.0)
                    .weight(600)
                    .color(INK),
                Badge {
                    text: "Draft".into(),
                    color: Some(PALE_BLUE),
                    text_color: Some(BLUE),
                    size: ComponentSize::Sm,
                    ..Default::default()
                },
                Icon::svg(material::action::check_circle::regular())
                    .size(18.0)
                    .color(rgba(0, 185, 133, 255)),
                Text::new("Saved just now").size(14.0).color(MUTED),
                Spacer {
                    flex_grow: 1.0,
                    ..Default::default()
                },
                Avatar {
                    name: Some("Alex Stone".into()),
                    size: Some(34.0),
                    ..Default::default()
                },
                Avatar {
                    name: Some("Jamie Miles".into()),
                    size: Some(34.0),
                    ..Default::default()
                },
                Avatar {
                    name: Some("Two more".into()),
                    size: Some(34.0),
                    ..Default::default()
                },
                Button {
                    variant: ButtonVariant::Outline,
                    size: ComponentSize::Md,
                    content: Some(
                        ButtonContent::new("History")
                            .leading_icon(Icon::svg(material::action::history::regular()))
                    ),
                    ..Default::default()
                },
                Button {
                    variant: ButtonVariant::Outline,
                    size: ComponentSize::Md,
                    content: Some(
                        ButtonContent::new("Preview")
                            .leading_icon(Icon::svg(material::av::play_arrow::regular()))
                    ),
                    ..Default::default()
                },
                Button {
                    variant: ButtonVariant::Primary,
                    size: ComponentSize::Md,
                    content: Some(
                        ButtonContent::new("Review template").trailing_icon(Icon::svg(
                            material::navigation::arrow_forward::regular()
                        ))
                    ),
                    background_fill: Some(Fill::Solid(BLUE)),
                    text_color: Some(Color::WHITE),
                    ..Default::default()
                },
            ],
            ..Default::default()
        })
        .height(76.0)
        .padding([32.0, 24.0, 16.0, 16.0])
        .border(LINE, 1.0)
        .into()
    }
}
