use crate::Icon;
use fission_core::ui::{Button, ButtonVariant, Container, Row, SemanticsRegion, TextInput, Widget};
use fission_core::{ActionEnvelope, WidgetId};
use fission_icons::material;
use fission_ir::Role;
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
    /// Accessible name for the value being adjusted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
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
            label: None,
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
        let recipe = view
            .env()
            .theme
            .recipe(fission_theme::recipe_names::NUMBER_INPUT);
        let display_text = this
            .display_text
            .clone()
            .unwrap_or_else(|| format!("{}", this.value));
        let glyph_count = display_text.chars().count().max(2) as f32;
        let field_width = this
            .field_width
            .unwrap_or((glyph_count * 10.0 + 20.0).clamp(52.0, 96.0));
        let button_size = this
            .button_size
            .or(recipe.part("stepper").width)
            .unwrap_or(32.0)
            .max(28.0);
        let icon_size = (button_size * 0.5).clamp(14.0, 18.0);
        let input_id = this
            .id
            .as_ref()
            .map(|id| WidgetId::derived(id.as_u128(), &[0]));

        let display_value = display_text.clone();
        let field = Container::new(
            Row::default()
                .gap(this.gap.or(recipe.base.gap).unwrap_or(4.0))
                .align_items(fission_ir::op::AlignItems::Center)
                .children(vec![
                    SemanticsRegion::new(Button {
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
                    })
                    // Icon-only controls need names; a bare glyph announces
                    // nothing.
                    .label("Decrease")
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
                    SemanticsRegion::new(Button {
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
                    })
                    .label("Increase")
                    .into(),
                ]),
        )
        .padding(recipe.base.padding_box(2.0, 2.0))
        .bg_fill(
            recipe
                .base
                .background
                .clone()
                .unwrap_or(fission_core::op::Fill::Solid(tokens.colors.background)),
        )
        .border(tokens.colors.border, 1.0)
        .border_radius(recipe.base.radius.unwrap_or(tokens.radii.medium));

        // A stepper over a number is a spin button: the group reports the value
        // and its bounds, so a reader hears the number and how far it can go
        // rather than three unrelated controls.
        let mut semantics = SemanticsRegion::new(field)
            .role(Role::SpinButton)
            .value(display_value);
        if let (Some(min), Some(max)) = (this.min, this.max) {
            semantics = semantics.range(min, max, this.value);
        }
        if let Some(label) = this.label.clone() {
            semantics = semantics.label(label);
        }
        semantics.into()
    }
}
