use crate::state::{add_todo, update_draft, AddTodo, TodoState, UpdateDraft};
use fission::op::AlignItems;
use fission::prelude::*;

/// Below this width the add button moves under the input and spans the card.
const STACKED_MAX_WIDTH: f32 = 480.0;

/// The input and button that add a task. Pressing Enter adds it too.
pub(super) struct TodoComposer;

impl From<TodoComposer> for Widget {
    fn from(_composer: TodoComposer) -> Self {
        Responsive::new(ComposerLayout { stacked: false })
            .case(ResponsiveCase::max_width(
                STACKED_MAX_WIDTH,
                ComposerLayout { stacked: true },
            ))
            .into()
    }
}

struct ComposerLayout {
    stacked: bool,
}

impl From<ComposerLayout> for Widget {
    fn from(layout: ComposerLayout) -> Self {
        let (ctx, view) = fission::build::current::<TodoState>();
        let tokens = &view.env().theme.tokens;
        let draft = &view.state().draft;
        let add = ctx.bind(AddTodo, reduce_with!(add_todo));

        let input = TextInput {
            semantics_identifier: Some("todo.composer.input".into()),
            value: draft.clone(),
            label: Some(view.tr("todo.composer.label").into()),
            placeholder: Some(view.tr("todo.composer.placeholder").into()),
            on_input: Some(ctx.bind(UpdateDraft, reduce_with!(update_draft))),
            on_submit: Some(add.clone()),
            ..Default::default()
        };
        let button = Button {
            variant: ButtonVariant::Primary,
            child: Some(Text::new(view.tr("todo.composer.add")).into()),
            on_press: Some(add),
            disabled: draft.trim().is_empty(),
            ..Default::default()
        }
        .semantics_identifier("todo.composer.add");

        if layout.stacked {
            Column {
                gap: Some(tokens.spacing.s),
                children: widgets![input, button],
                ..Default::default()
            }
            .into()
        } else {
            Row {
                gap: Some(tokens.spacing.s),
                align_items: AlignItems::End,
                children: widgets![Container::new(input).flex_grow(1.0), button],
                ..Default::default()
            }
            .into()
        }
    }
}
