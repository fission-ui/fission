use fission_core::op::Color;
use fission_core::ui::{Container, Text};
use fission_core::{BuildCtx, View, Widget};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Code {
    pub text: String,
}

impl<S: fission_core::AppState> Widget<S> for Code {
    fn build(&self, _ctx: &mut BuildCtx<S>, view: &View<S>) -> impl fission_core::IntoWidget<S> {
        fission_core::view::internal_node_widget({
            let tokens = &view.env.theme.tokens;
            Container::<fission_core::ui::Node>::lowered(
                Text::new(self.text.clone())
                    .size(12.0) // Monospace usually smaller?
                    .color(tokens.colors.text_primary)
                    .into_node(),
            )
            .bg(Color {
                r: 240,
                g: 240,
                b: 240,
                a: 255,
            })
            .padding_all(2.0)
            .border_radius(4.0)
            .into_node()
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Kbd {
    pub text: String,
}

impl<S: fission_core::AppState> Widget<S> for Kbd {
    fn build(&self, _ctx: &mut BuildCtx<S>, view: &View<S>) -> impl fission_core::IntoWidget<S> {
        fission_core::view::internal_node_widget({
            let tokens = &view.env.theme.tokens;
            Container::<fission_core::ui::Node>::lowered(
                Text::new(self.text.clone())
                    .size(12.0)
                    .color(tokens.colors.text_primary)
                    .into_node(),
            )
            .bg(Color {
                r: 245,
                g: 245,
                b: 245,
                a: 255,
            })
            .border(
                Color {
                    r: 200,
                    g: 200,
                    b: 200,
                    a: 255,
                },
                1.0,
            )
            .border_radius(4.0)
            .padding_all(4.0)
            .into_node()
        })
    }
}
