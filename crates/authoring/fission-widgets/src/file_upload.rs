use crate::stack::HStack;
use crate::Icon;
use fission_core::ui::{Button, ButtonVariant, Text, Widget};
use fission_core::ActionEnvelope;
use fission_icons::material;
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
    /// Action dispatched when the browse button is pressed.
    pub on_browse: Option<ActionEnvelope>,
}

impl From<FileUpload> for Widget {
    fn from(component: FileUpload) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let this = &component;

        let tokens = &view.env().theme.tokens;

        HStack {
            spacing: Some(8.0),
            children: vec![
                Button {
                    variant: ButtonVariant::Outline,
                    child: Some(
                        HStack {
                            spacing: Some(4.0),
                            children: vec![
                                Icon::svg(material::file::folder_open::regular())
                                    .size(16.0)
                                    .into(),
                                Text::new(this.label.clone()).flex_shrink(0.0).into(),
                            ],
                        }
                        .into(),
                    ),
                    on_press: this.on_browse.clone(),
                    ..Default::default()
                }
                .into(),
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
        .into()
    }
}
