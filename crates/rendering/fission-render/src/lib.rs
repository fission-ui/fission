use fission_ir::op::{
    EmbedKind, ImageAlignment, ImageRequest, RichTextAnnotation, TextParagraphStyle,
};
use fission_ir::WidgetId;
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
    pub font_family: Option<String>,
    pub locale: Option<String>,
    pub font_weight: u16,
    pub font_style: fission_ir::op::FontStyle,
    pub line_height: Option<LayoutUnit>,
    pub letter_spacing: LayoutUnit,
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
        node_id: Option<WidgetId>,
    },
    DrawText {
        text: String,
        position: LayoutPoint,
        size: LayoutUnit,
        color: Color,
        bounds: LayoutRect,
        node_id: Option<WidgetId>,
        underline: bool,
        wrap: bool,
        caret_index: Option<usize>,
        caret_color: Option<Color>,
        caret_width: Option<LayoutUnit>,
        caret_height: Option<LayoutUnit>,
        caret_radius: Option<LayoutUnit>,
        paragraph_style: Option<TextParagraphStyle>,
    },
    DrawRichText {
        runs: Vec<TextRun>,
        position: LayoutPoint,
        bounds: LayoutRect,
        node_id: Option<WidgetId>,
        wrap: bool,
        caret_index: Option<usize>,
        caret_color: Option<Color>,
        caret_width: Option<LayoutUnit>,
        caret_height: Option<LayoutUnit>,
        caret_radius: Option<LayoutUnit>,
        paragraph_style: Option<TextParagraphStyle>,
        #[serde(default)]
        annotations: Vec<RichTextAnnotation>,
    },
    DrawImage {
        rect: LayoutRect,
        request: ImageRequest,
        fit: ImageFit,
        alignment: ImageAlignment,
        bounds: LayoutRect,
        node_id: Option<WidgetId>,
    },
    DrawPath {
        path: String,
        fill: Option<Fill>,
        stroke: Option<Stroke>,
        bounds: LayoutRect,
        node_id: Option<WidgetId>,
    },
    DrawSvg {
        content: String,
        fill: Option<Fill>,
        stroke: Option<Stroke>,
        bounds: LayoutRect,
        node_id: Option<WidgetId>,
    },
    DrawSurface {
        rect: LayoutRect,
        surface_id: u64,
        position: u64,
        bounds: LayoutRect,
        node_id: Option<WidgetId>,
    },
}

pub fn embed_surface_id(kind: &EmbedKind, widget_id: WidgetId) -> u64 {
    let kind_tag = match kind {
        EmbedKind::Video => 0xF151_0000_0000_0001,
        EmbedKind::Web => 0xF151_0000_0000_0002,
        EmbedKind::Custom(_) => 0xF151_0000_0000_0003,
    };
    let raw = widget_id.as_u128();
    (raw as u64) ^ ((raw >> 64) as u64).rotate_left(13) ^ kind_tag
}

pub fn surface_placeholder_color(surface_id: u64, position: u64) -> Color {
    Color {
        r: (surface_id.wrapping_mul(50).wrapping_add(position / 20) % 255) as u8,
        g: (surface_id.wrapping_mul(30).wrapping_add(position / 30) % 255) as u8,
        b: (surface_id.wrapping_mul(70).wrapping_add(position / 40) % 255) as u8,
        a: 255,
    }
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
    pub node_id: Option<WidgetId>,
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

#[cfg(test)]
mod tests {
    use super::{embed_surface_id, surface_placeholder_color};
    use fission_ir::{EmbedKind, WidgetId};

    #[test]
    fn embed_surface_id_is_stable_and_kind_specific() {
        let id = WidgetId::explicit("embed.demo");

        assert_eq!(
            embed_surface_id(&EmbedKind::Video, id),
            embed_surface_id(&EmbedKind::Video, id)
        );
        assert_ne!(
            embed_surface_id(&EmbedKind::Video, id),
            embed_surface_id(&EmbedKind::Web, id)
        );
    }

    #[test]
    fn surface_placeholder_color_uses_wrapping_arithmetic() {
        let color = surface_placeholder_color(u64::MAX, u64::MAX);

        assert_eq!(color.a, 255);
    }
}
