use fission_core::ui::{Button, ButtonVariant, Text, TextInput, Widget};
use fission_core::{ActionEnvelope, WidgetId};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Editable {
    pub id: Option<WidgetId>,
    pub value: String,
    pub placeholder: String,
    pub is_editing: bool,
    pub on_change: Option<ActionEnvelope>,
    pub on_submit: Option<ActionEnvelope>, // Enter key
    pub on_edit: Option<ActionEnvelope>,   // Click to edit
    pub on_cancel: Option<ActionEnvelope>, // Esc or blur
}

impl From<Editable> for Widget {
    fn from(component: Editable) -> Self {
        let mut component = component;
        component.id = fission_core::build::current_widget_id().or(component.id);
        let this = &component;

        if this.is_editing {
            let input_id = this
                .id
                .as_ref()
                .map(|id| WidgetId::derived(id.as_u128(), &[0]));
            TextInput {
                id: input_id.map(Into::into),
                value: this.value.clone(),
                placeholder: Some(this.placeholder.clone().into()),
                on_change: this.on_change.clone(),
                // TODO: on_submit (Enter) and on_cancel (Esc/Blur) support in TextInput semantics?
                // Currently TextInput semantics supports `actions` but specific triggers like Enter are handled by Runtime key events dispatching first semantics action.
                // If we want Enter to submit, we should make sure `on_submit` is the primary action?
                // TextInput semantic role is TextInput.
                // We might need to wrap it or rely on focus/blur.
                ..Default::default()
            }
            .into()
        } else {
            Button {
                variant: ButtonVariant::Ghost,
                child: Some(
                    Text::new(if this.value.is_empty() {
                        this.placeholder.clone()
                    } else {
                        this.value.clone()
                    })
                    .into(),
                ),
                on_press: this.on_edit.clone(),
                ..Default::default()
            }
            .into()
        }
    }
}
