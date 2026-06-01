use crate::stack::{HStack, VStack};
use fission_core::ui::{Align, Container, Text};
use fission_core::{BuildCtx, View, Widget};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Stepper {
    pub steps: Vec<String>,
    pub active_index: usize,
}

impl<S: fission_core::AppState> Widget<S> for Stepper {
    fn build(&self, _ctx: &mut BuildCtx<S>, view: &View<S>) -> impl fission_core::IntoWidget<S> {
        fission_core::view::internal_node_widget({
            let tokens = &view.env.theme.tokens;
            if self.steps.is_empty() {
                return fission_core::view::internal_node_widget(
                    fission_core::ui::widgets::Spacer::default().into_node(),
                );
            }

            let last_index = self.steps.len().saturating_sub(1);
            let node_slot = 62.0;
            let connector_width = 22.0;
            let mut indicator_row = Vec::new();
            let mut label_row = Vec::new();

            for (i, label) in self.steps.iter().enumerate() {
                let is_active = i == self.active_index;
                let is_completed = i < self.active_index;
                let is_emphasized = is_active || is_completed;

                let mut circle = Container::<fission_core::ui::Node>::lowered(
                    Align::new(
                        Text::new(format!("{}", i + 1))
                            .size(12.0)
                            .color(if is_emphasized {
                                tokens.colors.on_primary
                            } else {
                                tokens.colors.text_secondary
                            })
                            .into_node(),
                    )
                    .into_node(),
                )
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

                indicator_row.push(
                    Container::<fission_core::ui::Node>::lowered(circle.into_node())
                        .width(node_slot)
                        .into_node(),
                );

                label_row.push(
                    Container::<fission_core::ui::Node>::lowered(
                        Align::new(
                            Text::new(label.clone())
                                .size(11.0)
                                .color(if is_emphasized {
                                    tokens.colors.text_primary
                                } else {
                                    tokens.colors.text_secondary
                                })
                                .into_node(),
                        )
                        .into_node(),
                    )
                    .width(node_slot)
                    .into_node(),
                );

                if i < last_index {
                    let line_color = if i < self.active_index {
                        tokens.colors.primary
                    } else {
                        tokens.colors.border
                    };

                    indicator_row.push(
                        Container::<fission_core::ui::Node>::lowered(
                            fission_core::ui::widgets::Spacer::default().into_node(),
                        )
                        .width(connector_width)
                        .height(2.0)
                        .bg(line_color)
                        .into_node(),
                    );
                    label_row.push(
                        fission_core::ui::widgets::Spacer {
                            width: Some(connector_width),
                            ..Default::default()
                        }
                        .into_node(),
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
                    .into_node(),
                    HStack {
                        spacing: Some(0.0),
                        children: label_row,
                    }
                    .into_node(),
                ],
            }
            .into_node()
        })
    }
}
