use crate::Icon;
use fission_core::ui::{Button, ButtonVariant, Container, Row, TextInput, Widget};
use fission_core::{ActionEnvelope, WidgetId};
use fission_icons::material;
use serde::{Deserialize, Serialize};

/// Controlled numeric field with increment and decrement buttons.
///
/// Button presses dispatch the supplied actions; typed text is delivered
/// through the normal contextual text-input contract for application parsing.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NumberInput {
    /// Optional stable identity, also used to derive the inner text-field ID.
    pub id: Option<WidgetId>,
    /// Current numeric value owned by application state.
    pub value: f32,
    /// Optional preformatted text; otherwise `value` is formatted directly.
    pub display_text: Option<String>,
    /// Optional inclusive lower bound communicated to button behavior.
    pub min: Option<f32>,
    /// Optional inclusive upper bound communicated to button behavior.
    pub max: Option<f32>,
    /// Amount represented by each increment or decrement action.
    pub step: f32,
    /// Optional logical width of the editable field.
    pub field_width: Option<f32>,
    /// Optional logical square size of each step button.
    pub button_size: Option<f32>,
    /// Optional logical gap between buttons and field.
    pub gap: Option<f32>,
    /// Action dispatched when increment is available and requested.
    pub on_increment: Option<ActionEnvelope>,
    /// Action dispatched when decrement is available and requested.
    pub on_decrement: Option<ActionEnvelope>,
    /// Action dispatched for typed edits. Parse `ctx.input.text_change().new_text`
    /// in the reducer so validation remains an application decision.
    pub on_input: Option<ActionEnvelope>,
}

impl Default for NumberInput {
    fn default() -> Self {
        Self {
            id: None,
            value: 0.0,
            display_text: None,
            min: None,
            max: None,
            step: 1.0,
            field_width: None,
            button_size: None,
            gap: None,
            on_increment: None,
            on_decrement: None,
            on_input: None,
        }
    }
}

impl From<NumberInput> for Widget {
    fn from(component: NumberInput) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let mut component = component;
        component.id = fission_core::build::current_widget_id().or(component.id);
        let this = &component;

        let tokens = &view.env().theme.tokens;
        let display_text = this
            .display_text
            .clone()
            .unwrap_or_else(|| format!("{}", this.value));
        let glyph_count = display_text.chars().count().max(2) as f32;
        let field_width = this
            .field_width
            .unwrap_or((glyph_count * 10.0 + 20.0).clamp(52.0, 96.0));
        let button_size = this.button_size.unwrap_or(32.0).max(28.0);
        let icon_size = (button_size * 0.5).clamp(14.0, 18.0);
        let input_id = this
            .id
            .as_ref()
            .map(|id| WidgetId::derived(id.as_u128(), &[0]));

        Container::new(
            Row::default()
                .gap(this.gap.unwrap_or(4.0))
                .align_items(fission_ir::op::AlignItems::Center)
                .children(vec![
                    Button {
                        variant: ButtonVariant::Ghost,
                        child: Some(
                            Icon::svg(material::content::remove::regular())
                                .size(icon_size)
                                .into(),
                        ),
                        on_press: this.on_decrement.clone(),
                        width: Some(button_size),
                        height: Some(button_size),
                        padding: Some([0.0; 4]),
                        ..Default::default()
                    }
                    .into(),
                    TextInput {
                        id: input_id.map(Into::into),
                        value: display_text,
                        width: Some(field_width),
                        borderless: true,
                        keyboard_type: fission_ir::semantics::TextInputType::Number,
                        on_input: this.on_input.clone(),
                        ..Default::default()
                    }
                    .into(),
                    Button {
                        variant: ButtonVariant::Ghost,
                        child: Some(
                            Icon::svg(material::content::add::regular())
                                .size(icon_size)
                                .into(),
                        ),
                        on_press: this.on_increment.clone(),
                        width: Some(button_size),
                        height: Some(button_size),
                        padding: Some([0.0; 4]),
                        ..Default::default()
                    }
                    .into(),
                ]),
        )
        .padding_all(2.0)
        .bg(tokens.colors.background)
        .border(tokens.colors.border, 1.0)
        .border_radius(tokens.radii.medium)
        .into()
    }
}
