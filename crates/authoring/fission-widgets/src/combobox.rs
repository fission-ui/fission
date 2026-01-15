use fission_core::ui::{Button, ButtonVariant, Container, Node, Text, TextInput};
use fission_core::{BuildCtx, View, Widget, ActionEnvelope, WidgetNodeId};
use crate::popover::Popover;
use crate::stack::VStack;
use std::sync::Arc;
use serde::{Deserialize, Serialize};

pub struct Combobox {
    pub id: WidgetNodeId,
    pub value: String,
    pub items: Vec<String>,
    pub is_open: bool,
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
        
        let input = TextInput {
            value: self.value.clone(),
            on_change: self.on_change.clone(),
            // TODO: on_focus -> open?
            ..Default::default()
        }.into_node();
        
        // Wrap input in button to toggle if not text input driven?
        // Combobox allows typing. So Input IS the trigger.
        // We need Input to allow focus.
        // Popover trigger can be the Input itself.
        
        let list_content = if self.is_open && !self.items.is_empty() {
            let mut list_items = Vec::new();
            for item in &self.items {
                let cb = self.on_select.clone();
                let val = item.clone();
                list_items.push(
                    Button {
                        variant: ButtonVariant::Ghost,
                        child: Some(Box::new(Text::new(item.clone()).flex_grow(1.0).into_node())),
                        on_press: cb.map(|f| f(val)),
                        width: Some(200.0), // Fixed width or match anchor?
                        // Match anchor requires LayoutReadback or Flex behavior.
                        // Popover content usually determines its own size.
                        ..Default::default()
                    }.into_node()
                );
            }
            
            let mut c = Container::new(
                VStack {
                    spacing: Some(0.0),
                    children: list_items,
                }.into_node()
            )
            .padding_all(4.0)
            .bg(tokens.colors.surface);
            
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
        }.build(_ctx, view)
    }
}
