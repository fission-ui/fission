use super::semantics::Semantics;
use super::widget_id::WidgetNodeId;
use crate::NodeId;
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
    Group { stable_hash: u64 },
}

pub type LayoutUnit = f32;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum FlexDirection {
    Row,
    Column,
}

impl Default for FlexDirection {
    fn default() -> Self {
        FlexDirection::Row
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum EmbedKind {
    Video,
    Web,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum GridTrack {
    Points(LayoutUnit),
    Percent(f32),
    Fr(f32),
    Auto,
    MinContent,
    MaxContent,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum GridPlacement {
    Auto,
    Line(i16),
    Span(u16),
}

impl Default for GridPlacement {
    fn default() -> Self { Self::Auto }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum FlexWrap {
    NoWrap,
    Wrap,
    WrapReverse,
}

impl Default for FlexWrap {
    fn default() -> Self {
        FlexWrap::NoWrap
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LayoutOp {
    Box {
        width: Option<LayoutUnit>,
        height: Option<LayoutUnit>,
        min_width: Option<LayoutUnit>,
        max_width: Option<LayoutUnit>,
        min_height: Option<LayoutUnit>,
        max_height: Option<LayoutUnit>,
        padding: [LayoutUnit; 4],
        flex_grow: LayoutUnit,
        flex_shrink: LayoutUnit,
        aspect_ratio: Option<f32>,
    },
    Flex {
        direction: FlexDirection,
        wrap: FlexWrap,
        flex_grow: LayoutUnit,
        flex_shrink: LayoutUnit,
        padding: [LayoutUnit; 4],
        gap: Option<LayoutUnit>,
    },
    Grid {
        columns: Vec<GridTrack>,
        rows: Vec<GridTrack>,
        column_gap: Option<LayoutUnit>,
        row_gap: Option<LayoutUnit>,
        padding: [LayoutUnit; 4],
    },
    GridItem {
        row_start: GridPlacement,
        row_end: GridPlacement,
        col_start: GridPlacement,
        col_end: GridPlacement,
    },
    Scroll {
        direction: FlexDirection,
        show_scrollbar: bool,
        width: Option<LayoutUnit>,
        height: Option<LayoutUnit>,
        min_width: Option<LayoutUnit>,
        max_width: Option<LayoutUnit>,
        min_height: Option<LayoutUnit>,
        max_height: Option<LayoutUnit>,
        padding: [LayoutUnit; 4],
    },
    Embed {
        kind: EmbedKind,
        widget_id: WidgetNodeId,
        width: Option<LayoutUnit>,
        height: Option<LayoutUnit>,
    },
    AbsoluteFill,
    Positioned {
        left: Option<LayoutUnit>,
        top: Option<LayoutUnit>,
        right: Option<LayoutUnit>,
        bottom: Option<LayoutUnit>,
        width: Option<LayoutUnit>,
        height: Option<LayoutUnit>,
    },
    ZStack,
    Align,
    Flyout {
        anchor: NodeId,
        content: NodeId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const BLACK: Self = Self {
        r: 0,
        g: 0,
        b: 0,
        a: 255,
    };
    pub const WHITE: Self = Self {
        r: 255,
        g: 255,
        b: 255,
        a: 255,
    };
    pub const RED: Self = Self {
        r: 255,
        g: 0,
        b: 0,
        a: 255,
    };
    pub const GREEN: Self = Self {
        r: 0,
        g: 255,
        b: 0,
        a: 255,
    };
    pub const BLUE: Self = Self {
        r: 0,
        g: 0,
        b: 255,
        a: 255,
    };

    pub fn with_alpha(mut self, a: u8) -> Self {
        self.a = a;
        self
    }
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
pub struct TextStyle {
    pub font_size: LayoutUnit,
    pub color: Color,
    pub underline: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextRun {
    pub text: String,
    pub style: TextStyle,
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
        underline: bool,
        caret_index: Option<usize>,
    },
    DrawRichText {
        runs: Vec<TextRun>,
        caret_index: Option<usize>,
    },
    DrawImage {
        source: String,
        fit: ImageFit,
    },
    DrawPath {
        path: String,
        fill: Option<Fill>,
        stroke: Option<Stroke>,
    },
    DrawSvg {
        content: String,
        fill: Option<Fill>,
        stroke: Option<Stroke>,
    },
}
