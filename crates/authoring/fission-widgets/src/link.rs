use fission_core::ui::{Button, ButtonVariant, Node, Text};
use fission_core::{BuildCtx, View, Widget, ActionEnvelope};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Link {
    pub text: String,
    pub on_click: Option<ActionEnvelope>,
}

impl<S: fission_core::AppState> Widget<S> for Link {
    fn build(&self, _ctx: &mut BuildCtx<S>, view: &View<S>) -> Node {
        let tokens = &view.env.theme.tokens;
        
        Button {
            variant: ButtonVariant::Ghost,
            child: Some(Box::new(
                Text::new(self.text.clone())
                    .color(tokens.colors.primary)
                    .underline(true)
                    .into_node()
            )),
            on_press: self.on_click.clone(),
            content_align: fission_core::ui::ButtonContentAlign::Start,
            padding: Some([0.0; 4]), // Minimal padding
            ..Default::default()
        }.into_node()
    }
}
