use crate::catalog::Target;
use crate::state::ShowcaseState;
use fission::op::Fill;
use fission::prelude::*;

#[derive(Clone, Copy, Debug)]
pub(crate) struct TargetChip {
    pub(crate) target: Target,
}

impl From<TargetChip> for Widget {
    fn from(component: TargetChip) -> Self {
        let (_ctx, view) = fission::build::current::<ShowcaseState>();
        let tokens = &view.env().theme.tokens;
        Container::new(
            Text::new(component.target.label())
                .size(tokens.typography.font_size_xs)
                .weight(tokens.typography.font_weight_medium)
                .color(tokens.colors.text_secondary),
        )
        .padding([
            tokens.spacing.s,
            tokens.spacing.s,
            tokens.spacing.xs,
            tokens.spacing.xs,
        ])
        .bg_fill(Fill::Solid(tokens.colors.surface_sunken))
        .border_radius(tokens.radii.full)
        .into()
    }
}
