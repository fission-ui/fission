use crate::stack::VStack;
use fission_core::ui::{Text, Widget};
use fission_core::WidgetId;
use serde::{Deserialize, Serialize};

/// A form field wrapper that adds a label, error message, and helper text.
///
/// Wraps any child widget (typically a `TextInput`, `Select`, or `Combobox`)
/// with a vertical stack containing:
/// 1. An optional label (with a required-field asterisk when `required` is `true`).
/// 2. The child widget.
/// 3. An error message (red) or helper text (secondary color).
///
/// # Example
///
/// ```rust,ignore
/// FormControl {
///     id: None,
///     label: Some("Email".into()),
///     child: text_input_node,
///     error: if invalid { Some("Invalid email".into()) } else { None },
///     helper: Some("We'll never share your email.".into()),
///     required: true,
/// }
/// ```
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FormControl {
    pub id: Option<WidgetId>,
    pub label: Option<String>,
    pub child: Widget,
    pub error: Option<String>,
    pub helper: Option<String>,
    pub required: bool,
}

impl From<FormControl> for Widget {
    fn from(component: FormControl) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let mut component = component;
        component.id = fission_core::build::current_widget_id().or(component.id);
        let this = &component;

        let tokens = &view.env().theme.tokens;
        let mut children = Vec::new();

        if let Some(label_text) = &this.label {
            let display_text = if this.required {
                format!("{} *", label_text)
            } else {
                label_text.clone()
            };

            children.push(
                Text::new(display_text)
                    .size(tokens.typography.label_large_size)
                    .color(tokens.colors.text_primary)
                    .into(),
            );
        }

        children.push(this.child.clone());

        if let Some(err) = &this.error {
            children.push(
                Text::new(err.clone())
                    .size(12.0)
                    .color(tokens.colors.error)
                    .into(),
            );
        } else if let Some(help) = &this.helper {
            children.push(
                Text::new(help.clone())
                    .size(12.0)
                    .color(tokens.colors.text_secondary)
                    .into(),
            );
        }

        let mut root: Widget = VStack {
            spacing: Some(4.0),
            children,
        }
        .into();
        if let Some(id) = this.id {
            root = root.id(id);
        }
        root
    }
}
