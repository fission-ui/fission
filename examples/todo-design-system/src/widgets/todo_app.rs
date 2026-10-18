use super::{CompletedFooter, TodoComposer, TodoHeader, TodoList};
use crate::state::TodoState;
use fission::op::AlignItems;
use fission::prelude::*;

/// The widest the todo card grows; wider windows centre it.
const CONTENT_MAX_WIDTH: f32 = 640.0;

fission_component! {
    pub TodoApp<TodoState>
    |_app, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
    Container(
        Column(
            align_items: AlignItems::Center,
            children: [
                Container(
                    Card(
                        pattern: CardPattern::Raised,
                        child: Column(
                            gap: tokens.spacing.l,
                            children: [TodoHeader, TodoComposer, TodoList, CompletedFooter],
                        ),
                    ),
                )
                .width_length(Length::percent(100.0))
                .max_width(CONTENT_MAX_WIDTH),
            ],
        ),
    )
    .bg(tokens.colors.background)
    .padding_all(tokens.spacing.xl)
}
