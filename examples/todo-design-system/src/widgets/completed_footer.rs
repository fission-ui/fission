use crate::state::{clear_completed, ClearCompleted, TodoState};
use fission::op::AlignItems;
use fission::prelude::*;

/// Progress so far and the action that clears finished tasks.
pub(super) struct CompletedFooter;

impl From<CompletedFooter> for Widget {
    fn from(_footer: CompletedFooter) -> Self {
        let (ctx, view) = fission::build::current::<TodoState>();
        let tokens = &view.env().theme.tokens;
        let state = view.state();
        let done = state.completed_count();

        Row {
            gap: Some(tokens.spacing.m),
            align_items: AlignItems::Center,
            children: widgets![
                Badge {
                    text: format!("{done}/{} {}", state.items.len(), view.tr("todo.progress")),
                    tone: BadgeTone::Success,
                    ..Default::default()
                },
                Spacer {
                    flex_grow: 1.0,
                    ..Default::default()
                },
                Button {
                    variant: ButtonVariant::TertiaryGray,
                    child: Some(Text::new(view.tr("todo.clear_completed")).into()),
                    on_press: Some(ctx.bind(ClearCompleted, reduce_with!(clear_completed))),
                    disabled: done == 0,
                    ..Default::default()
                }
                .semantics_identifier("todo.clear_completed"),
            ],
            ..Default::default()
        }
        .into()
    }
}
