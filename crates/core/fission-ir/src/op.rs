use super::semantics::Semantics;
use serde::{Deserialize, Serialize};

// The fundamental operations that can be performed in the Core IR.
// These are low-level, platform-agnostic, and deterministic.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Op {
    Structural(StructuralOp),
    Layout(LayoutOp),
    Paint(PaintOp),
    Semantics(Semantics),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum StructuralOp {
    Group,
}

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
pub enum EmbedKind {
    Video,
    Web,
    Custom,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LayoutOp {
    Box { 
        width: Option<LayoutUnit>,
        height: Option<LayoutUnit>,
        padding: [LayoutUnit; 4], 
    },
    Flex {
        direction: FlexDirection,
        flex_grow: LayoutUnit,
        flex_shrink: LayoutUnit,
        padding: [LayoutUnit; 4],
    },
    Scroll {
        direction: FlexDirection, 
        show_scrollbar: bool,
        width: Option<LayoutUnit>,
        height: Option<LayoutUnit>,
        padding: [LayoutUnit; 4],
    },
    Embed {
        kind: EmbedKind,
    },
    AbsoluteFill,
    Grid,
    Stack,
    Align,
}

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

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BoxShadow {
    pub color: Color,
    pub blur_radius: LayoutUnit,
    pub offset: (LayoutUnit, LayoutUnit),
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ImageFit {
    Contain,
    Cover,
    Fill,
    None,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PaintOp {
    DrawRect { 
        fill: Option<Fill>,
        stroke: Option<Stroke>,
        corner_radius: LayoutUnit,
        shadow: Option<BoxShadow>,
    },
    DrawText {
        text: String,
        size: LayoutUnit,
        color: Color,
    },
    DrawImage {
        source: String,
        fit: ImageFit,
    },
}
