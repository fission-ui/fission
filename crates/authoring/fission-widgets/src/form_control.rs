use fission_core::ui::{Column, SemanticsRegion, Text, TextContent, Widget};
use fission_core::WidgetId;
use fission_ir::{Role, Semantics};
use serde::{Deserialize, Serialize};

const IMPLICIT_FORM_CONTROL_ID_SALT: u32 = 0x464F_524D;
// Keep explicit anatomy identities outside the control's ordinary lowering
// sequence. Short positional paths can collide with internal layout nodes
// allocated beneath the same control identity.
const LABEL_ID_PATH: &[u32] = &[0x4c41_424c];
const DESCRIPTION_ID_PATH: &[u32] = &[0x4445_5343];
const ERROR_ID_PATH: &[u32] = &[0x4552_524f];

/// Retained label anatomy for a [`FormControlLayout`].
///
/// Literal or localized text receives the active field-label recipe. Custom
/// retained presentation may be supplied without losing the accessible label.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FormLabel {
    /// Localized or literal accessible label.
    pub text: TextContent,
    /// Optional custom retained visual presentation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub child: Option<Widget>,
    /// Optional explicit semantic identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<WidgetId>,
    /// Optional stable identifier exposed to shells and semantic tests.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantics_identifier: Option<String>,
}

impl FormLabel {
    /// Creates a design-system-styled field label.
    pub fn new(text: impl Into<TextContent>) -> Self {
        Self {
            text: text.into(),
            child: None,
            id: None,
            semantics_identifier: None,
        }
    }

    /// Uses custom retained presentation while preserving `text` as the
    /// accessible label.
    pub fn custom(text: impl Into<TextContent>, child: impl Into<Widget>) -> Self {
        Self {
            child: Some(child.into()),
            ..Self::new(text)
        }
    }

    /// Uses an explicit stable semantic identity.
    pub fn id(mut self, id: WidgetId) -> Self {
        self.id = Some(id);
        self
    }

    /// Exposes a stable identifier to accessibility and semantic tests.
    pub fn semantics_identifier(mut self, identifier: impl Into<String>) -> Self {
        self.semantics_identifier = Some(identifier.into());
        self
    }
}

impl From<FormLabel> for Widget {
    fn from(label: FormLabel) -> Self {
        FormLabelRegion {
            label,
            required: false,
            invalid: false,
        }
        .into()
    }
}

/// Retained helper-description anatomy for a [`FormControlLayout`].
///
/// The text is associated with the actual descendant form control through
/// `described_by`, including when custom retained presentation is used.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FormDescription {
    /// Localized or literal accessible description.
    pub text: TextContent,
    /// Optional custom retained visual presentation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub child: Option<Widget>,
    /// Optional explicit semantic identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<WidgetId>,
    /// Optional stable identifier exposed to shells and semantic tests.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantics_identifier: Option<String>,
}

impl FormDescription {
    /// Creates a design-system-styled helper description.
    pub fn new(text: impl Into<TextContent>) -> Self {
        Self {
            text: text.into(),
            child: None,
            id: None,
            semantics_identifier: None,
        }
    }

    /// Uses custom retained presentation while preserving accessible text.
    pub fn custom(text: impl Into<TextContent>, child: impl Into<Widget>) -> Self {
        Self {
            child: Some(child.into()),
            ..Self::new(text)
        }
    }

    /// Uses an explicit stable semantic identity.
    pub fn id(mut self, id: WidgetId) -> Self {
        self.id = Some(id);
        self
    }

    /// Exposes a stable identifier to accessibility and semantic tests.
    pub fn semantics_identifier(mut self, identifier: impl Into<String>) -> Self {
        self.semantics_identifier = Some(identifier.into());
        self
    }
}

impl From<FormDescription> for Widget {
    fn from(description: FormDescription) -> Self {
        FormMessageRegion::description(description).into()
    }
}

/// Retained validation-error anatomy for a [`FormControlLayout`].
///
/// The error is both visible and installed as the descendant control's
/// authoritative invalid state and validation message.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FormError {
    /// Localized or literal validation message.
    pub text: TextContent,
    /// Optional custom retained visual presentation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub child: Option<Widget>,
    /// Optional explicit semantic identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<WidgetId>,
    /// Optional stable identifier exposed to shells and semantic tests.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantics_identifier: Option<String>,
}

impl FormError {
    /// Creates a design-system-styled validation message.
    pub fn new(text: impl Into<TextContent>) -> Self {
        Self {
            text: text.into(),
            child: None,
            id: None,
            semantics_identifier: None,
        }
    }

    /// Uses custom retained presentation while preserving accessible text.
    pub fn custom(text: impl Into<TextContent>, child: impl Into<Widget>) -> Self {
        Self {
            child: Some(child.into()),
            ..Self::new(text)
        }
    }

    /// Uses an explicit stable semantic identity.
    pub fn id(mut self, id: WidgetId) -> Self {
        self.id = Some(id);
        self
    }

    /// Exposes a stable identifier to accessibility and semantic tests.
    pub fn semantics_identifier(mut self, identifier: impl Into<String>) -> Self {
        self.semantics_identifier = Some(identifier.into());
        self
    }
}

impl From<FormError> for Widget {
    fn from(error: FormError) -> Self {
        FormMessageRegion::error(error).into()
    }
}

/// A complete labelled form field assembled from retained named anatomy.
///
/// Label, control, helper description, and validation error keep separate
/// identities and semantic relationships. When both description and error are
/// supplied, both remain visible and describe the control; the error alone
/// supplies its invalid state. [`FormControl`] remains the compact compatibility
/// API and lowers through this same recipe.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FormControlLayout {
    /// Stable identity for the complete field group.
    pub id: WidgetId,
    /// Optional retained label anatomy.
    pub label: Option<FormLabel>,
    /// The actual input or other semantic form control.
    pub control: Widget,
    /// Optional non-error helper description.
    pub description: Option<FormDescription>,
    /// Optional validation error.
    pub error: Option<FormError>,
    /// Whether the associated field is required.
    pub required: bool,
}

impl FormControlLayout {
    /// Creates an unlabelled field group around `control`.
    pub fn new(id: WidgetId, control: impl Into<Widget>) -> Self {
        Self {
            id,
            label: None,
            control: control.into(),
            description: None,
            error: None,
            required: false,
        }
    }

    /// Adds retained label anatomy.
    pub fn label(mut self, label: FormLabel) -> Self {
        self.label = Some(label);
        self
    }

    /// Adds retained helper-description anatomy.
    pub fn description(mut self, description: FormDescription) -> Self {
        self.description = Some(description);
        self
    }

    /// Adds retained validation-error anatomy.
    pub fn error(mut self, error: FormError) -> Self {
        self.error = Some(error);
        self
    }

    /// Sets the associated field's required state.
    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }
}

/// A form field wrapper that adds a label, error message, and helper text.
///
/// This source-compatible convenience API displays helper text only while no
/// error is present and lowers into [`FormControlLayout`], [`FormLabel`],
/// [`FormDescription`], and [`FormError`]. Use the named anatomy directly for
/// localized text, custom retained presentation, or simultaneous helper and
/// validation messages.
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
    /// Optional stable identity for the complete labelled field group.
    pub id: Option<WidgetId>,
    /// Optional user-facing field label.
    pub label: Option<String>,
    /// Input or other form field associated with the label and messages.
    pub child: Widget,
    /// Validation message; when present it takes precedence over helper text.
    pub error: Option<String>,
    /// Optional guidance shown while no validation error is present.
    pub helper: Option<String>,
    /// Whether to mark the label as required.
    pub required: bool,
}

#[derive(Clone, Debug)]
struct FormControlRecipe {
    id: Option<WidgetId>,
    label: Option<FormLabel>,
    control: Widget,
    description: Option<FormDescription>,
    error: Option<FormError>,
    required: bool,
    legacy_message_identity: bool,
}

impl From<FormControl> for Widget {
    fn from(component: FormControl) -> Self {
        let has_error = component.error.is_some();
        FormControlRecipe {
            id: component.id,
            label: component.label.map(FormLabel::new),
            control: component.child,
            description: (!has_error)
                .then_some(component.helper)
                .flatten()
                .map(FormDescription::new),
            error: component.error.map(FormError::new),
            required: component.required,
            legacy_message_identity: true,
        }
        .into()
    }
}

impl From<FormControlLayout> for Widget {
    fn from(component: FormControlLayout) -> Self {
        FormControlRecipe {
            id: Some(component.id),
            label: component.label,
            control: component.control,
            description: component.description,
            error: component.error,
            required: component.required,
            legacy_message_identity: false,
        }
        .into()
    }
}

#[derive(Clone, Debug)]
struct FormLabelRegion {
    label: FormLabel,
    required: bool,
    invalid: bool,
}

impl From<FormLabelRegion> for Widget {
    fn from(region: FormLabelRegion) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let input_theme = &view.env().theme.components.text_input;
        let tokens = &view.env().theme.tokens;
        let style = &input_theme.label_style;
        let resolved = region.label.text.resolve(view.env());
        let child = region.label.child.unwrap_or_else(|| {
            Text::new(if region.required {
                format!("{resolved} *")
            } else {
                resolved.clone()
            })
            .size(style.font_size.unwrap_or(input_theme.font_size))
            .weight(
                style
                    .font_weight
                    .unwrap_or(tokens.typography.font_weight_medium),
            )
            .line_height(
                style
                    .line_height
                    .unwrap_or(style.font_size.unwrap_or(input_theme.font_size)),
            )
            .color(if region.invalid {
                tokens.colors.error
            } else {
                style.text_color.unwrap_or(tokens.colors.text_primary)
            })
            .into()
        });

        SemanticsRegion {
            id: region.label.id,
            identifier: region.label.semantics_identifier,
            label: Some(resolved),
            role: Role::Text,
            focusable: Some(false),
            sequential_focusable: false,
            child: Some(child),
            ..Default::default()
        }
        .into()
    }
}

#[derive(Clone, Debug)]
enum FormMessageRegion {
    Description(FormDescription),
    Error(FormError),
}

impl FormMessageRegion {
    fn description(description: FormDescription) -> Self {
        Self::Description(description)
    }

    fn error(error: FormError) -> Self {
        Self::Error(error)
    }
}

impl From<FormMessageRegion> for Widget {
    fn from(region: FormMessageRegion) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let input_theme = &view.env().theme.components.text_input;
        let tokens = &view.env().theme.tokens;
        let style = &input_theme.helper_style;
        let (text, child, id, semantics_identifier, color) = match region {
            FormMessageRegion::Description(description) => (
                description.text,
                description.child,
                description.id,
                description.semantics_identifier,
                style.text_color.unwrap_or(tokens.colors.text_muted),
            ),
            FormMessageRegion::Error(error) => (
                error.text,
                error.child,
                error.id,
                error.semantics_identifier,
                tokens.colors.error,
            ),
        };
        let resolved = text.resolve(view.env());
        let child = child.unwrap_or_else(|| {
            Text::new(text)
                .size(style.font_size.unwrap_or(input_theme.font_size))
                .weight(
                    style
                        .font_weight
                        .unwrap_or(tokens.typography.font_weight_regular),
                )
                .line_height(style.line_height.unwrap_or(input_theme.line_height))
                .color(color)
                .into()
        });

        SemanticsRegion {
            id,
            identifier: semantics_identifier,
            label: Some(resolved),
            role: Role::Text,
            focusable: Some(false),
            sequential_focusable: false,
            child: Some(child),
            ..Default::default()
        }
        .into()
    }
}

impl From<FormControlRecipe> for Widget {
    fn from(mut component: FormControlRecipe) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        component.id = fission_core::build::current_widget_id()
            .or(component.id)
            .or_else(|| {
                fission_core::build::next_implicit_widget_id(IMPLICIT_FORM_CONTROL_ID_SALT)
            });
        let control_id = component
            .id
            .unwrap_or_else(|| WidgetId::explicit("fission.widgets.form-control"));
        let label_id = WidgetId::derived(control_id.as_u128(), LABEL_ID_PATH);
        let description_id = WidgetId::derived(control_id.as_u128(), DESCRIPTION_ID_PATH);
        let error_id = WidgetId::derived(
            control_id.as_u128(),
            if component.legacy_message_identity {
                DESCRIPTION_ID_PATH
            } else {
                ERROR_ID_PATH
            },
        );
        let mut children = Vec::new();
        let mut labelled_by = Vec::new();
        let mut described_by = Vec::new();
        let mut invalid_message = None;

        if let Some(mut label) = component.label {
            let id = label.id.unwrap_or(label_id);
            label.id = Some(id);
            children.push(
                FormLabelRegion {
                    label,
                    required: component.required,
                    invalid: component.error.is_some(),
                }
                .into(),
            );
            labelled_by.push(id);
        }

        let control_index = children.len();
        if let Some(mut description) = component.description {
            let id = description.id.unwrap_or(description_id);
            description.id = Some(id);
            children.push(FormMessageRegion::description(description).into());
            described_by.push(id);
        }
        if let Some(mut error) = component.error {
            let id = error.id.unwrap_or(error_id);
            invalid_message = Some(error.text.resolve(view.env()));
            error.id = Some(id);
            children.push(FormMessageRegion::error(error).into());
            described_by.push(id);
        }

        let control = component.control.with_form_field_relationships(
            labelled_by.clone(),
            described_by.clone(),
            component.required,
            invalid_message,
        );
        children.insert(control_index, control);

        Column {
            id: Some(control_id),
            children,
            semantics: Some(Semantics {
                role: Role::Group,
                labelled_by,
                described_by,
                ..Default::default()
            }),
            gap: Some(view.env().theme.tokens.spacing.s),
            ..Default::default()
        }
        .into()
    }
}
