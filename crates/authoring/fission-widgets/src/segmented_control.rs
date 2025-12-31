use fission_core::ui::{Button, ButtonVariant, Container, Node, Text};
use fission_core::{BuildCtx, View, Widget, ActionEnvelope};
use crate::stack::HStack;
use std::sync::Arc;
use serde::{Deserialize, Serialize};

pub struct SegmentedControl {
    pub options: Vec<String>,
    pub selected_index: usize,
    pub on_change: Option<Arc<dyn Fn(usize) -> ActionEnvelope + Send + Sync>>,
}

// Manual Debug
impl std::fmt::Debug for SegmentedControl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SegmentedControl")
            .field("options", &self.options)
            .field("selected", &self.selected_index)
            .finish()
    }
}

impl<S: fission_core::AppState> Widget<S> for SegmentedControl {
    fn build(&self, _ctx: &mut BuildCtx<S>, view: &View<S>) -> Node {
        let tokens = &view.env.theme.tokens;
        let mut children = Vec::new();
        
        for (i, opt) in self.options.iter().enumerate() {
            let is_selected = i == self.selected_index;
            let cb = self.on_change.clone();
            
            children.push(
                Button {
                    variant: if is_selected { ButtonVariant::Filled } else { ButtonVariant::Ghost },
                    child: Some(Box::new(Text::new(opt.clone())
                        .color(if is_selected { tokens.colors.on_primary } else { tokens.colors.text_primary })
                        .into_node()
                    )),
                    on_press: cb.map(|f| f(i)),
                    // Remove default padding to fit tightly?
                    // padding: Some([4.0; 4]),
                    ..Default::default()
                }.into_node()
            );
        }

        Container::new(
            HStack {
                spacing: Some(2.0),
                children,
            }.into_node()
        )
        .padding_all(2.0)
        .bg(tokens.colors.surface) // or slightly darker/lighter
        .border(tokens.colors.border, 1.0)
        .border_radius(tokens.radii.full) // Pill shape
        .into_node()
    }
}
