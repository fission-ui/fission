use super::semantics::Semantics;
use serde::{Deserialize, Serialize};

// The fundamental operations that can be performed in the Core IR.
// These are low-level, platform-agnostic, and deterministic.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Op {
    // Structural operations (tree manipulation, identity)
    Structural(StructuralOp),

    // Layout operations (size, position, flex properties)
    Layout(LayoutOp),

    // Painting operations (drawing primitives)
    Paint(PaintOp),

    // Semantic operations (accessibility, hit testing, actions)
    Semantics(Semantics),

    // Input operations (e.g., event handlers, focus management) - TBD
    // Input(InputOp),
}

// --- Structural Operations --- //

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum StructuralOp {
    // Represents a logical grouping or an identity node without specific layout/paint.
    // Useful for grouping children or providing a stable NodeId.
    Group,
}

// --- Layout Operations --- //

pub type LayoutUnit = f32;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum FlexDirection {
    Row,
    Column,
}

impl Default for FlexDirection {
    fn default() -> Self { FlexDirection::Row }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum LayoutOp {
    // A basic rectangular box with optional fixed size.
    Box { 
        width: Option<LayoutUnit>,
        height: Option<LayoutUnit>,
    },
    // A flex container that lays out its children in a single direction.
    Flex {
        direction: FlexDirection,
        flex_grow: LayoutUnit,
        flex_shrink: LayoutUnit,
    },
    // A node that absolutely fills its parent (used for content layers like paint).
    AbsoluteFill,
    
    // Placeholder for grid layout
    Grid,
    // Placeholder for stack layout (overlapping children)
    Stack,
    // Placeholder for alignment (aligning a single child within bounds)
    Align,
}

// --- Painting Operations --- //

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

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Fill {
    pub color: Color,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Stroke {
    pub color: Color,
    pub width: LayoutUnit,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PaintOp {
    DrawRect { 
        fill: Option<Fill>,
        stroke: Option<Stroke>,
    },
    DrawText {
        text: String,
        size: LayoutUnit,
        color: Color,
    },
}
