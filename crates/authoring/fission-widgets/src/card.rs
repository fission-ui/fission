use fission_core::op::Fill;
use fission_core::ui::{CardPattern, Container, Node};
use fission_core::{BuildCtx, View, Widget};
use serde::{Deserialize, Serialize};

/// An elevated surface container with rounded corners and a box shadow.
///
/// Cards provide a visual grouping for related content. They use the theme's
/// `surface` background color, `medium` border radius, and `level1` elevation
/// shadow. Content is padded with the `spacing.m` (16px) token.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Card {
    pub child: Box<Node>,
    pub pattern: CardPattern,
    pub interactive: bool,
}

impl Default for Card {
    fn default() -> Self {
        Self {
            child: Box::new(fission_core::ui::Row::<fission_core::ui::Node>::default().into()),
            pattern: CardPattern::Raised,
            interactive: false,
        }
    }
}

impl<S: fission_core::AppState> Widget<S> for Card {
    fn build(&self, _ctx: &mut BuildCtx<S>, view: &View<S>) -> impl fission_core::IntoWidget<S> {
        fission_core::view::internal_node_widget({
            let theme = &view.env.theme.components.card;
            let style = theme.resolve(self.pattern, self.interactive);
            let tokens = &view.env.theme.tokens;
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

            let mut card = Container::<fission_core::ui::Node>::lowered(*self.child.clone())
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
            card.into_node()
        })
    }
}
