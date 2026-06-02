use fission_core::ui::{Button, ButtonVariant, Text, Widget};
use fission_core::ActionEnvelope;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Link {
    pub text: String,
    pub on_click: Option<ActionEnvelope>,
}

impl From<Link> for Widget {
    fn from(component: Link) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let this = &component;

        let tokens = &view.env().theme.tokens;

        Button {
            variant: ButtonVariant::Ghost,
            child: Some(
                Text::new(this.text.clone())
                    .color(tokens.colors.primary)
                    .underline(true)
                    .into(),
            ),
            on_press: this.on_click.clone(),
            content_align: fission_core::ui::ButtonContentAlign::Start,
            padding: Some([0.0; 4]), // Minimal padding
            ..Default::default()
        }
        .into()
    }
}
