use crate::state::TodoState;
use fission::op::AlignItems;
use fission::prelude::*;

/// Shown in place of the list once every task has been cleared.
pub(super) struct EmptyState;

impl From<EmptyState> for Widget {
    fn from(_empty: EmptyState) -> Self {
        let (_, view) = fission::build::current::<TodoState>();
        let tokens = &view.env().theme.tokens;
        let typography = &tokens.typography;

        Container::new(Column {
            gap: Some(tokens.spacing.xs),
            align_items: AlignItems::Center,
            children: widgets![
                Text::new(view.tr("todo.empty.title"))
                    .size(typography.body_large_size)
                    .weight(typography.font_weight_semibold)
                    .color(tokens.colors.text_primary),
                Text::new(view.tr("todo.empty.body"))
                    .size(typography.font_size_sm)
                    .color(tokens.colors.text_muted),
            ],
            ..Default::default()
        })
        .padding_all(tokens.spacing.xl)
        .border(tokens.colors.border, 1.0)
        .border_dash(vec![tokens.spacing.xs, tokens.spacing.xs])
        .border_radius(tokens.radii.large)
        .into()
    }
}
