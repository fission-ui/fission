use fission_core::ui::{Container, Node};
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
}

impl Default for Card {
    fn default() -> Self {
        Self {
            child: Box::new(fission_core::ui::Row::default().into()),
        }
    }
}

impl<S: fission_core::AppState> Widget<S> for Card {
    fn build(&self, _ctx: &mut BuildCtx<S>, view: &View<S>) -> Node {
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

        Container::new(*self.child.clone())
            .bg(tokens.colors.surface)
            .border_radius(tokens.radii.medium)
            .shadow(tokens.elevations.level1.unwrap_or(default_shadow))
            .padding_all(tokens.spacing.m)
            .into_node()
    }
}
