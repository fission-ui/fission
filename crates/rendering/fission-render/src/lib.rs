use fission_ir::op::{
    EmbedKind, ImageAlignment, ImageRequest, RichTextAnnotation, TextParagraphStyle,
};
use fission_ir::WidgetId;
pub use fission_layout::{
    LayoutPoint, LayoutRect, LayoutSize, LayoutUnit, ResolvedParagraphLayout,
};
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
    /// A gradient whose start and end points are normalized to the painted
    /// bounds, where `(0.0, 0.0)` is the top-left and `(1.0, 1.0)` is the
    /// bottom-right.
    LinearGradient {
        start: (f32, f32),
        end: (f32, f32),
        stops: Vec<(f32, Color)>,
        #[serde(default)]
        extend: fission_ir::GradientExtend,
    },
    /// A gradient whose center and radius are normalized to the painted bounds.
    RadialGradient {
        center: (f32, f32),
        radius: f32,
        stops: Vec<(f32, Color)>,
        #[serde(default)]
        extend: fission_ir::GradientExtend,
    },
    /// A gradient sweeping around a center point, normalized to the painted
    /// bounds. Angles are radians, clockwise from the positive x axis.
    SweepGradient {
        center: (f32, f32),
        start_angle: f32,
        end_angle: f32,
        stops: Vec<(f32, Color)>,
        #[serde(default)]
        extend: fission_ir::GradientExtend,
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

/// Strokes for the four edges of a box, in physical order.
///
/// The render-level counterpart of [`fission_ir::BorderSides`], carrying this
/// crate's [`Stroke`] so a backend never has to reach back into the IR.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct BorderSides {
    pub top: Option<Stroke>,
    pub right: Option<Stroke>,
    pub bottom: Option<Stroke>,
    pub left: Option<Stroke>,
}

impl BorderSides {
    /// The same stroke on every edge.
    pub fn uniform(stroke: Stroke) -> Self {
        Self {
            top: Some(stroke.clone()),
            right: Some(stroke.clone()),
            bottom: Some(stroke.clone()),
            left: Some(stroke),
        }
    }

    /// Whether no edge carries a stroke.
    pub fn is_empty(&self) -> bool {
        self.top.is_none() && self.right.is_none() && self.bottom.is_none() && self.left.is_none()
    }

    /// The single stroke shared by all four edges, if there is one.
    ///
    /// Lets a backend take its uniform stroke path when the edges agree rather
    /// than always drawing four of them.
    pub fn as_uniform(&self) -> Option<&Stroke> {
        let top = self.top.as_ref()?;
        (self.right.as_ref() == Some(top)
            && self.bottom.as_ref() == Some(top)
            && self.left.as_ref() == Some(top))
        .then_some(top)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BoxShadow {
    /// Shadow color.
    pub color: Color,
    /// Blur radius in logical points.
    pub blur_radius: LayoutUnit,
    /// Positive or negative spread radius in logical points.
    pub spread_radius: LayoutUnit,
    /// Shadow offset in logical points.
    pub offset: (LayoutUnit, LayoutUnit),
    /// Whether this is an inset shadow.
    pub inset: bool,
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
    /// Extended typography carried by the same backend-neutral text path.
    #[serde(default)]
    pub typography: fission_ir::op::TextTypography,
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
    BackdropFilter {
        rect: LayoutRect,
        filter: fission_ir::op::BackdropFilter,
        corner_radius: LayoutUnit,
        bounds: LayoutRect,
        node_id: Option<WidgetId>,
        /// Per-corner override. Replaces `corner_radius` when present.
        #[serde(default)]
        corner_radii: Option<fission_ir::CornerRadii>,
    },
    DrawRect {
        rect: LayoutRect,
        fill: Option<Fill>,
        stroke: Option<Stroke>,
        corner_radius: LayoutUnit,
        shadow: Option<BoxShadow>,
        bounds: LayoutRect,
        node_id: Option<WidgetId>,
        /// Per-corner override. Replaces `corner_radius` when present.
        #[serde(default)]
        corner_radii: Option<fission_ir::CornerRadii>,
        /// Per-edge strokes. Replaces `stroke` when present.
        #[serde(default)]
        border_sides: Option<BorderSides>,
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
        #[serde(default)]
        resolved_layout: Option<ResolvedParagraphLayout>,
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
        /// The paragraph resolved during layout. This is the authoritative
        /// wrapping and text-geometry decision for downstream rendering.
        #[serde(default)]
        resolved_layout: Option<ResolvedParagraphLayout>,
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

impl DisplayOp {
    /// Returns the text this op draws, if it draws any.
    ///
    /// The display-list counterpart of [`fission_ir::PaintOp::text`]. Whether a
    /// widget produced one unstyled string or styled runs is an implementation
    /// detail of that widget, so callers asking what the user reads should go
    /// through this rather than matching a single variant.
    pub fn text(&self) -> Option<std::borrow::Cow<'_, str>> {
        match self {
            Self::DrawText { text, .. } => Some(std::borrow::Cow::Borrowed(text.as_str())),
            Self::DrawRichText { runs, .. } => match runs.as_slice() {
                [] => None,
                [run] => Some(std::borrow::Cow::Borrowed(run.text.as_str())),
                runs => Some(std::borrow::Cow::Owned(
                    runs.iter().map(|run| run.text.as_str()).collect(),
                )),
            },
            _ => None,
        }
    }
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
    /// How this layer's pixels combine with what is already painted beneath it.
    ///
    /// Carried through to the backend even when the active backend cannot honour
    /// it. A backend that only does source-over renders the layer normally; the
    /// information is not discarded on the way down, so a backend that can blend
    /// gets it without another pass through the pipeline.
    #[serde(default)]
    pub blend_mode: fission_ir::BlendMode,
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
            blend_mode: fission_ir::BlendMode::Normal,
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
#[doc(hidden)]
pub mod image_cache_store;
