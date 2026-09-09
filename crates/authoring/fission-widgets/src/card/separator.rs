use fission_core::op::Fill;
use fission_core::ui::{Container, Row, Widget};

/// Design-system source for a fixed-height card-region boundary.
#[derive(Clone, Copy, Debug, Default)]
pub(super) enum CardSeparatorRecipe {
    /// General boundary between adjacent named regions.
    #[default]
    Section,
    /// Footer-specific top boundary.
    Footer,
}

/// A fixed-height separator for the vertical card-region stack.
///
/// The general-purpose divider grows along its parent axis; card separators
/// must instead stay exactly one point tall when the card receives extra
/// vertical space. The card layout may override its symmetric horizontal inset;
/// otherwise the selected recipe owns its margin.
#[derive(Clone, Debug, Default)]
pub(super) struct CardSeparator {
    recipe: CardSeparatorRecipe,
    inset: Option<f32>,
}

impl CardSeparator {
    pub(super) fn section(inset: Option<f32>) -> Self {
        Self {
            recipe: CardSeparatorRecipe::Section,
            inset,
        }
    }

    pub(super) fn footer(inset: Option<f32>) -> Self {
        Self {
            recipe: CardSeparatorRecipe::Footer,
            inset,
        }
    }
}

impl From<CardSeparator> for Widget {
    fn from(component: CardSeparator) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let theme = &view.env().theme.components.card;
        let (style, prefer_border) = match component.recipe {
            CardSeparatorRecipe::Section => (&theme.separator_style, false),
            CardSeparatorRecipe::Footer => (&theme.footer_style, true),
        };
        let width = style.border.as_ref().map_or(1.0, |border| border.width);
        let fill = if prefer_border {
            style
                .border
                .as_ref()
                .map(|border| border.fill.clone())
                .or_else(|| style.background.clone())
        } else {
            style
                .background
                .clone()
                .or_else(|| style.border.as_ref().map(|border| border.fill.clone()))
        };

        let mut separator = Container::new(Row::default())
            .height(width)
            .flex_grow(0.0)
            .flex_shrink(0.0);
        if let Some(inset) = component.inset {
            let recipe_margin = style.margin.unwrap_or([0.0; 4]);
            separator = separator.margin([inset, inset, recipe_margin[2], recipe_margin[3]]);
        } else if let Some(margin) = style.margin {
            separator = separator.margin(margin);
        }
        separator =
            separator.bg_fill(fill.unwrap_or(Fill::Solid(view.env().theme.tokens.colors.border)));
        separator.into()
    }
}
