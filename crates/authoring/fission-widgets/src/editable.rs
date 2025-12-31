use fission_core::ui::{Node, Text, TextInput, Button, ButtonVariant};
use fission_core::{BuildCtx, View, Widget, ActionEnvelope};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Editable {
    pub value: String,
    pub placeholder: String,
    pub is_editing: bool,
    pub on_change: Option<ActionEnvelope>,
    pub on_submit: Option<ActionEnvelope>, // Enter key
    pub on_edit: Option<ActionEnvelope>,   // Click to edit
    pub on_cancel: Option<ActionEnvelope>, // Esc or blur
}

impl<S: fission_core::AppState> Widget<S> for Editable {
    fn build(&self, _ctx: &mut BuildCtx<S>, _view: &View<S>) -> Node {
        if self.is_editing {
            TextInput {
                value: self.value.clone(),
                placeholder: Some(self.placeholder.clone().into()),
                on_change: self.on_change.clone(),
                // TODO: on_submit (Enter) and on_cancel (Esc/Blur) support in TextInput semantics?
                // Currently TextInput semantics supports `actions` but specific triggers like Enter are handled by Runtime key events dispatching first semantics action.
                // If we want Enter to submit, we should make sure `on_submit` is the primary action?
                // TextInput semantic role is TextInput.
                // We might need to wrap it or rely on focus/blur.
                ..Default::default()
            }.into_node()
        } else {
            Button {
                variant: ButtonVariant::Ghost,
                child: Some(Box::new(
                    Text::new(if self.value.is_empty() { self.placeholder.clone() } else { self.value.clone() })
                        .into_node()
                )),
                on_press: self.on_edit.clone(),
                ..Default::default()
            }.into_node()
        }
    }
}
