use fission_core::ui::{Button, ButtonVariant, Text};
use fission_core::{ActionEnvelope, BuildCtx, View, Widget};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Link {
    pub text: String,
    pub on_click: Option<ActionEnvelope>,
}

impl<S: fission_core::AppState> Widget<S> for Link {
    fn build(&self, _ctx: &mut BuildCtx<S>, view: &View<S>) -> impl fission_core::IntoWidget<S> {
        fission_core::AnyWidget::from_node({
            let tokens = &view.env.theme.tokens;

            Button {
                variant: ButtonVariant::Ghost,
                child: Some(Box::new(
                    Text::new(self.text.clone())
                        .color(tokens.colors.primary)
                        .underline(true)
                        .into_node(),
                )),
                on_press: self.on_click.clone(),
                content_align: fission_core::ui::ButtonContentAlign::Start,
                padding: Some([0.0; 4]), // Minimal padding
                ..Default::default()
            }
            .into_node()
        })
    }
}
