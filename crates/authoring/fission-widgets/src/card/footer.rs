use fission_core::op::{AlignItems, JustifyContent};
use fission_core::ui::{ComponentSize, Row, Widget};
use serde::{Deserialize, Serialize};

use super::region::{CardRegion, CardRegionKind};

/// The action region at the bottom of a sectioned card.
///
/// Children are arranged as a compact, end-aligned action row on the active
/// design system's subtle footer surface. The footer recipe's border becomes a
/// full-width top boundary rather than a four-sided inset stroke. A child may
/// itself be a retained layout when the footer needs custom left/right content.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CardFooter {
    /// Controls or custom retained content shown in the footer.
    pub children: Vec<Widget>,
    #[serde(default)]
    size: ComponentSize,
    #[serde(default = "default_true")]
    include_vertical_padding: bool,
}

impl Default for CardFooter {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

const fn default_true() -> bool {
    true
}

impl CardFooter {
    pub fn new(children: Vec<Widget>) -> Self {
        Self {
            children,
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

impl From<CardFooter> for Widget {
    fn from(component: CardFooter) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let theme = &view.env().theme.components.card;

        CardRegion {
            child: Row {
                children: component.children,
                gap: theme.footer_style.gap,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::End,
                ..Default::default()
            }
            .into(),
            kind: CardRegionKind::Footer,
            size: component.size,
            include_vertical_padding: component.include_vertical_padding,
        }
        .into()
    }
}
