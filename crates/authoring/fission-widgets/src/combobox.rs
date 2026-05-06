use crate::popover::Popover;
use crate::stack::VStack;
use fission_core::ui::{Button, ButtonVariant, Container, Node, Scroll, Text, TextInput};
use fission_core::{ActionEnvelope, BuildCtx, NodeId, View, Widget, WidgetNodeId};
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
    pub id: WidgetNodeId,
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

impl<S: fission_core::AppState> Widget<S> for Combobox {
    fn build(&self, _ctx: &mut BuildCtx<S>, view: &View<S>) -> Node {
        let tokens = &view.env.theme.tokens;
        let input_id = NodeId::derived(self.id.as_u128(), &[1]);
        let popup_width = self.width.unwrap_or(320.0);
        let popup_max_height = self.max_popup_height.unwrap_or(240.0);

        let input = TextInput {
            id: Some(input_id),
            value: self.value.clone(),
            on_change: self.on_change.clone(),
            width: self.width,
            // TODO: on_focus -> open?
            ..Default::default()
        }
        .into_node();

        // Wrap input in button to toggle if not text input driven?
        // Combobox allows typing. So Input IS the trigger.
        // We need Input to allow focus.
        // Popover trigger can be the Input itself.

        let list_content = if self.is_open && !self.items.is_empty() {
            let item_height = 36.0;
            let estimated_height = self.items.len() as f32 * item_height;
            let popup_height = estimated_height.min(popup_max_height);
            let show_scrollbar = estimated_height > popup_height + 0.5;

            let mut list_items = Vec::new();
            for item in &self.items {
                let cb = self.on_select.clone();
                let val = item.clone();
                list_items.push(
                    Button {
                        variant: ButtonVariant::Ghost,
                        child: Some(Box::new(
                            Text::new(item.clone())
                                .size(14.0)
                                .flex_grow(1.0)
                                .into_node(),
                        )),
                        on_press: cb.map(|f| f(val)),
                        height: Some(36.0),
                        padding: Some([12.0, 12.0, 0.0, 0.0]),
                        ..Default::default()
                    }
                    .into_node(),
                );
            }

            let list = VStack {
                spacing: Some(0.0),
                children: list_items,
            }
            .into_node();

            let mut c = Container::new(
                Scroll {
                    child: Some(Box::new(list)),
                    width: Some(popup_width),
                    height: Some(popup_height),
                    show_scrollbar,
                    ..Default::default()
                }
                .into_node(),
            )
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
            c.into_node()
        } else {
            fission_core::ui::widgets::spacer::Spacer::default().into_node()
        };

        Popover {
            id: self.id,
            is_open: self.is_open && !self.items.is_empty(),
            on_toggle: self.on_toggle.clone(),
            on_close: self.on_toggle.clone(), // Close on click outside
            trigger: Box::new(input),
            content: Box::new(list_content),
        }
        .build(_ctx, view)
    }
}
