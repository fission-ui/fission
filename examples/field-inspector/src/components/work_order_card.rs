use crate::components::ui::{MutedText, SmallButton};
use crate::data::WorkOrder;
use crate::model::FieldInspectorState;
use fission::prelude::*;
const COMPACT_CARD_WIDTH: f32 = 220.0;
fn priority_tone(priority: &str) -> BadgeTone {
    match priority {
        "Critical" => BadgeTone::Error,
        "High" => BadgeTone::Warning,
        _ => BadgeTone::Gray,
    }
}
fission_component! {
    pub WorkOrderCard<FieldInspectorState> { order: WorkOrder, selected: bool, compact: bool, action: ActionEnvelope }
    |card, _state, _cx| {} =>
        if card.compact {
            Container(WorkOrderCardSurface(card.order, card.selected, card.action))
                .width_length(Length::points(COMPACT_CARD_WIDTH))
        } else {
            WorkOrderCardSurface(card.order, card.selected, card.action)
        }
}

fission_component! {
    WorkOrderCardSurface<FieldInspectorState> { order: WorkOrder, selected: bool, action: ActionEnvelope }
    |card, _state, cx| {
        let tokens = &cx.view.env().theme.tokens; let typography = &tokens.typography;
    } =>
        Container(
            Column(
                gap: tokens.spacing.s,
                children: [
                    Row(
                        gap: tokens.spacing.s,
                        children: [
                            Text(card.order.id)
                                .size(typography.font_size_base)
                                .weight(typography.font_weight_bold)
                                .color(tokens.colors.text_primary),
                            Spacer(flex_grow: 1.0_f32),
                            Badge(
                                text: card.order.priority.to_string(),
                                tone: priority_tone(card.order.priority),
                                size: ComponentSize::Sm,
                            ),
                        ],
                    ),
                    Text(card.order.title)
                        .size(typography.label_large_size)
                        .line_height(typography.label_large_size * typography.line_height_snug)
                        .weight(typography.font_weight_bold)
                        .color(tokens.colors.text_primary),
                    MutedText(format!("{} - {}", card.order.site, card.order.due)),
                    SmallButton(
                        format!("field-inspector.work-order.{}", card.order.id),
                        if card.selected { "Selected" } else { "Open" }.to_string(),
                        card.action.clone(),
                        if card.selected { ButtonVariant::SecondaryColor } else { ButtonVariant::Ghost },
                    ),
                ],
            )
        )
        .bg(if card.selected { tokens.colors.primary.with_alpha(26) } else { tokens.colors.background.with_alpha(140) })
        .border(if card.selected { tokens.colors.primary } else { tokens.colors.border.with_alpha(120) }, 1.0)
        .border_radius(tokens.radii.xl)
        .padding_all(tokens.spacing.s)
}
