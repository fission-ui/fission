use super::layout_id;
use crate::GalleryState;
use fission::prelude::*;

/// Workspace roles in menu order, with the separator placed before `Owner`.
const ROLES: [(&str, &str); 3] = [
    ("Viewer", "quality-gallery.role.viewer"),
    ("Editor", "quality-gallery.role.editor"),
    ("Owner", "quality-gallery.role.owner"),
];

#[fission_reducer(ToggleQualitySelect)]
fn toggle_quality_select(state: &mut GalleryState) {
    state.select_open = !state.select_open;
    state.menu_open = false;
    state.modal_open = false;
}

#[fission_reducer(SelectQualityRole)]
fn select_quality_role(state: &mut GalleryState, role: String) {
    state.select_value = Some(role);
    state.select_open = false;
}

pub(super) struct RoleField {
    pub compact: bool,
}

impl From<RoleField> for Widget {
    fn from(field: RoleField) -> Self {
        let (ctx, view) = fission::build::current::<GalleryState>();
        let state = view.state();
        let selected = state.select_value.as_deref().unwrap_or("Editor");

        let mut options = Vec::new();
        for (role, identifier) in ROLES {
            if role == "Owner" {
                options.push(SelectSeparator::new().into());
            }
            options.push(
                SelectOption::option(role, selected == role)
                    .on_select(with_reducer!(
                        ctx,
                        SelectQualityRole(role.into()),
                        select_quality_role
                    ))
                    .semantics_identifier(identifier)
                    .into(),
            );
        }

        Container::new(FormControl {
            id: Some(layout_id("quality-gallery.role", field.compact)),
            label: Some("Workspace role".into()),
            child: SelectLayout::new(
                layout_id("quality-gallery.role.select", field.compact),
                SelectTrigger::new(selected.to_string())
                    .size(ComponentSize::Md)
                    .semantics_identifier("quality-gallery.select.trigger"),
                SelectContent::new(vec![SelectGroup::new(options).label("Access level").into()]),
            )
            .open(state.select_open)
            .on_toggle(with_reducer!(
                ctx,
                ToggleQualitySelect,
                toggle_quality_select
            ))
            .into(),
            error: None,
            helper: None,
            required: false,
        })
        .width_length(Length::percent(50.0))
        .flex_grow(1.0)
        .into()
    }
}
