use crate::stack::VStack;
use fission_core::ui::{Node, Text};
use fission_core::{BuildCtx, View, Widget, WidgetNodeId};
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
///     child: Box::new(text_input_node),
///     error: if invalid { Some("Invalid email".into()) } else { None },
///     helper: Some("We'll never share your email.".into()),
///     required: true,
/// }
/// ```
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FormControl {
    pub id: Option<WidgetNodeId>,
    pub label: Option<String>,
    pub child: Box<Node>,
    pub error: Option<String>,
    pub helper: Option<String>,
    pub required: bool,
}

impl<S: fission_core::AppState> Widget<S> for FormControl {
    fn build(&self, ctx: &mut BuildCtx<S>, view: &View<S>) -> Node {
        let tokens = &view.env.theme.tokens;
        let mut children = Vec::new();

        if let Some(label_text) = &self.label {
            let display_text = if self.required {
                format!("{} *", label_text)
            } else {
                label_text.clone()
            };

            children.push(
                Text::new(display_text)
                    .size(tokens.typography.label_large_size)
                    .color(tokens.colors.text_primary)
                    .into_node(),
            );
        }

        children.push(*self.child.clone());

        if let Some(err) = &self.error {
            children.push(
                Text::new(err.clone())
                    .size(12.0)
                    .color(tokens.colors.error)
                    .into_node(),
            );
        } else if let Some(help) = &self.helper {
            children.push(
                Text::new(help.clone())
                    .size(12.0)
                    .color(tokens.colors.text_secondary)
                    .into_node(),
            );
        }

        VStack {
            spacing: Some(4.0),
            children,
        }
        .build(ctx, view)
    }
}
