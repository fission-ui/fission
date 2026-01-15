use fission_core::ui::{Align, Container, Node, Text};
use fission_core::{BuildCtx, View, Widget};
use fission_core::op::Color;
use crate::stack::{HStack, VStack};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Stepper {
    pub steps: Vec<String>,
    pub active_index: usize,
}

impl<S: fission_core::AppState> Widget<S> for Stepper {
    fn build(&self, _ctx: &mut BuildCtx<S>, view: &View<S>) -> Node {
        let tokens = &view.env.theme.tokens;
        let mut children = Vec::new();
        
        for (i, label) in self.steps.iter().enumerate() {
            let is_active = i == self.active_index;
            let is_completed = i < self.active_index;
            
            let color = if is_active || is_completed { tokens.colors.primary } else { tokens.colors.border };
            let text_color = if is_active || is_completed { tokens.colors.on_primary } else { tokens.colors.text_secondary };
            
            // Connector line
            if i > 0 {
                children.push(
                    Container::new(fission_core::ui::widgets::Spacer::default().into_node())
                        .width(40.0) // Fixed line length
                        .height(2.0)
                        .bg(if is_completed { tokens.colors.primary } else { tokens.colors.border })
                        .into_node()
                );
            }
            
            // Circle + Label
            children.push(
                VStack {
                    spacing: Some(4.0),
                    children: vec![
                        // Circle
                        Container::new(
                            Align::new(
                                if is_completed {
                                    Text::new("✓").color(text_color).into_node()
                                } else {
                                    Text::new(format!("{}", i + 1)).color(text_color).into_node()
                                }
                            )
                            .into_node()
                        )
                        .width(24.0)
                        .height(24.0)
                        .border_radius(12.0)
                        .bg(color)
                        // .border?
                        .into_node(),
                        
                        Text::new(label.clone())
                            .size(12.0)
                            .color(text_color)
                            .into_node()
                    ]
                }.into_node()
            );
        }

        HStack {
            spacing: Some(8.0),
            children,
        }.into_node()
    }
}
