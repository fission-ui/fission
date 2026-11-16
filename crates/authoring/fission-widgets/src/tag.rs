use crate::stack::HStack;
use fission_core::action::ActionEnvelope;
use fission_core::ui::{
    Button, ButtonVariant, Container, SemanticsRegion, Text, TextContent, Widget,
};
use fission_ir::{Role, Semantics};
use fission_theme::ColorRole;
use serde::{Deserialize, Serialize};

fn relative_luminance(color: fission_theme::Color) -> f64 {
    let channel = |value: u8| {
        let value = f64::from(value) / 255.0;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(color.r) + 0.7152 * channel(color.g) + 0.0722 * channel(color.b)
}

fn contrast_ratio(a: fission_theme::Color, b: fission_theme::Color) -> f64 {
    let (a, b) = (relative_luminance(a), relative_luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

fn readable_selected_foreground(
    background: fission_theme::Color,
    preferred: fission_theme::Color,
) -> fission_theme::Color {
    if contrast_ratio(background, preferred) >= 4.5 {
        return preferred;
    }
    let black = fission_theme::Color {
        r: 0,
        g: 0,
        b: 0,
        a: 255,
    };
    let white = fission_theme::Color {
        r: 255,
        g: 255,
        b: 255,
        a: 255,
    };
    if contrast_ratio(background, black) >= contrast_ratio(background, white) {
        black
    } else {
        white
    }
}

/// A pill-shaped label with an optional close button.
///
/// Tags are typically used for removable filters, categories, or selections.
/// The close button (an "x" character) appears when `on_close` is provided, and
/// `on_press` makes the tag itself a toggle such as a filter.
#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct Tag {
    /// Text displayed inside the tag.
    pub label: String,
    /// Optional removal action; supplying it also displays the close affordance.
    pub on_close: Option<ActionEnvelope>,
    /// Optional press action; supplying it makes the tag a toggle, such as a filter.
    pub on_press: Option<ActionEnvelope>,
    /// Whether a pressable tag is on; filled with the primary colour and announced as selected.
    #[serde(default)]
    pub selected: bool,
}

impl From<Tag> for Widget {
    fn from(component: Tag) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let this = &component;

        let tokens = &view.env().theme.tokens;
        let recipe = view.env().theme.recipe(fission_theme::recipes::Tag);
        // A selected tag is filled, not ringed. A ring alone reads as a border
        // treatment and disappears next to an unselected pill, so the on state
        // takes the primary fill. Prefer the design system's paired foreground,
        // but do not use it for this small label unless it clears body-text
        // contrast.
        let selected_pair = tokens.colors.pair(ColorRole::Primary);
        let selected_foreground =
            readable_selected_foreground(selected_pair.fill, selected_pair.on);

        let text: Widget = Text {
            content: TextContent::Literal(this.label.clone()),
            font_size: Some(13.0),
            color: Some(if this.selected {
                selected_foreground
            } else {
                tokens.colors.text_primary
            }),
            ..Default::default()
        }
        .into();
        let press_semantics = || Semantics {
            role: Role::Button,
            label: Some(this.label.clone()),
            selected: Some(this.selected),
            focusable: true,
            ..Default::default()
        };
        // With a close button the label alone presses, so the two controls stay siblings; without
        // one the whole pill presses (below).
        let label = match (&this.on_press, &this.on_close) {
            (Some(action), Some(_)) => Button {
                variant: ButtonVariant::Ghost,
                child: Some(text),
                on_press: Some(action.clone()),
                padding: Some([0.0; 4]),
                height: Some(20.0),
                semantics: Some(press_semantics()),
                ..Default::default()
            }
            .into(),
            _ => text,
        };
        let mut children = vec![label];

        if let Some(action) = &this.on_close {
            children.push(
                SemanticsRegion::new(Button {
                    variant: ButtonVariant::Ghost,
                    child: Some(
                        Text {
                            content: TextContent::Literal("×".into()),
                            font_size: Some(14.0),
                            color: Some(if this.selected {
                                selected_foreground
                            } else {
                                tokens.colors.text_secondary
                            }),
                            ..Default::default()
                        }
                        .into(),
                    ),
                    on_press: Some(action.clone()),
                    // Minimal styling for close button
                    width: Some(20.0),
                    height: Some(20.0),
                    ..Default::default()
                })
                // A bare multiplication sign announces nothing. Name the action
                // and what it removes.
                .label(format!("Remove {}", this.label))
                .into(),
            );
        }

        let background = if this.selected {
            fission_core::op::Fill::Solid(selected_pair.fill)
        } else {
            recipe
                .base
                .background
                .clone()
                .unwrap_or(fission_core::op::Fill::Solid(tokens.colors.surface))
        };
        let radius = recipe.base.radius.unwrap_or(tokens.radii.full);
        let pill: Widget = Container::new(HStack {
            spacing: Some(4.0),
            children,
        })
        .bg_fill(background)
        .border(
            if this.selected {
                selected_pair.fill
            } else {
                tokens.colors.border
            },
            1.0,
        )
        .border_radius(radius)
        .padding(
            recipe
                .base
                .padding_box(tokens.spacing.xs, tokens.spacing.xs / 2.0),
        )
        .height(recipe.base.height.unwrap_or(24.0))
        .into();

        match (&this.on_press, &this.on_close) {
            (Some(action), None) => Button {
                variant: ButtonVariant::Ghost,
                child: Some(pill),
                on_press: Some(action.clone()),
                padding: Some([0.0; 4]),
                style: Some(fission_core::ui::ButtonStyleOverride {
                    corner_radius: Some(radius),
                    ..Default::default()
                }),
                semantics: Some(press_semantics()),
                ..Default::default()
            }
            .into(),
            _ => pill,
        }
    }
}
