use crate::stack::HStack;
use fission_core::action::ActionEnvelope;
use fission_core::ui::{
    Button, ButtonVariant, Container, SemanticsRegion, Text, TextContent, Widget,
};
use serde::{Deserialize, Serialize};

/// A pill-shaped label with an optional close button.
///
/// Tags are typically used for removable filters, categories, or selections.
/// The close button (an "x" character) appears when `on_close` is provided.
#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct Tag {
    /// Text displayed inside the tag.
    pub label: String,
    /// Optional removal action; supplying it also displays the close affordance.
    pub on_close: Option<ActionEnvelope>,
}

impl From<Tag> for Widget {
    fn from(component: Tag) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let this = &component;

        let tokens = &view.env().theme.tokens;
        let recipe = view.env().theme.recipe(fission_theme::recipe_names::TAG);

        let mut children = vec![Text {
            content: TextContent::Literal(this.label.clone()),
            font_size: Some(13.0),
            color: Some(tokens.colors.text_primary),
            ..Default::default()
        }
        .into()];

        if let Some(action) = &this.on_close {
            children.push(
                SemanticsRegion::new(Button {
                    variant: ButtonVariant::Ghost,
                    child: Some(
                        Text {
                            content: TextContent::Literal("×".into()),
                            font_size: Some(14.0),
                            color: Some(tokens.colors.text_secondary),
                            ..Default::default()
                        }
                        .into(),
                    ),
                    on_press: Some(action.clone()),
                    // Minimal styling for close button
                    width: Some(20.0),
                    height: Some(20.0),
                    ..Default::default()
                })
                // A bare multiplication sign announces nothing. Name the action
                // and what it removes.
                .label(format!("Remove {}", this.label))
                .into(),
            );
        }

        Container::new(HStack {
            spacing: Some(4.0),
            children,
        })
        .bg_fill(
            recipe
                .base
                .background
                .clone()
                .unwrap_or(fission_core::op::Fill::Solid(tokens.colors.surface)),
        )
        .border(tokens.colors.border, 1.0)
        .border_radius(recipe.base.radius.unwrap_or(tokens.radii.full))
        .padding(
            recipe
                .base
                .padding_box(tokens.spacing.xs, tokens.spacing.xs / 2.0),
        )
        .height(recipe.base.height.unwrap_or(24.0))
        .into()
    }
}
