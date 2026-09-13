use super::layout_id;
use fission::prelude::*;

pub(super) struct DisplayNameField {
    pub compact: bool,
    pub value: String,
    pub on_input: ActionEnvelope,
}

impl From<DisplayNameField> for Widget {
    fn from(field: DisplayNameField) -> Self {
        Container::new(FormControl {
            id: Some(layout_id("quality-gallery.display-name", field.compact)),
            label: Some("Display name".into()),
            child: TextInput {
                id: Some(layout_id(
                    "quality-gallery.display-name.input",
                    field.compact,
                )),
                semantics_identifier: Some("quality-gallery.input.display-name".into()),
                value: field.value,
                on_input: Some(field.on_input),
                size: ComponentSize::Md,
                ..Default::default()
            }
            .into(),
            error: None,
            helper: (!field.compact).then(|| "Visible to your team.".into()),
            required: false,
        })
        .width_length(Length::percent(50.0))
        .flex_grow(1.0)
        .into()
    }
}
