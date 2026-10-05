use crate::stack::HStack;
use crate::Icon;
use fission_core::ui::{Button, ButtonVariant, SemanticsRegion, Text, Widget};
use fission_core::ActionEnvelope;
use fission_icons::material;
use fission_ir::Role;
use serde::{Deserialize, Serialize};

/// File-selection row containing a browse action and the selected file label.
///
/// The widget does not open a platform picker itself. Bind `on_browse` to a
/// reducer that requests Fission's file-picker capability and pass the returned
/// display name back through `selected_file`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileUpload {
    /// Text shown on the browse button.
    pub label: String,
    /// Optional selected file name shown beside the button.
    pub selected_file: Option<String>,
    /// Optional validation or picker error rendered below the selection row.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_text: Option<String>,
    /// Action dispatched when the browse button is pressed.
    pub on_browse: Option<ActionEnvelope>,
    /// Stable identifier exposed on the generated browse button.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub browse_semantics_identifier: Option<String>,
}

impl From<FileUpload> for Widget {
    fn from(component: FileUpload) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let this = &component;

        let tokens = &view.env().theme.tokens;
        let recipe = view.env().theme.recipe(fission_theme::recipes::FileUpload);

        let gap = recipe.base.gap.unwrap_or(tokens.spacing.s);
        let mut browse_button = Button {
            variant: ButtonVariant::Outline,
            child: Some(
                HStack {
                    spacing: Some(gap),
                    children: vec![
                        Icon::svg(material::file::folder_open::regular())
                            .size(view.env().theme.components.button.icon_size)
                            .into(),
                        Text::new(this.label.clone()).flex_shrink(0.0).into(),
                    ],
                }
                .into(),
            ),
            on_press: this.on_browse.clone(),
            ..Default::default()
        };
        if let Some(identifier) = &this.browse_semantics_identifier {
            browse_button = browse_button.semantics_identifier(identifier.clone());
        }

        let selection_row: Widget = HStack {
            spacing: Some(gap),
            children: vec![
                browse_button.into(),
                Text::new(
                    this.selected_file
                        .clone()
                        .unwrap_or("No file selected".into()),
                )
                .color(if this.selected_file.is_some() {
                    tokens.colors.text_primary
                } else {
                    tokens.colors.text_secondary
                })
                .flex_grow(1.0)
                .into(),
            ],
        }
        .into();
        let content: Widget = if let Some(error) = &this.error_text {
            crate::stack::VStack {
                spacing: Some(tokens.spacing.xs),
                children: vec![
                    selection_row,
                    Text::new(error.clone())
                        .color(tokens.colors.error)
                        .size(tokens.typography.body_medium_size)
                        .into(),
                ],
            }
            .into()
        } else {
            selection_row
        };

        let value = this
            .error_text
            .as_ref()
            .map(|error| format!("Error: {error}"))
            .or_else(|| this.selected_file.clone())
            .unwrap_or_else(|| "No file selected".to_string());
        let mut region = SemanticsRegion::new(content)
            // The button and the filename beside it are one control; grouping them
            // means the selection is announced with the control rather than as
            // stray text somewhere after it.
            .role(Role::Group)
            .label("File upload")
            .value(value);
        if let Some(error) = &this.error_text {
            region = region.invalid(error.clone());
        }
        region.into()
    }
}
