use crate::catalog::Target;
use fission::prelude::*;
use fission::widgets::Badge;

/// A platform the example runs on, as a quiet neutral badge.
#[derive(Clone, Copy, Debug)]
pub(crate) struct TargetChip {
    pub(crate) target: Target,
}

impl From<TargetChip> for Widget {
    fn from(component: TargetChip) -> Self {
        Badge {
            text: component.target.label().into(),
            tone: BadgeTone::Gray,
            size: ComponentSize::Sm,
            ..Default::default()
        }
        .into()
    }
}
