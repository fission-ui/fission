use super::{EmptyState, TodoRow};
use crate::state::TodoState;
use fission::prelude::*;

fission_component! {
    pub(super) TodoList<TodoState> |_list, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        if state.items.is_empty() {
            EmptyState()
        } else {
            SemanticsRegion(
                role: Role::Group,
                label: cx.view.tr("todo.list.label"),
                identifier: "todo.list".to_string(),
                child: Column(
                    gap: tokens.spacing.s,
                    children: [
                        for item in state.items.iter().cloned() {
                            TodoRow(item.clone(), key: item.id)
                        },
                    ],
                ),
            )
        }
}
