use anyhow::Result;
use fission_ir::NodeId;
use serde::{Deserialize, Serialize};

// Re-export layout types so consumers of fission-render can access them
pub use fission_layout::{LayoutRect, LayoutUnit, LayoutPoint, LayoutSize};

pub const DISPLAY_LIST_VERSION: u32 = 1;

// Basic Color definition
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const BLACK: Self = Self { r: 0, g: 0, b: 0, a: 255 };
    pub const WHITE: Self = Self { r: 255, g: 255, b: 255, a: 255 };
    pub const RED: Self = Self { r: 255, g: 0, b: 0, a: 255 };
    pub const GREEN: Self = Self { r: 0, g: 255, b: 0, a: 255 };
    pub const BLUE: Self = Self { r: 0, g: 0, b: 255, a: 255 };
}

// Style attributes for drawing operations
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Fill {
    pub color: Color,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Stroke {
    pub color: Color,
    pub width: LayoutUnit,
}

// Removed BoxShadow struct

// Paint bounds for a DisplayOp, typically derived from LayoutRect but can be expanded for effects.
pub type PaintBounds = LayoutRect;

// Represents a single drawing or state-modifying operation in the DisplayList.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DisplayOp {
    // State operations
    Save, // Push current graphics state onto a stack
    Restore, // Pop graphics state from stack

    // Transform operations
    Translate(LayoutPoint),
    Scale(LayoutUnit, LayoutUnit),

    // Clipping operations
    ClipRect(LayoutRect), // Restrict drawing to this rectangle

    // Drawing operations
    DrawRect { rect: LayoutRect, fill: Option<Fill>, stroke: Option<Stroke>, corner_radius: LayoutUnit, /* Removed shadow: Option<BoxShadow>, */ bounds: PaintBounds, node_id: Option<NodeId> }, 
    DrawText {
        text: String,
        position: LayoutPoint, // Top-left corner of the text
        size: LayoutUnit,
        color: Color,
        bounds: LayoutRect, // Bounding box for the text, for layout/clipping
        node_id: Option<NodeId>,
    },
}

// The DisplayList itself, an ordered sequence of drawing operations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DisplayList {
    pub version: u32,
    pub bounds: LayoutRect, // Bounding box encompassing all drawing operations
    pub ops: Vec<DisplayOp>,
}

impl DisplayList {
    pub fn new(bounds: LayoutRect) -> Self {
        Self {
            version: DISPLAY_LIST_VERSION,
            bounds,
            ops: Vec::new(),
        }
    }

    pub fn push(&mut self, op: DisplayOp) {
        self.ops.push(op);
    }
}

// The Renderer trait, consumed by platform shells to render DisplayLists.
pub trait Renderer {
    fn render(&mut self, display_list: &DisplayList) -> Result<()>;
}