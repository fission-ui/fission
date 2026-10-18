use crate::state::TodoState;
use fission::op::AlignItems;
use fission::prelude::*;

/// Below this width the add button moves under the input and spans the card.
const STACKED_MAX_WIDTH: f32 = 480.0;

fission_component! {
/// The input and button that add a task. Pressing Enter adds it too.
    pub(super) TodoComposer<TodoState> |_composer, _state, _cx| {} =>
        Responsive::new(ComposerLayout::new(false)).case(ResponsiveCase::max_width(
            STACKED_MAX_WIDTH,
            ComposerLayout::new(true),
        ))
}

fission_component! {
    ComposerLayout<TodoState> { stacked: bool } |layout, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        if layout.stacked {
            Column(
                gap: tokens.spacing.s,
                children: [ComposerInput(), ComposerAddButton()],
            )
        } else {
            Row(
                gap: tokens.spacing.s,
                align_items: AlignItems::End,
                children: [
                    Container(ComposerInput()).flex_grow(1.0),
                    ComposerAddButton(),
                ],
            )
        }
}

fission_component! {
    ComposerInput<TodoState> |_input, state, cx| {} =>
        TextInput(
            key: "draft",
            semantics_identifier: "todo.composer.input".to_string(),
            label: Some(cx.view.tr("todo.composer.label").into()),
            placeholder: Some(cx.view.tr("todo.composer.placeholder").into()),
            bind: state.draft,
            on_submit: |_input, state, _cx| state.add_draft(),
        )
}

fission_component! {
    ComposerAddButton<TodoState> |_button, state, cx| {} =>
        Button(
            key: "add",
            variant: ButtonVariant::Primary,
            child: Text(cx.view.tr("todo.composer.add")),
            on_press: |_button, state, _cx| state.add_draft(),
            disabled: state.draft.trim().is_empty(),
        ).semantics_identifier("todo.composer.add")
}
