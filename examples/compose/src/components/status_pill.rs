use fission::prelude::*;

/// Deliberately ordinary Rust widget used to prove that Compose does not need
/// a registration, derive, adapter, or knowledge of third-party widget types.
#[derive(Default)]
pub struct StatusPill {
    pub text: String,
    pub active: bool,
}

impl From<StatusPill> for Widget {
    fn from(value: StatusPill) -> Self {
        let (_, view) = fission::build::current::<crate::state::AppState>();
        let tokens = &view.env().theme.tokens;
        Container::new(Text::new(value.text))
            .padding([
                tokens.spacing.xs,
                tokens.spacing.s,
                tokens.spacing.xs,
                tokens.spacing.s,
            ])
            .border_radius(tokens.radii.full)
            .bg(if value.active {
                tokens.colors.success
            } else {
                tokens.colors.surface_sunken
            })
            .into()
    }
}
