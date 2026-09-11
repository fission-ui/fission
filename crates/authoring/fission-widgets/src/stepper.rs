use crate::stack::{HStack, VStack};
use fission_core::ui::{Align, Container, SemanticsRegion, Text, Widget};
use fission_ir::Role;
use serde::{Deserialize, Serialize};

/// Ordered progress indicator for a finite multi-step workflow.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Stepper {
    /// Step labels in workflow order.
    pub steps: Vec<String>,
    /// Zero-based current step; earlier steps receive completed treatment.
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

        // A stepper is a progress measure with named stops. Reporting position
        // and total means a reader hears "step 2 of 4, Delivery" rather than a
        // row of unlabelled circles.
        let position = (this.active_index + 1).min(this.steps.len());
        let active_label = this
            .steps
            .get(this.active_index)
            .cloned()
            .unwrap_or_default();
        SemanticsRegion::new(VStack {
            spacing: Some(tokens.spacing.s),
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
        })
        .role(Role::ProgressBar)
        .label(format!(
            "Step {position} of {}: {active_label}",
            this.steps.len()
        ))
        .value(active_label)
        .range(1.0, this.steps.len() as f32, position as f32)
        .into()
    }
}
