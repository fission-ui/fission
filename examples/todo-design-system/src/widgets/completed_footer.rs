use crate::state::TodoState;
use fission::op::AlignItems;
use fission::prelude::*;

fission_component! {
/// Progress so far and the action that clears finished tasks.
    pub(super) CompletedFooter<TodoState> |_footer, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let done = state.completed_count();
    } =>
        Row(
            gap: tokens.spacing.m,
            align_items: AlignItems::Center,
            children: [
                Badge(
                    text: format!("{done}/{} {}", state.items.len(), cx.view.tr("todo.progress")),
                    tone: BadgeTone::Success,
                ),
                Spacer(flex_grow: 1.0),
                Button(
                    key: "clear-completed",
                    variant: ButtonVariant::TertiaryGray,
                    child: Text(cx.view.tr("todo.clear_completed")),
                    on_press: |_footer, state, _cx| state.clear_completed(),
                    disabled: done == 0,
                )
                .semantics_identifier("todo.clear_completed"),
            ],
        )
}
