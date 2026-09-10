use fission_core::ui::{CardPattern, Column, ComponentSize, Widget};
use serde::{Deserialize, Serialize};

use super::separator::CardSeparator;
use super::{CardContent, CardFooter, CardHeader, CardSurface, CardSurfacePadding};

/// A complete card surface with retained header, content, and footer regions.
///
/// With separators enabled, each region owns density-derived padding so every
/// boundary reaches the surface edge. Without separators, the same density is
/// expressed as vertical surface padding, horizontal region padding, and one
/// gap between adjacent regions. Use this directly instead of nesting it inside
/// [`Card`](super::Card).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CardLayout {
    /// Optional heading and trailing-action region.
    pub header: Option<CardHeader>,
    /// Optional primary content region.
    pub content: Option<CardContent>,
    /// Optional end-aligned footer region.
    pub footer: Option<CardFooter>,
    /// Whether adjacent present regions use the shared separator recipe.
    ///
    /// A footer-specific recipe boundary remains part of the footer treatment;
    /// otherwise the footer also uses this shared fallback. Disabled layouts
    /// use the density recipe's section gap between unbounded regions.
    pub separated: bool,
    /// Visual density shared by all standard card regions.
    pub size: ComponentSize,
    /// Visual treatment shared with [`Card`](super::Card).
    pub pattern: CardPattern,
    /// Whether the active card hover treatment may be shown.
    pub interactive: bool,
    /// Whether the surface uses the active design system's selected treatment.
    #[serde(default)]
    pub selected: bool,
    /// Optional symmetric horizontal inset for region separators.
    ///
    /// When omitted, the separator recipe's margin is used. `Some(0.0)`
    /// explicitly requests an edge-to-edge boundary.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub separator_inset: Option<f32>,
}

impl Default for CardLayout {
    fn default() -> Self {
        Self {
            header: None,
            content: None,
            footer: None,
            separated: true,
            size: ComponentSize::Md,
            pattern: CardPattern::Raised,
            interactive: false,
            selected: false,
            separator_inset: None,
        }
    }
}

impl CardLayout {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn header(mut self, header: CardHeader) -> Self {
        self.header = Some(header);
        self
    }

    pub fn content(mut self, content: CardContent) -> Self {
        self.content = Some(content);
        self
    }

    pub fn footer(mut self, footer: CardFooter) -> Self {
        self.footer = Some(footer);
        self
    }

    pub fn separated(mut self, separated: bool) -> Self {
        self.separated = separated;
        self
    }

    /// Applies one card density to region padding, spacing, and typography.
    pub fn size(mut self, size: ComponentSize) -> Self {
        self.size = size;
        self
    }

    pub fn pattern(mut self, pattern: CardPattern) -> Self {
        self.pattern = pattern;
        self
    }

    pub fn interactive(mut self, interactive: bool) -> Self {
        self.interactive = interactive;
        self
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn separator_inset(mut self, inset: f32) -> Self {
        self.separator_inset = Some(inset.max(0.0));
        self
    }
}

impl From<CardLayout> for Widget {
    fn from(component: CardLayout) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let theme = &view.env().theme.components.card;
        let density = theme.resolve_size(component.size);
        let padding = density.padding_box(theme.padding, theme.padding);
        let has_footer = component.footer.is_some();
        let mut regions = Vec::new();

        if let Some(header) = component.header {
            regions.push(
                header
                    .with_region_context(component.size, component.separated)
                    .into(),
            );
        }
        if let Some(content) = component.content {
            if component.separated && !regions.is_empty() {
                regions.push(CardSeparator::section(component.separator_inset).into());
            }
            regions.push(
                content
                    .with_region_context(component.size, component.separated)
                    .into(),
            );
        }
        if let Some(footer) = component.footer {
            if !regions.is_empty() {
                if theme.footer_style.border.is_some() {
                    regions.push(CardSeparator::footer(component.separator_inset).into());
                } else if component.separated {
                    regions.push(CardSeparator::section(component.separator_inset).into());
                }
            }
            regions.push(footer.with_region_context(component.size, true).into());
        }

        CardSurface {
            child: Column {
                children: regions,
                gap: (!component.separated).then_some(density.gap.unwrap_or(theme.padding)),
                ..Default::default()
            }
            .into(),
            pattern: component.pattern,
            interactive: component.interactive,
            selected: component.selected,
            content_padding: if component.separated {
                CardSurfacePadding::None
            } else {
                CardSurfacePadding::Explicit([
                    0.0,
                    0.0,
                    padding[2],
                    if has_footer { 0.0 } else { padding[3] },
                ])
            },
        }
        .into()
    }
}
