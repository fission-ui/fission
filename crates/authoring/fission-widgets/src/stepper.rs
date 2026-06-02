use crate::stack::{HStack, VStack};
use fission_core::ui::{Align, Container, Text, Widget};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Stepper {
    pub steps: Vec<String>,
    pub active_index: usize,
}

impl From<Stepper> for Widget {
    fn from(component: Stepper) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let this = &component;

        let tokens = &view.env().theme.tokens;
        if this.steps.is_empty() {
            return fission_core::ui::widgets::Spacer::default().into();
        }

        let last_index = this.steps.len().saturating_sub(1);
        let node_slot = 62.0;
        let connector_width = 22.0;
        let mut indicator_row = Vec::new();
        let mut label_row = Vec::new();

        for (i, label) in this.steps.iter().enumerate() {
            let is_active = i == this.active_index;
            let is_completed = i < this.active_index;
            let is_emphasized = is_active || is_completed;

            let mut circle = Container::new(Align::new(
                Text::new(format!("{}", i + 1))
                    .size(12.0)
                    .color(if is_emphasized {
                        tokens.colors.on_primary
                    } else {
                        tokens.colors.text_secondary
                    }),
            ))
            .width(24.0)
            .height(24.0)
            .border_radius(12.0)
            .bg(if is_emphasized {
                tokens.colors.primary
            } else {
                tokens.colors.surface
            });

            if !is_emphasized {
                circle = circle.border(tokens.colors.border, 1.0);
            }

            indicator_row.push(Container::new(circle).width(node_slot).into());

            label_row.push(
                Container::new(Align::new(Text::new(label.clone()).size(11.0).color(
                    if is_emphasized {
                        tokens.colors.text_primary
                    } else {
                        tokens.colors.text_secondary
                    },
                )))
                .width(node_slot)
                .into(),
            );

            if i < last_index {
                let line_color = if i < this.active_index {
                    tokens.colors.primary
                } else {
                    tokens.colors.border
                };

                indicator_row.push(
                    Container::new(fission_core::ui::widgets::Spacer::default())
                        .width(connector_width)
                        .height(2.0)
                        .bg(line_color)
                        .into(),
                );
                label_row.push(
                    fission_core::ui::widgets::Spacer {
                        width: Some(connector_width),
                        ..Default::default()
                    }
                    .into(),
                );
            }
        }

        VStack {
            spacing: Some(8.0),
            children: vec![
                HStack {
                    spacing: Some(0.0),
                    children: indicator_row,
                }
                .into(),
                HStack {
                    spacing: Some(0.0),
                    children: label_row,
                }
                .into(),
            ],
        }
        .into()
    }
}
