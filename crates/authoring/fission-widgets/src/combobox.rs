use crate::popover::Popover;
use crate::stack::VStack;
use fission_core::ui::{Button, ButtonVariant, Container, Scroll, Text, TextInput, Widget};
use fission_core::{ActionEnvelope, WidgetId};
use std::sync::Arc;

/// A searchable dropdown that combines a text input with a filterable item list.
///
/// The user types into a `TextInput` (the trigger), which filters the available
/// items. The filtered list is displayed in a [`Popover`] anchored to the input.
/// Selecting an item dispatches `on_select` with the chosen value.
///
/// # Fields
///
/// * `id` - Stable widget identity.
/// * `value` - Current text input value (controlled).
/// * `items` - The list of available options to display.
/// * `is_open` - Whether the dropdown list is visible.
/// * `on_change` - Action dispatched when the text input value changes.
/// * `on_select` - Closure that produces an action when an item is picked.
/// * `on_toggle` - Action dispatched to open/close the dropdown.
pub struct Combobox {
    pub id: WidgetId,
    pub value: String,
    pub items: Vec<String>,
    pub is_open: bool,
    pub width: Option<f32>,
    pub max_popup_height: Option<f32>,
    pub on_change: Option<ActionEnvelope>, // Text changed
    pub on_select: Option<Arc<dyn Fn(String) -> ActionEnvelope + Send + Sync>>, // Item picked
    pub on_toggle: Option<ActionEnvelope>, // Focus/Blur handling usually
}

impl std::fmt::Debug for Combobox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Combobox")
            .field("value", &self.value)
            .field("items_count", &self.items.len())
            .finish()
    }
}

impl From<Combobox> for Widget {
    fn from(component: Combobox) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let mut component = component;
        if let Some(id) = fission_core::build::current_widget_id() {
            component.id = id;
        }
        let this = &component;

        let tokens = &view.env().theme.tokens;
        let input_id = WidgetId::derived(this.id.as_u128(), &[1]);
        let popup_width = this.width.unwrap_or(320.0);
        let popup_max_height = this.max_popup_height.unwrap_or(240.0);

        let input = TextInput {
            id: Some(input_id.into()),
            value: this.value.clone(),
            on_change: this.on_change.clone(),
            width: this.width,
            // TODO: on_focus -> open?
            ..Default::default()
        }
        .into();

        // Wrap input in button to toggle if not text input driven?
        // Combobox allows typing. So Input IS the trigger.
        // We need Input to allow focus.
        // Popover trigger can be the Input itself.

        let list_content = if this.is_open && !this.items.is_empty() {
            let item_height = 36.0;
            let estimated_height = this.items.len() as f32 * item_height;
            let popup_height = estimated_height.min(popup_max_height);
            let show_scrollbar = estimated_height > popup_height + 0.5;

            let mut list_items = Vec::new();
            for item in &this.items {
                let cb = this.on_select.clone();
                let val = item.clone();
                list_items.push(
                    Button {
                        variant: ButtonVariant::Ghost,
                        child: Some(Text::new(item.clone()).size(14.0).flex_grow(1.0).into()),
                        on_press: cb.map(|f| f(val)),
                        height: Some(36.0),
                        padding: Some([12.0, 12.0, 0.0, 0.0]),
                        ..Default::default()
                    }
                    .into(),
                );
            }

            let list = VStack {
                spacing: Some(0.0),
                children: list_items,
            }
            .into();

            let mut c = Container::new(Scroll {
                child: Some(list),
                width: Some(popup_width),
                height: Some(popup_height),
                show_scrollbar,
                ..Default::default()
            })
            // Keep the popover hit region aligned to visible content only.
            .width(popup_width + 8.0)
            .height(popup_height + 8.0)
            .padding_all(4.0)
            .bg(tokens.colors.surface)
            .border(tokens.colors.border, 1.0)
            .border_radius(tokens.radii.medium);

            if let Some(s) = tokens.elevations.level2 {
                c = c.shadow(s);
            }
            c.into()
        } else {
            fission_core::ui::widgets::spacer::Spacer::default().into()
        };

        Popover {
            id: this.id,
            is_open: this.is_open && !this.items.is_empty(),
            on_toggle: this.on_toggle.clone(),
            on_close: this.on_toggle.clone(), // Close on click outside
            trigger: input,
            content: list_content,
        }
        .into()
    }
}
