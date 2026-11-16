use crate::state::TodoState;
use fission::op::AlignItems;
use fission::prelude::*;

fission_component! {
/// Shown in place of the list once every task has been cleared.
    pub(super) EmptyState<TodoState> |_empty, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let typography = &tokens.typography;
    } =>
        Container(
            Column(
            gap: tokens.spacing.xs,
            align_items: AlignItems::Center,
            children: [
                Text(cx.view.tr("todo.empty.title"))
                    .size(typography.body_large_size)
                    .weight(typography.font_weight_semibold)
                    .color(tokens.colors.text_primary),
                Text(cx.view.tr("todo.empty.body"))
                    .size(typography.font_size_sm)
                    .color(tokens.colors.text_muted),
            ],
        ))
        .padding_all(tokens.spacing.xl)
        .border(tokens.colors.border, 1.0)
        .border_dash(vec![tokens.spacing.xs, tokens.spacing.xs])
        .border_radius(tokens.radii.large)
}
