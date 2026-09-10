use fission_core::ui::{ComponentSize, Widget};
use serde::{Deserialize, Serialize};

use super::region::{CardRegion, CardRegionKind};

/// The main body region of a sectioned card.
///
/// The supplied retained widget receives the active card region padding. It is
/// otherwise left untouched so forms, lists, media, and other custom content
/// keep their own layout behavior.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CardContent {
    /// Main card content.
    pub child: Widget,
    #[serde(default)]
    size: ComponentSize,
    #[serde(default = "default_true")]
    include_vertical_padding: bool,
}

const fn default_true() -> bool {
    true
}

impl CardContent {
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
            size: ComponentSize::Md,
            include_vertical_padding: true,
        }
    }

    pub(super) fn with_region_context(
        mut self,
        size: ComponentSize,
        include_vertical_padding: bool,
    ) -> Self {
        self.size = size;
        self.include_vertical_padding = include_vertical_padding;
        self
    }
}

impl From<CardContent> for Widget {
    fn from(component: CardContent) -> Self {
        CardRegion {
            child: component.child,
            kind: CardRegionKind::Content,
            size: component.size,
            include_vertical_padding: component.include_vertical_padding,
        }
        .into()
    }
}
