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
    ClipRoundedRect {
        rect: LayoutRect,
        radius: LayoutUnit,
    },
    OpacityLayer {
        alpha: f32,
        bounds: LayoutRect,
    },
    Translate(LayoutPoint),
    Transform([LayoutUnit; 16]),
    CachedScene {
        cache_key: u64,
        bounds: LayoutRect,
        list: Box<DisplayList>,
    },
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
        wrap: bool,
        caret_index: Option<usize>,
    },
    DrawRichText {
        runs: Vec<TextRun>,
        position: LayoutPoint,
        bounds: LayoutRect,
        node_id: Option<NodeId>,
        wrap: bool,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LayerClip {
    Rect(LayoutRect),
    RoundedRect {
        rect: LayoutRect,
        radius: LayoutUnit,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayerStyle {
    pub clip: Option<LayerClip>,
    pub opacity: f32,
    pub transform: Option<[LayoutUnit; 16]>,
    pub transform_clip: bool,
    pub cache_key: Option<u64>,
    pub content_cache_key: Option<u64>,
}

impl Default for LayerStyle {
    fn default() -> Self {
        Self {
            clip: None,
            opacity: 1.0,
            transform: None,
            transform_clip: true,
            cache_key: None,
            content_cache_key: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RenderNode {
    Layer(RenderLayer),
    Paint(DisplayList),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RenderLayer {
    pub node_id: Option<NodeId>,
    pub bounds: LayoutRect,
    pub style: LayerStyle,
    pub children: Vec<RenderNode>,
}

impl RenderLayer {
    pub fn new(bounds: LayoutRect) -> Self {
        Self {
            node_id: None,
            bounds,
            style: LayerStyle::default(),
            children: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RenderScene {
    pub bounds: LayoutRect,
    pub roots: Vec<RenderNode>,
}

impl RenderScene {
    pub fn new(bounds: LayoutRect) -> Self {
        Self {
            bounds,
            roots: Vec::new(),
        }
    }

    pub fn from_display_list(display_list: DisplayList) -> Self {
        Self {
            bounds: display_list.bounds,
            roots: vec![RenderNode::Paint(display_list)],
        }
    }

    pub fn flatten(&self) -> DisplayList {
        let mut list = DisplayList::new(self.bounds);
        for root in &self.roots {
            flatten_render_node(root, &mut list.ops);
        }
        list
    }
}

fn flatten_render_node(node: &RenderNode, out: &mut Vec<DisplayOp>) {
    match node {
        RenderNode::Paint(list) => out.extend(list.ops.clone()),
        RenderNode::Layer(layer) => {
            let needs_save = layer.style.clip.is_some()
                || layer.style.transform.is_some()
                || (layer.style.opacity - 1.0).abs() > 0.001;
            if needs_save {
                out.push(DisplayOp::Save);
            }
            if let Some(clip) = &layer.style.clip {
                match clip {
                    LayerClip::Rect(rect) => out.push(DisplayOp::ClipRect(*rect)),
                    LayerClip::RoundedRect { rect, radius } => {
                        out.push(DisplayOp::ClipRoundedRect {
                            rect: *rect,
                            radius: *radius,
                        })
                    }
                }
            }
            if (layer.style.opacity - 1.0).abs() > 0.001 {
                out.push(DisplayOp::OpacityLayer {
                    alpha: layer.style.opacity,
                    bounds: layer.bounds,
                });
            }
            if let Some(transform) = layer.style.transform {
                out.push(DisplayOp::Transform(transform));
            }
            for child in &layer.children {
                flatten_render_node(child, out);
            }
            if needs_save {
                out.push(DisplayOp::Restore);
            }
        }
    }
}

pub trait Renderer {
    fn render_scene(&mut self, scene: &RenderScene) -> anyhow::Result<()>;

    fn render(&mut self, display_list: &DisplayList) -> anyhow::Result<()> {
        self.render_scene(&RenderScene::from_display_list(display_list.clone()))
    }
}
