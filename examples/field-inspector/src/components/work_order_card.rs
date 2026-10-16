use crate::components::ui::{MutedText, SmallButton};
use crate::data::WorkOrder;
use crate::model::FieldInspectorState;
use fission::prelude::*;

const COMPACT_CARD_WIDTH: f32 = 220.0;

/// Maps a work-order priority onto the badge tone that carries its urgency.
///
/// Priority is not a status of the card, so it reads by urgency rather than by
/// selection: the destructive tone for the job that cannot wait, the warning
/// tone below it, and a neutral grey for routine work.
fn priority_tone(priority: &str) -> BadgeTone {
    match priority {
        "Critical" => BadgeTone::Error,
        "High" => BadgeTone::Warning,
        _ => BadgeTone::Gray,
    }
}

pub struct WorkOrderCard {
    pub order: WorkOrder,
    pub selected: bool,
    pub compact: bool,
    pub action: ActionEnvelope,
}

impl From<WorkOrderCard> for Widget {
    fn from(card: WorkOrderCard) -> Self {
        let (_, view) = fission::build::current::<FieldInspectorState>();
        let tokens = &view.env().theme.tokens;
        let typography = &tokens.typography;

        let content = Column {
            gap: Some(tokens.spacing.s),
            children: widgets![
                Row {
                    gap: Some(tokens.spacing.s),
                    children: widgets![
                        Text::new(card.order.id)
                            .size(typography.font_size_base)
                            .weight(typography.font_weight_bold)
                            .color(tokens.colors.text_primary),
                        Spacer {
                            flex_grow: 1.0,
                            ..Default::default()
                        },
                        Badge {
                            text: card.order.priority.to_string(),
                            tone: priority_tone(card.order.priority),
                            size: ComponentSize::Sm,
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                },
                Text::new(card.order.title)
                    .size(typography.label_large_size)
                    .line_height(typography.label_large_size * typography.line_height_snug)
                    .weight(typography.font_weight_bold)
                    .color(tokens.colors.text_primary),
                MutedText::new(format!("{} - {}", card.order.site, card.order.due)),
                SmallButton::new(
                    format!("field-inspector.work-order.{}", card.order.id),
                    if card.selected { "Selected" } else { "Open" },
                    card.action,
                    if card.selected {
                        ButtonVariant::SecondaryColor
                    } else {
                        ButtonVariant::Ghost
                    },
                ),
            ],
            ..Default::default()
        };

        let mut container = Container::new(content)
            .bg(if card.selected {
                tokens.colors.primary.with_alpha(26)
            } else {
                tokens.colors.background.with_alpha(140)
            })
            .border(
                if card.selected {
                    tokens.colors.primary
                } else {
                    tokens.colors.border.with_alpha(120)
                },
                1.0,
            )
            .border_radius(tokens.radii.xl)
            .padding_all(tokens.spacing.s);

        if card.compact {
            container = container.width_length(Length::points(COMPACT_CARD_WIDTH));
        }

        container.into()
    }
}
