use fission_ir::NodeId;
pub use fission_layout::{LayoutPoint, LayoutRect, LayoutSize, LayoutUnit};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Fill {
    Solid(Color),
    LinearGradient {
        start: (f32, f32),
        end: (f32, f32),
        stops: Vec<(f32, Color)>,
    },
    RadialGradient {
        center: (f32, f32),
        radius: f32,
        stops: Vec<(f32, Color)>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LineCap {
    Butt,
    Round,
    Square,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LineJoin {
    Miter,
    Round,
    Bevel,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stroke {
    pub fill: Fill,
    pub width: LayoutUnit,
    pub dash_array: Option<Vec<f32>>,
    pub line_cap: LineCap,
    pub line_join: LineJoin,
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
    /// Optional background highlight color for this run.
    pub background_color: Option<Color>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextRun {
    pub text: String,
    pub style: TextStyle,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DisplayOp {
    Save,
    Restore,
    ClipRect(LayoutRect),
    ClipRoundedRect { rect: LayoutRect, radius: LayoutUnit },
    Translate(LayoutPoint),
    Transform([LayoutUnit; 16]),
    DrawRect {
        rect: LayoutRect,
        fill: Option<Fill>,
        stroke: Option<Stroke>,
        corner_radius: LayoutUnit,
        shadow: Option<BoxShadow>,
        bounds: LayoutRect,
        node_id: Option<NodeId>,
    },
    DrawText {
        text: String,
        position: LayoutPoint,
        size: LayoutUnit,
        color: Color,
        bounds: LayoutRect,
        node_id: Option<NodeId>,
        underline: bool,
        caret_index: Option<usize>,
    },
    DrawRichText {
        runs: Vec<TextRun>,
        position: LayoutPoint,
        bounds: LayoutRect,
        node_id: Option<NodeId>,
        caret_index: Option<usize>,
    },
    DrawImage {
        rect: LayoutRect,
        source: String,
        fit: ImageFit,
        bounds: LayoutRect,
        node_id: Option<NodeId>,
    },
    DrawPath {
        path: String,
        fill: Option<Fill>,
        stroke: Option<Stroke>,
        bounds: LayoutRect,
        node_id: Option<NodeId>,
    },
    DrawSvg {
        content: String,
        fill: Option<Fill>,
        stroke: Option<Stroke>,
        bounds: LayoutRect,
        node_id: Option<NodeId>,
    },
    DrawSurface {
        rect: LayoutRect,
        surface_id: u64,
        position: u64,
        bounds: LayoutRect,
        node_id: Option<NodeId>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DisplayList {
    pub ops: Vec<DisplayOp>,
    pub bounds: LayoutRect,
}

impl DisplayList {
    pub fn new(bounds: LayoutRect) -> Self {
        Self {
            ops: Vec::new(),
            bounds,
        }
    }

    pub fn push(&mut self, op: DisplayOp) {
        self.ops.push(op);
    }
}

pub trait Renderer {
    fn render(&mut self, display_list: &DisplayList) -> anyhow::Result<()>;
}
