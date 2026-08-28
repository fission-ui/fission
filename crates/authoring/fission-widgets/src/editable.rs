use fission_core::ui::{Button, ButtonVariant, Text, TextInput, Widget};
use fission_core::{ActionEnvelope, WidgetId};
use serde::{Deserialize, Serialize};

/// Inline value that switches between read and edit presentations.
///
/// The application owns `is_editing` and `value`; actions let reducers enter
/// editing and accept or cancel changes. This keeps the editable value in the
/// normal retained Fission state flow rather than hidden inside the widget.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Editable {
    /// Optional stable identity used to derive the inner text field identity.
    pub id: Option<WidgetId>,
    /// Current controlled text value.
    pub value: String,
    /// Text shown when `value` is empty and as the editor placeholder.
    pub placeholder: String,
    /// Whether to render the text editor instead of the read-only button.
    pub is_editing: bool,
    /// Action dispatched when the editor text changes. The new text and
    /// selection are available through `ReducerContext::input.text_change()`.
    pub on_input: Option<ActionEnvelope>,
    /// Action intended to accept the current edit.
    pub on_submit: Option<ActionEnvelope>,
    /// Action dispatched from the read presentation to enter editing.
    pub on_edit: Option<ActionEnvelope>,
    /// Action intended to abandon editing and restore application state.
    pub on_cancel: Option<ActionEnvelope>,
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
                on_input: this.on_input.clone(),
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
