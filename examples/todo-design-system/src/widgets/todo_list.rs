use super::{EmptyState, TodoRow};
use crate::state::TodoState;
use fission::prelude::*;

pub(super) struct TodoList;

impl From<TodoList> for Widget {
    fn from(_list: TodoList) -> Self {
        let (_, view) = fission::build::current::<TodoState>();
        let tokens = &view.env().theme.tokens;
        let items = &view.state().items;
        if items.is_empty() {
            return EmptyState.into();
        }

        SemanticsRegion::new(Column {
            gap: Some(tokens.spacing.s),
            children: items
                .iter()
                .map(|item| TodoRow { item: item.clone() }.into())
                .collect(),
            ..Default::default()
        })
        .role(Role::Group)
        .label(view.tr("todo.list.label"))
        .identifier("todo.list")
        .into()
    }
}
