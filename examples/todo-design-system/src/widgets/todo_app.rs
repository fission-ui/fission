use super::{CompletedFooter, TodoComposer, TodoHeader, TodoList};
use crate::state::TodoState;
use fission::op::AlignItems;
use fission::prelude::*;

/// The widest the todo card grows; wider windows centre it.
const CONTENT_MAX_WIDTH: f32 = 640.0;

#[derive(Clone)]
pub struct TodoApp;

impl From<TodoApp> for Widget {
    fn from(_app: TodoApp) -> Self {
        let (_, view) = fission::build::current::<TodoState>();
        let tokens = &view.env().theme.tokens;
        let card = Card {
            pattern: CardPattern::Raised,
            child: Column {
                gap: Some(tokens.spacing.l),
                children: widgets![TodoHeader, TodoComposer, TodoList, CompletedFooter],
                ..Default::default()
            }
            .into(),
            ..Default::default()
        };

        Container::new(Column {
            align_items: AlignItems::Center,
            children: widgets![Container::new(card)
                .width_length(Length::percent(100.0))
                .max_width(CONTENT_MAX_WIDTH)],
            ..Default::default()
        })
        .bg(tokens.colors.background)
        .padding_all(tokens.spacing.xl)
        .into()
    }
}
