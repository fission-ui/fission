//! Fission-owned low-level interface to the pinned Skia build.
//!
//! The public safe handles enforce deterministic ownership and make native
//! thread-affine objects `!Send` and `!Sync`. [`ffi`] remains available for ABI
//! validation and platform integration; callers using it directly must obey
//! the ownership rules documented by `include/fission_skia.h`.

#![deny(unsafe_op_in_unsafe_fn)]

mod error;
mod frame;
mod handles;
mod paragraph;
mod thread_affinity;

pub mod ffi;

pub use error::{Error, ErrorKind, Result};
pub use frame::{
    Affine, BoxShadow, Color, FillRule, Frame, FrameOp, GradientStop, LineCap, LineJoin, Paint,
    Path, PathCommand, PixelRect, Point, Rect, Stroke,
};
pub use handles::{BuildInfo, Context, Engine, MemoryPressure, RasterSurface};
pub use paragraph::{
    ParagraphAffinity, ParagraphCapabilities, ParagraphCaret, ParagraphCluster, ParagraphColor,
    ParagraphDirection, ParagraphEngine, ParagraphFontFeature, ParagraphFontSlant,
    ParagraphFontVariation, ParagraphHitRegion, ParagraphInlineBox, ParagraphInlineObject,
    ParagraphLine, ParagraphOutput, ParagraphOverflow, ParagraphPreedit, ParagraphRange,
    ParagraphRect, ParagraphRequest, ParagraphSize, ParagraphStyle, ParagraphTextAlign,
    ParagraphTextDirection, ParagraphTextStyleRun, ParagraphTextWidthBasis, UnresolvedGlyph,
};

/// Exact bridge ABI accepted by this crate.
pub const ABI_VERSION: u32 = 2;

/// Exact Skia source revision used by both source and prebuilt profiles.
pub const SKIA_REVISION: &str = "cf5c36972b73698eb3939cda147ea47152670312";
