use crate::state::{toggle_todo, TodoItem, TodoState, ToggleTodo};
use fission::prelude::*;

/// One task. The whole label toggles it, and finished tasks sink into the card.
pub(super) struct TodoRow {
    pub item: TodoItem,
}

impl From<TodoRow> for Widget {
    fn from(row: TodoRow) -> Self {
        let (ctx, view) = fission::build::current::<TodoState>();
        let tokens = &view.env().theme.tokens;
        let item = row.item;
        let background = if item.done {
            tokens.colors.surface_sunken
        } else {
            tokens.colors.surface
        };

        Container::new(Checkbox {
            semantics_identifier: Some(format!("todo.item.{}", item.id)),
            checked: item.done,
            on_toggle: Some(ctx.bind(ToggleTodo(item.id), reduce_with!(toggle_todo))),
            label: Some(item.title),
            ..Default::default()
        })
        .padding_all(tokens.spacing.m)
        .border(tokens.colors.border, 1.0)
        .border_radius(tokens.radii.medium)
        .bg(background)
        .into()
    }
}
