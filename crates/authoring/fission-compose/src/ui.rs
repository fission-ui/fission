//! Small authoring-level composites and editable-control bindings.
//!
//! Compose does not mirror Fission's widget catalogue. Named arguments become
//! ordinary Rust struct fields followed by `..Default::default()`, so any
//! imported widget with public fields and `Default` works without registration.
//! Positional arguments deliberately opt into that type's existing `new`.

use crate::{
    binding::{control_for, ControlSource},
    core::{self, ActionEnvelope, Widget},
    events::{Event, TextChanged, Toggle},
    ComposeBind,
};

pub struct Page {
    pub title: String,
    pub navigation: Option<Widget>,
    pub actions: Vec<Widget>,
    pub content: Option<Widget>,
    pub footer: Option<Widget>,
}

impl Page {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            navigation: None,
            actions: Vec::new(),
            content: None,
            footer: None,
        }
    }
}

impl From<Page> for Widget {
    fn from(value: Page) -> Self {
        let mut children = vec![core::ui::Text::new(value.title).size(28.0).into()];
        children.extend(value.navigation);
        children.push(value.content.expect("Compose Page requires content"));
        if !value.actions.is_empty() {
            children.push(
                core::ui::Row {
                    gap: Some(12.0),
                    children: value.actions,
                    ..Default::default()
                }
                .into(),
            );
        }
        children.extend(value.footer);
        core::ui::Column {
            gap: Some(20.0),
            children,
            ..Default::default()
        }
        .into()
    }
}

#[derive(Default)]
pub struct Form {
    pub children: Vec<Widget>,
}

impl Form {
    pub fn new() -> Self {
        Self {
            children: Vec::new(),
        }
    }
}

impl From<Form> for Widget {
    fn from(value: Form) -> Self {
        core::ui::Column {
            gap: Some(12.0),
            children: value.children,
            ..Default::default()
        }
        .into()
    }
}

pub struct TextField {
    pub label: String,
    pub semantics_identifier: Option<String>,
    pub value: String,
    pub placeholder: String,
    pub required: bool,
    pub enabled: bool,
    pub multiline: bool,
    pub autofocus: bool,
    pub on_submit: Option<ActionEnvelope>,
    pub on_input: Option<ActionEnvelope>,
}

impl TextField {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            semantics_identifier: None,
            value: String::new(),
            placeholder: String::new(),
            required: false,
            enabled: true,
            multiline: false,
            autofocus: false,
            on_submit: None,
            on_input: None,
        }
    }
}

impl<S> ComposeBind<S> for TextField
where
    S: ControlSource<String>,
{
    fn compose_bind(mut self, source: S) -> Self {
        assert!(
            self.on_input.is_none(),
            "TextField cannot have both bind and on_input"
        );
        let control = control_for::<String, TextChanged, _>("bind", source);
        self.value = control.value;
        self.on_input = control.on_change.map(Event::into_envelope);
        self
    }
}

impl From<TextField> for Widget {
    fn from(value: TextField) -> Self {
        core::ui::TextInput {
            semantics_identifier: value.semantics_identifier,
            value: value.value,
            label: Some(value.label.into()),
            placeholder: Some(value.placeholder.into()),
            required: value.required,
            enabled: value.enabled,
            multiline: value.multiline,
            autofocus: value.autofocus,
            read_only: value.on_input.is_none(),
            on_input: value.on_input,
            on_submit: value.on_submit,
            ..Default::default()
        }
        .into()
    }
}

/// A labelled Boolean control built from the existing Fission `Text` and
/// `Switch` widgets.
pub struct ToggleField {
    pub label: String,
    pub semantics_identifier: Option<String>,
    pub checked: bool,
    pub disabled: bool,
    pub on_toggle: Option<ActionEnvelope>,
}

impl ToggleField {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            semantics_identifier: None,
            checked: false,
            disabled: false,
            on_toggle: None,
        }
    }
}

impl<S> ComposeBind<S> for ToggleField
where
    S: ControlSource<bool>,
{
    fn compose_bind(mut self, source: S) -> Self {
        let control = control_for::<bool, Toggle, _>("bind", source);
        self.checked = control.value;
        self.on_toggle = control.on_change.map(Event::into_envelope);
        self
    }
}

impl From<ToggleField> for Widget {
    fn from(value: ToggleField) -> Self {
        let control: Widget = core::ui::Switch {
            semantics_identifier: value.semantics_identifier,
            checked: value.checked,
            disabled: value.disabled || value.on_toggle.is_none(),
            on_toggle: value.on_toggle,
            ..Default::default()
        }
        .into();
        core::ui::Row {
            gap: Some(8.0),
            children: vec![core::ui::Text::new(value.label).into(), control],
            ..Default::default()
        }
        .into()
    }
}

// Only editable behaviour needs an explicit semantic contract. These impls do
// not register widgets with Compose; any core or third-party widget remains
// freely composable through its normal Rust constructor API.
impl<S> ComposeBind<S> for core::ui::TextInput
where
    S: ControlSource<String>,
{
    fn compose_bind(mut self, source: S) -> Self {
        let control = control_for::<String, TextChanged, _>("bind", source);
        self.value = control.value;
        self.read_only = control.on_change.is_none();
        self.on_input = control.on_change.map(Event::into_envelope);
        self
    }
}

impl<S> ComposeBind<S> for core::ui::Switch
where
    S: ControlSource<bool>,
{
    fn compose_bind(mut self, source: S) -> Self {
        let control = control_for::<bool, Toggle, _>("bind", source);
        self.checked = control.value;
        self.disabled |= control.on_change.is_none();
        self.on_toggle = control.on_change.map(Event::into_envelope);
        self
    }
}
