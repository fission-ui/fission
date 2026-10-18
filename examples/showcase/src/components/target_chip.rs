use crate::catalog::Target;
use crate::state::ShowcaseState;
use fission::prelude::*;
use fission::widgets::Badge;

fission_component! {
/// A platform the example runs on, as a quiet neutral badge.
    #[derive(Clone, Copy, Debug)] pub(crate) TargetChip<ShowcaseState> { target: Target }
    |component, _state, _cx| {} =>
        Badge(
            text: component.target.label().to_string(),
            tone: BadgeTone::Gray,
            size: ComponentSize::Sm,
        )
}
