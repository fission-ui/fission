use fission_core::op::Fill;
use fission_core::ui::{CardPattern, Container, Widget};
use serde::{Deserialize, Serialize};

/// An elevated surface container with rounded corners and a box shadow.
///
/// Cards provide a visual grouping for related content. They use the theme's
/// `surface` background color, `medium` border radius, and `level1` elevation
/// shadow. Content is padded with the `spacing.m` (16px) token.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Card {
    pub child: Widget,
    pub pattern: CardPattern,
    pub interactive: bool,
}

impl Default for Card {
    fn default() -> Self {
        Self {
            child: fission_core::ui::Row::default().into(),
            pattern: CardPattern::Raised,
            interactive: false,
        }
    }
}

impl From<Card> for Widget {
    fn from(component: Card) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let this = &component;

        let theme = &view.env().theme.components.card;
        let style = theme.resolve(this.pattern, this.interactive);
        let tokens = &view.env().theme.tokens;
        let default_shadow = fission_core::op::BoxShadow {
            color: fission_core::op::Color {
                r: 0,
                g: 0,
                b: 0,
                a: 20,
            },
            blur_radius: 2.0,
            offset: (0.0, 1.0),
        };

        let mut card = Container::new(this.child.clone())
            .bg_fill(
                style
                    .background
                    .clone()
                    .unwrap_or(Fill::Solid(tokens.colors.surface)),
            )
            .border_radius(style.radius.unwrap_or(theme.radius))
            .shadows(style.outer_shadows())
            .padding(style.padding_box(theme.padding, theme.padding));
        if let Some(border) = style.border {
            if let Fill::Solid(color) = border.fill {
                card = card.border(color, border.width);
            }
        }
        if style.shadows.is_empty() {
            card = card.shadow(tokens.elevations.level1.unwrap_or(default_shadow));
        }
        card.into()
    }
}
