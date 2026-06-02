use fission_core::op::Color;
use fission_core::ui::{Container, Text, Widget};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Code {
    pub text: String,
}

impl From<Code> for Widget {
    fn from(component: Code) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let this = &component;

        let tokens = &view.env().theme.tokens;
        Container::new(
            Text::new(this.text.clone())
                .size(12.0) // Monospace usually smaller?
                .color(tokens.colors.text_primary),
        )
        .bg(Color {
            r: 240,
            g: 240,
            b: 240,
            a: 255,
        })
        .padding_all(2.0)
        .border_radius(4.0)
        .into()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Kbd {
    pub text: String,
}

impl From<Kbd> for Widget {
    fn from(component: Kbd) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let this = &component;

        let tokens = &view.env().theme.tokens;
        Container::new(
            Text::new(this.text.clone())
                .size(12.0)
                .color(tokens.colors.text_primary),
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
        .into()
    }
}
