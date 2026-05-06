use crate::stack::HStack;
use crate::Icon;
use fission_core::ui::{Button, ButtonVariant, Node, Text};
use fission_core::{ActionEnvelope, BuildCtx, View, Widget};
use fission_icons::material;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileUpload {
    pub label: String,
    pub selected_file: Option<String>,
    pub on_browse: Option<ActionEnvelope>,
}

impl<S: fission_core::AppState> Widget<S> for FileUpload {
    fn build(&self, _ctx: &mut BuildCtx<S>, view: &View<S>) -> Node {
        let tokens = &view.env.theme.tokens;

        HStack {
            spacing: Some(8.0),
            children: vec![
                Button {
                    variant: ButtonVariant::Outline,
                    child: Some(Box::new(
                        HStack {
                            spacing: Some(4.0),
                            children: vec![
                                Icon::svg(material::file::folder_open::regular())
                                    .size(16.0)
                                    .into_node(),
                                Text::new(self.label.clone()).flex_shrink(0.0).into_node(),
                            ],
                        }
                        .into_node(),
                    )),
                    on_press: self.on_browse.clone(),
                    ..Default::default()
                }
                .into_node(),
                Text::new(
                    self.selected_file
                        .clone()
                        .unwrap_or("No file selected".into()),
                )
                .color(if self.selected_file.is_some() {
                    tokens.colors.text_primary
                } else {
                    tokens.colors.text_secondary
                })
                .flex_grow(1.0)
                .into_node(),
            ],
        }
        .into_node()
    }
}
