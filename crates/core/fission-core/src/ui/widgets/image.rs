use crate::lowering::{LoweringContext, NodeBuilder};
use crate::ui::traits::Lower;
use fission_ir::{
    op::{ImageFit, LayoutOp, Op, PaintOp},
    NodeId,
};
use serde::{Deserialize, Serialize};

fn resolve_svg_content(source: &str) -> Option<String> {
    let trimmed = source.trim_start();
    if trimmed.starts_with("<svg") {
        return Some(source.to_string());
    }

    if source.to_ascii_lowercase().ends_with(".svg") {
        return std::fs::read_to_string(source).ok();
    }

    None
}

/// A raster image widget.
///
/// Displays an image from a URL or asset path. The `fit` property controls
/// how the image is scaled within its layout box.
///
/// # Example
///
/// ```rust,ignore
/// Image {
///     source: "https://example.com/photo.jpg".into(),
///     width: Some(200.0),
///     height: Some(150.0),
///     fit: Some(ImageFit::Cover),
///     ..Default::default()
/// }
/// ```
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Image {
    /// Explicit node identity.
    pub id: Option<NodeId>,
    /// URL or asset path to the image.
    pub source: String,
    /// Fixed width in layout points.
    pub width: Option<f32>,
    /// Fixed height in layout points.
    pub height: Option<f32>,
    /// How the image is scaled to fit its layout box (default: `Contain`).
    pub fit: Option<ImageFit>,
}

impl Image {
    pub fn into_node(self) -> crate::ui::Node {
        crate::ui::Node::Image(self)
    }
}

impl Lower for Image {
    fn lower(&self, cx: &mut LoweringContext) -> NodeId {
        let layout_id = self.id.unwrap_or_else(|| cx.next_node_id());
        let paint_op = if let Some(content) = resolve_svg_content(&self.source) {
            PaintOp::DrawSvg {
                content,
                fill: None,
                stroke: None,
            }
        } else {
            PaintOp::DrawImage {
                source: self.source.clone(),
                fit: self.fit.unwrap_or(ImageFit::Contain),
            }
        };
        let paint_id = NodeBuilder::new(cx.next_node_id(), Op::Paint(paint_op)).build(cx);

        let mut layout_builder = NodeBuilder::new(
            layout_id,
            Op::Layout(LayoutOp::Box {
                width: self.width,
                height: self.height,
                min_width: None,
                max_width: None,
                min_height: None,
                max_height: None,
                padding: [0.0; 4],
                flex_grow: 0.0,
                flex_shrink: 0.0,
                aspect_ratio: None,
            }),
        );
        layout_builder.add_child(paint_id);
        layout_builder.build(cx)
    }
}
