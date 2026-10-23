use crate::state::TodoState;
use fission::prelude::*;

fission_component! {
/// One task. The whole label toggles it, and finished tasks sink into the card.
    pub(super) TodoRow<TodoState> {
        id: usize,
        title: String,
        done: bool,
    } |row, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let background = if row.done {
            tokens.colors.surface_sunken
        } else {
            tokens.colors.surface
        };
    } =>
        Container(
            Checkbox(
                key: row.id,
                handler: "toggle",
                semantics_identifier: format!("todo.item.{}", row.id),
                checked: row.done,
                on_toggle: |row, state, _cx| state.toggle(row.id),
                label: row.title.clone(),
            ),
        )
        .padding_all(tokens.spacing.m)
        .border(tokens.colors.border, 1.0)
        .border_radius(tokens.radii.medium)
        .bg(background)
}
