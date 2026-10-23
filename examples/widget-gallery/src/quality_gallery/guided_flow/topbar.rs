use super::palette::{rgba, BLUE, INK, LINE, MUTED, PALE_BLUE};
use fission::prelude::*;

fission_component! {
    pub(super) GuidedTopbar<()> {}
    |_topbar, _state, _cx| {} => Container(Row(
            gap: 16.0_f32,
            align_items: fission::op::AlignItems::Center,
            children: [
                Text("Forge").size(14.0).line_height(20.0).color(MUTED),
                Text("/").size(14.0).weight(600).color(MUTED),
                Text("Customer feedback triage")
                    .size(14.0)
                    .weight(600)
                    .color(INK),
                Badge(
                    text: "Draft",
                    color: PALE_BLUE,
                    text_color: BLUE,
                    size: ComponentSize::Sm,
                ),
                Icon::svg(material::action::check_circle::regular())
                    .size(18.0)
                    .color(rgba(0, 185, 133, 255)),
                Text("Saved just now").size(14.0).color(MUTED),
                Spacer(flex_grow: 1.0_f32),
                Avatar(name: "Alex Stone".to_string(), size: 34.0_f32),
                Avatar(name: "Jamie Miles".to_string(), size: 34.0_f32),
                Avatar(name: "Two more".to_string(), size: 34.0_f32),
                Button(
                    variant: ButtonVariant::Outline,
                    size: ComponentSize::Md,
                    content: ButtonContent::new("History")
                        .leading_icon(Icon::svg(material::action::history::regular())),
                ),
                Button(
                    variant: ButtonVariant::Outline,
                    size: ComponentSize::Md,
                    content: ButtonContent::new("Preview")
                        .leading_icon(Icon::svg(material::av::play_arrow::regular())),
                ),
                Button(
                    variant: ButtonVariant::Primary,
                    size: ComponentSize::Md,
                    content: ButtonContent::new("Review template").trailing_icon(Icon::svg(
                        material::navigation::arrow_forward::regular()
                    )),
                    background_fill: Fill::Solid(BLUE),
                    text_color: Color::WHITE,
                ),
            ],
        ))
        .height(76.0)
        .padding([32.0, 24.0, 16.0, 16.0])
        .border(LINE, 1.0)
}
