use crate::stack::HStack;
use fission_core::action::ActionEnvelope;
use fission_core::ui::{Button, ButtonVariant, Container, Text, TextContent};
use fission_core::{BuildCtx, View, Widget};
use serde::{Deserialize, Serialize};

/// A pill-shaped label with an optional close button.
///
/// Tags are typically used for removable filters, categories, or selections.
/// The close button (an "x" character) appears when `on_close` is provided.
#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct Tag {
    pub label: String,
    pub on_close: Option<ActionEnvelope>,
}

impl<S: fission_core::AppState> Widget<S> for Tag {
    fn build(&self, ctx: &mut BuildCtx<S>, view: &View<S>) -> impl fission_core::IntoWidget<S> {
        fission_core::view::internal_node_widget({
            let tokens = &view.env.theme.tokens;

            let mut children = vec![Text {
                content: TextContent::Literal(self.label.clone()),
                font_size: Some(13.0),
                color: Some(tokens.colors.text_primary),
                ..Default::default()
            }
            .into()];

            if let Some(action) = &self.on_close {
                children.push(
                    Button::<fission_core::ui::Node> {
                        variant: ButtonVariant::Ghost,
                        child: Some(Box::new(
                            Text {
                                content: TextContent::Literal("×".into()),
                                font_size: Some(14.0),
                                color: Some(tokens.colors.text_secondary),
                                ..Default::default()
                            }
                            .into(),
                        )),
                        on_press: Some(action.clone()),
                        // Minimal styling for close button
                        width: Some(20.0),
                        height: Some(20.0),
                        ..Default::default()
                    }
                    .into(),
                );
            }

            Container::<fission_core::ui::Node>::lowered(fission_core::view::lower_widget_to_node(
                &HStack {
                    spacing: Some(4.0),
                    children,
                },
                ctx,
                view,
            ))
            .bg(tokens.colors.surface) // or slightly darker
            .border(tokens.colors.border, 1.0)
            .border_radius(16.0)
            .padding_all(6.0)
            .height(30.0)
            .into_node()
        })
    }
}
