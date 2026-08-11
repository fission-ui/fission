//! Constraint-based layout engine for the Fission UI framework.
//!
//! This crate takes a flat list of [LayoutInputNode]s (produced from the
//! [fission-ir](fission_ir) intermediate representation) and computes the
//! absolute position and size of every node on screen. It implements:
//!
//! * **Box layout** -- constrained containers with padding, min/max, and aspect ratio.
//! * **Flexbox** -- single-axis distribution with grow, shrink, wrap, alignment, and justification.
//! * **CSS Grid** -- two-dimensional track-based layout with fr, %, and fixed sizing.
//! * **Scroll containers** -- clipped viewports with infinite content axes.
//! * **Absolute positioning** -- top/left/right/bottom offsets.
//! * **ZStack** -- overlapping children.
//! * **Flyout anchoring** -- popups positioned relative to an anchor node.
//!
//! The engine is pure computation with no platform dependencies. Give it nodes and
//! a viewport size, and it returns a [LayoutSnapshot] mapping every
//! [WidgetId](fission_ir::WidgetId) to a [LayoutRect].
//!
//! # Example
//!
//! ~~~rust,no_run
//! use fission_layout::*;
//! use fission_ir::{WidgetId, LayoutOp};
//!
//! let mut engine = LayoutEngine::new();
//! let root_id = WidgetId::explicit("root");
//! // ... build LayoutInputNode list ...
//! // let snapshot = engine.compute_layout(&nodes, root_id, viewport, &|_| 0.0).unwrap();
//! ~~~

mod engine;
mod geometry;
mod grid_tracks;
mod input;
mod paragraph;
mod snapshot;
mod style;
mod text;

pub use engine::LayoutEngine;
pub use fission_ir::{FlexDirection, GridPlacement, GridTrack, LayoutOp};
pub use geometry::{BoxConstraints, LayoutPoint, LayoutRect, LayoutSize, LayoutUnit};
pub use input::LayoutInputNode;
#[doc(hidden)]
pub use paragraph::{
    ParagraphAffinity, ParagraphCacheKey, ParagraphCapabilities, ParagraphCapability,
    ParagraphCapabilityError, ParagraphCaret, ParagraphCluster, ParagraphDescription,
    ParagraphDirection, ParagraphDrawDataId, ParagraphEngine, ParagraphError, ParagraphFontFeature,
    ParagraphFontVariation, ParagraphGeometry, ParagraphHitRegion, ParagraphHitTest,
    ParagraphInlineBox, ParagraphInlineObject, ParagraphLine, ParagraphPreedit, ParagraphResult,
    ParagraphResultStore, ParagraphSelectionBox, ParagraphStyleRun, ParagraphUnresolvedGlyph,
    Utf8Index, Utf8Range,
};
pub use snapshot::{LayoutInspection, LayoutNodeGeometry, LayoutSnapshot};
pub use text::{LineMetric, RichTextInlineBox, RichTextLayoutInfo, TextMeasurer};

use fission_ir::WidgetId;

/// A source of scroll offsets for scroll containers.
///
/// The layout engine calls [get_offset](ScrollDataSource::get_offset) for each
/// [LayoutOp::Scroll] node to learn how far the user has scrolled. Platform
/// backends implement this trait (or pass a closure, which also implements it).
///
/// # Example
///
/// ~~~rust
/// use fission_layout::ScrollDataSource;
/// use fission_ir::WidgetId;
///
/// let source = |_node: WidgetId| -> f32 { 0.0 };
/// assert_eq!(source.get_offset(WidgetId::explicit("scroll")), 0.0);
/// ~~~
pub trait ScrollDataSource {
    /// Returns the current scroll offset for the given scroll container node.
    fn get_offset(&self, node_id: WidgetId) -> f32;
}

impl<F> ScrollDataSource for F
where
    F: Fn(WidgetId) -> f32,
{
    fn get_offset(&self, node_id: WidgetId) -> f32 {
        self(node_id)
    }
}
