use super::palette::{rgba, BLUE, INK, LINE, MUTED, PALE_BLUE, SOFT};
use fission::prelude::*;

fission_component! {
    pub(super) GuidedAssistant<()> {}
    |_assistant, _state, _cx| {} => Container(Column(
            gap: 0.0_f32,
            children: [
                Text("Build with Fission").size(23.0).line_height(28.0).weight(700).color(INK),
                Text("Describe a change or edit the workflow directly.").size(15.0).line_height(22.0).color(MUTED),
                Spacer(height: 45.0_f32),
                GuidedAssistantMessage("I kept Inbox Listener read-only and limited it to two mailbox actions.\nWhat would you like to change?"),
                Spacer(height: 19.0_f32),
                Container(Column(gap: 10.0_f32, children: [
                    Text("▣   Last change").size(14.0).weight(600).color(BLUE),
                    Text("●   Added 2 rules").size(14.0).color(rgba(64, 82, 141, 255)),
                    Text("●   Scoped mailbox access").size(14.0).color(rgba(64, 82, 141, 255)),
                    Text("View changes   →").size(14.0).weight(500).color(BLUE),
                ]))
                .bg(SOFT).border_radius(8.0).padding_all(16.0).min_height(150.0).margin([58.0, 0.0, 0.0, 0.0]),
                Spacer(height: 22.0_f32),
                Row(gap: 12.0_f32, justify_content: fission::op::JustifyContent::End, align_items: fission::op::AlignItems::Center, children: [
                    Container(Text("Add a fallback when an email has no product area.").size(14.0).line_height(22.0).color(rgba(37, 55, 117, 255))).max_width(265.0).bg(PALE_BLUE).border_radius(8.0).padding_all(12.0),
                    Avatar(name: "Alex Stone".to_string(), size: 34.0_f32),
                ]),
                Spacer(height: 16.0_f32),
                GuidedProposal(),
                Spacer(flex_grow: 1.0_f32),
                TextInput(
                    id: WidgetId::explicit("quality-gallery.guided.assistant-input"),
                    value: String::new(),
                    placeholder: Some("Ask Fission to change this template...".into()),
                    size: ComponentSize::Xl,
                    prefix: Some(
                        Icon::svg(material::file::attachment::regular())
                            .size(18.0)
                            .color(INK)
                            .into()
                    ),
                    suffix: Some(
                        Button(
                            variant: ButtonVariant::Primary,
                            size: ComponentSize::Lg,
                            child: Some(
                                Icon::svg(material::navigation::arrow_upward::regular())
                                    .size(18.0)
                                    .color(Color::WHITE)
                                    .into()
                            ),
                            background_fill: Fill::Solid(BLUE),
                        ),
                    ),
                    border_color: rgba(204, 214, 237, 255),
                    border_radius: 8.0_f32,
                ),
                Spacer(height: 9.0_f32),
                Text("Fission proposes changes. You choose what to apply.").size(12.0).line_height(32.0).color(MUTED),
            ],
        ))
        .width(422.0)
        .padding([22.0, 22.0, 30.0, 0.0])
        .border(LINE, 1.0)
}

fission_component! {
    pub(super) GuidedAssistantMessage<()> { text: &'static str }
    |message, _state, _cx| {} =>
        Row(
            gap: 12.0_f32,
            align_items: fission::op::AlignItems::Start,
            children: [
                Container(
                    Icon::svg(material::hardware::toys::regular())
                        .size(22.0)
                        .color(BLUE)
                )
                .size(40.0, 40.0)
                .align_child(BoxAlignment::Center)
                .border(LINE, 1.0)
                .border_radius(20.0),
                Container(
                    Text(message.text)
                        .size(14.0)
                        .line_height(23.0)
                        .color(rgba(37, 55, 117, 255))
                )
                .bg(SOFT)
                .border_radius(8.0)
                .padding([14.0, 14.0, 12.0, 12.0])
                .flex_grow(1.0),
            ],
        )
}

fission_component! {
    pub(super) GuidedProposal<()> {}
    |_proposal, _state, _cx| {} =>
        Row(
            gap: 12.0_f32,
            align_items: fission::op::AlignItems::Start,
            children: [
                Container(Icon::svg(material::hardware::toys::regular()).size(22.0).color(BLUE))
                    .size(40.0, 40.0).align_child(BoxAlignment::Center).border(LINE, 1.0).border_radius(20.0),
                Container(Column(gap: 10.0_f32, children: [
                    Text("I can add a routing rule and send unresolved items to Product Planner.").size(14.0).line_height(22.0).color(rgba(37,55,117,255)),
                    Divider::default(),
                    Text("ϟ  This change is proposed, not applied.").size(13.0).color(MUTED),
                    Row(gap: 10.0_f32, children: [
                        Button(variant: ButtonVariant::Primary, size: ComponentSize::Md, content: ButtonContent::new("Apply"), background_fill: Fill::Solid(BLUE), text_color: Color::WHITE),
                        Button(variant: ButtonVariant::Outline, size: ComponentSize::Md, content: ButtonContent::new("Adjust")),
                    ]),
                ]))
                .bg(SOFT).border_radius(8.0).padding_all(14.0).flex_grow(1.0),
            ],
        )
}
