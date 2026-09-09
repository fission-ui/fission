use fission_core::op::AlignItems;
use fission_core::ui::{Column, ComponentSize, Row, Widget};
use serde::{Deserialize, Serialize};

use super::region::{CardRegion, CardRegionKind};
use super::{CardDescription, CardTitle};

#[derive(Clone, Debug, Serialize, Deserialize)]
enum CardHeaderTitle {
    Standard(CardTitle),
    Custom(Widget),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum CardHeaderDescription {
    Standard(CardDescription),
    Custom(Widget),
}

/// The heading region of a sectioned card.
///
/// [`CardTitle`] and [`CardDescription`] receive the enclosing card's density
/// automatically. Richer retained content remains available through
/// [`CardHeader::custom`] and [`CardHeader::custom_description`]. The trailing
/// action is top-aligned and may contain any control or action group.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CardHeader {
    title: CardHeaderTitle,
    description: Option<CardHeaderDescription>,
    action: Option<Widget>,
    #[serde(default)]
    size: ComponentSize,
    #[serde(default = "default_true")]
    include_vertical_padding: bool,
}

const fn default_true() -> bool {
    true
}

impl CardHeader {
    /// Creates a standard heading whose typography follows the card density.
    pub fn new(title: CardTitle) -> Self {
        Self {
            title: CardHeaderTitle::Standard(title),
            description: None,
            action: None,
            size: ComponentSize::Md,
            include_vertical_padding: true,
        }
    }

    /// Creates a heading with fully custom retained title content.
    pub fn custom(title: impl Into<Widget>) -> Self {
        Self {
            title: CardHeaderTitle::Custom(title.into()),
            description: None,
            action: None,
            size: ComponentSize::Md,
            include_vertical_padding: true,
        }
    }

    /// Adds the standard supporting description treatment.
    pub fn description(mut self, description: CardDescription) -> Self {
        self.description = Some(CardHeaderDescription::Standard(description));
        self
    }

    /// Adds fully custom retained supporting content.
    pub fn custom_description(mut self, description: impl Into<Widget>) -> Self {
        self.description = Some(CardHeaderDescription::Custom(description.into()));
        self
    }

    /// Adds a trailing control or action group.
    pub fn action(mut self, action: impl Into<Widget>) -> Self {
        self.action = Some(action.into());
        self
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

impl From<CardHeader> for Widget {
    fn from(component: CardHeader) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let theme = &view.env().theme.components.card;
        let density = theme.resolve_size(component.size);

        let title = match component.title {
            CardHeaderTitle::Standard(title) => title.size(component.size).into(),
            CardHeaderTitle::Custom(title) => title,
        };
        let mut heading = vec![title];
        if let Some(description) = component.description {
            heading.push(match description {
                CardHeaderDescription::Standard(description) => {
                    description.size(component.size).into()
                }
                CardHeaderDescription::Custom(description) => description,
            });
        }

        let mut row_children = vec![Column {
            children: heading,
            gap: theme.header_style.gap,
            flex_grow: 1.0,
            flex_shrink: 1.0,
            ..Default::default()
        }
        .into()];
        if let Some(action) = component.action {
            row_children.push(action);
        }

        CardRegion {
            child: Row {
                children: row_children,
                gap: density.gap,
                align_items: AlignItems::Start,
                ..Default::default()
            }
            .into(),
            kind: CardRegionKind::Header,
            size: component.size,
            include_vertical_padding: component.include_vertical_padding,
        }
        .into()
    }
}
