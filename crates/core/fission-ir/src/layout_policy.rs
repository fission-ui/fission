use crate::op::{JustifyContent, LayoutUnit};
use crate::WidgetId;
use serde::{Deserialize, Serialize};

/// Logical direction used to resolve horizontal layout and interaction.
///
/// Widget trees retain their logical source order. Layout and keyboard
/// navigation resolve that order against this direction at their respective
/// boundaries, so changing direction does not require applications to rebuild
/// collections in reverse.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum LayoutDirection {
    /// Logical start is the physical left edge.
    #[default]
    LeftToRight,
    /// Logical start is the physical right edge.
    RightToLeft,
}

impl LayoutDirection {
    /// Returns whether logical start is the physical left edge.
    pub const fn is_left_to_right(&self) -> bool {
        matches!(self, Self::LeftToRight)
    }

    /// Resolves logical main-axis justification to physical flex placement.
    pub const fn resolve_horizontal_justification(
        self,
        justification: JustifyContent,
    ) -> JustifyContent {
        match (self, justification) {
            (Self::RightToLeft, JustifyContent::Start) => JustifyContent::End,
            (Self::RightToLeft, JustifyContent::End) => JustifyContent::Start,
            (_, justification) => justification,
        }
    }

    /// Resolves a logical flyout edge to the physical anchor edge.
    pub const fn resolve_flyout_alignment(self, alignment: FlyoutAlignment) -> FlyoutAlignment {
        match (self, alignment) {
            (Self::RightToLeft, FlyoutAlignment::Start) => FlyoutAlignment::End,
            (Self::RightToLeft, FlyoutAlignment::End) => FlyoutAlignment::Start,
            (_, alignment) => alignment,
        }
    }
}

/// Logical horizontal alignment of a flyout surface to its anchor.
///
/// `Start` and `End` express caller intent rather than physical left/right
/// placement. The layout engine resolves the edge against the application's
/// [`LayoutDirection`] without changing widget contracts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum FlyoutAlignment {
    #[default]
    Start,
    End,
}

/// Preferred vertical side for a flyout surface.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum FlyoutPlacement {
    /// Prefer below the anchor, fall back above it, then clamp to the viewport.
    #[default]
    Auto,
    /// Prefer the space below the anchor and clamp if it does not fit.
    Below,
    /// Prefer the space above the anchor and clamp if it does not fit.
    Above,
}

/// How a flyout surface derives its width from its anchor.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum FlyoutWidth {
    /// Preserve the surface's intrinsic content width.
    #[default]
    Content,
    /// Constrain the surface to exactly the anchor width.
    MatchAnchor,
    /// Preserve intrinsic width while enforcing the anchor width as a minimum.
    AtLeastAnchor,
}

/// Retained positioning and sizing policy for a flyout surface.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FlyoutOptions {
    /// Logical horizontal edge shared by the anchor and surface.
    pub alignment: FlyoutAlignment,
    /// Preferred vertical side and collision fallback behavior.
    pub placement: FlyoutPlacement,
    /// Intrinsic or anchor-derived surface sizing.
    pub width: FlyoutWidth,
    /// Logical distance between the anchor edge and the visible surface.
    pub gap: LayoutUnit,
    /// Optional descendant whose vertical center should align with the
    /// anchor's center. Selection popups use this to keep the selected option
    /// visually connected to the closed control without encoding item heights
    /// in the positioning system.
    pub alignment_target: Option<WidgetId>,
}

impl FlyoutOptions {
    pub const fn with_alignment(mut self, alignment: FlyoutAlignment) -> Self {
        self.alignment = alignment;
        self
    }

    pub const fn with_placement(mut self, placement: FlyoutPlacement) -> Self {
        self.placement = placement;
        self
    }

    pub const fn with_width(mut self, width: FlyoutWidth) -> Self {
        self.width = width;
        self
    }

    pub const fn with_gap(mut self, gap: LayoutUnit) -> Self {
        self.gap = gap;
        self
    }

    pub const fn with_alignment_target(mut self, target: WidgetId) -> Self {
        self.alignment_target = Some(target);
        self
    }
}

impl Default for FlyoutOptions {
    fn default() -> Self {
        Self {
            alignment: FlyoutAlignment::Start,
            placement: FlyoutPlacement::Auto,
            width: FlyoutWidth::Content,
            gap: 0.0,
            alignment_target: None,
        }
    }
}
