use crate::state::{toggle_todo, TodoItem, TodoState, ToggleTodo};
use fission::prelude::*;

fission_component! {
/// One task. The whole label toggles it, and finished tasks sink into the card.
    pub(super) TodoRow<TodoState> { item: TodoItem } |row, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let background = if row.item.done {
            tokens.colors.surface_sunken
        } else {
            tokens.colors.surface
        };
    } =>
        Container(
            Checkbox(
                semantics_identifier: format!("todo.item.{}", row.item.id),
                checked: row.item.done,
                on_toggle: cx.bind(
                    ToggleTodo(row.item.id),
                    reduce_with!(toggle_todo),
                ),
                label: row.item.title.clone(),
            ),
        )
        .padding_all(tokens.spacing.m)
        .border(tokens.colors.border, 1.0)
        .border_radius(tokens.radii.medium)
        .bg(background)
}
