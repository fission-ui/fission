use super::layout_id;
use crate::GalleryState;
use fission::prelude::*;

fission_component! {
    pub(super) DisplayNameField<GalleryState> {
        compact: bool,
        value: String,
        on_input: ActionEnvelope,
    }
    |field, _state, _cx| {} =>
        Container(FormControl(
            id: layout_id("quality-gallery.display-name", field.compact),
            label: "Display name".to_string(),
            child: TextInput(
                id: layout_id(
                    "quality-gallery.display-name.input",
                    field.compact,
                ),
                semantics_identifier: "quality-gallery.input.display-name".to_string(),
                value: field.value.clone(),
                on_input: field.on_input.clone(),
                size: ComponentSize::Md,
            ),
            error: None,
            helper: (!field.compact).then(|| "Visible to your team.".into()),
            required: false,
        ))
        .width_length(Length::percent(50.0))
        .flex_grow(1.0)
}
