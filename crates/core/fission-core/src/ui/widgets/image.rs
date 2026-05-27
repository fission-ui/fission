use crate::lowering::{LoweringContext, NodeBuilder};
use crate::ui::traits::Lower;
use fission_ir::{
    op::{ImageFit, LayoutOp, Op, PaintOp},
    NodeId,
};
use serde::{Deserialize, Serialize};

pub use fission_ir::op::{
    HttpHeader, ImageAlignment, ImageCachePolicy, ImageErrorBehavior, ImageLoadingBehavior,
    ImageRequest, ImageSource,
};

/// Displays an image from an asset, file, network URL, memory buffer, or inline SVG.
///
/// `Image` is declarative: it describes the image source and presentation. The
/// active shell is responsible for loading, decoding, caching, and repainting
/// when the image becomes available.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Image {
    /// Explicit node identity.
    pub id: Option<NodeId>,
    /// Typed image request consumed by the shell image pipeline.
    pub request: ImageRequest,
    /// Fixed width in layout points.
    pub width: Option<f32>,
    /// Fixed height in layout points.
    pub height: Option<f32>,
    /// How the image is scaled to fit its layout box.
    pub fit: ImageFit,
    /// How fitted image content is positioned inside its layout box.
    pub alignment: ImageAlignment,
}

impl Default for Image {
    fn default() -> Self {
        Self {
            id: None,
            request: ImageRequest::default(),
            width: None,
            height: None,
            fit: ImageFit::Contain,
            alignment: ImageAlignment::Center,
        }
    }
}

impl Image {
    pub fn asset(path: impl Into<String>) -> Self {
        Self::from_source(ImageSource::Asset { path: path.into() })
    }

    pub fn file(path: impl Into<String>) -> Self {
        Self::from_source(ImageSource::File { path: path.into() })
    }

    pub fn network(url: impl Into<String>) -> Self {
        Self::from_source(ImageSource::Network {
            url: url.into(),
            headers: Vec::new(),
            cache_policy: ImageCachePolicy::Default,
        })
    }

    pub fn memory(bytes: impl Into<Vec<u8>>) -> Self {
        Self::from_source(ImageSource::Memory {
            bytes: bytes.into(),
            mime_type: None,
        })
    }

    pub fn svg_text(content: impl Into<String>) -> Self {
        Self::from_source(ImageSource::SvgText {
            content: content.into(),
        })
    }

    pub fn from_source(source: ImageSource) -> Self {
        Self {
            request: ImageRequest {
                source,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    pub fn id(mut self, id: NodeId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    pub fn height(mut self, height: f32) -> Self {
        self.height = Some(height);
        self
    }

    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.width = Some(width);
        self.height = Some(height);
        self
    }

    pub fn fit(mut self, fit: ImageFit) -> Self {
        self.fit = fit;
        self
    }

    pub fn alignment(mut self, alignment: ImageAlignment) -> Self {
        self.alignment = alignment;
        self
    }

    pub fn semantic_label(mut self, label: impl Into<String>) -> Self {
        self.request.semantic_label = Some(label.into());
        self
    }

    pub fn cache_size(mut self, width: u32, height: u32) -> Self {
        self.request.cache_width = Some(width);
        self.request.cache_height = Some(height);
        self
    }

    pub fn loading(mut self, loading: ImageLoadingBehavior) -> Self {
        self.request.loading = loading;
        self
    }

    pub fn error(mut self, error: ImageErrorBehavior) -> Self {
        self.request.error = error;
        self
    }

    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        if let ImageSource::Network { headers, .. } = &mut self.request.source {
            headers.push(HttpHeader {
                name: name.into(),
                value: value.into(),
            });
        }
        self
    }

    pub fn cache_policy(mut self, cache_policy: ImageCachePolicy) -> Self {
        if let ImageSource::Network {
            cache_policy: policy,
            ..
        } = &mut self.request.source
        {
            *policy = cache_policy;
        }
        self
    }

    pub fn into_node(self) -> crate::ui::Node {
        crate::ui::Node::Image(self)
    }
}

impl Lower for Image {
    fn lower(&self, cx: &mut LoweringContext) -> NodeId {
        let layout_id = self.id.unwrap_or_else(|| cx.next_node_id());
        let paint_op = match &self.request.source {
            ImageSource::SvgText { content } => PaintOp::DrawSvg {
                content: content.clone(),
                fill: None,
                stroke: None,
            },
            _ => PaintOp::DrawImage {
                request: self.request.clone(),
                fit: self.fit,
                alignment: self.alignment,
            },
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
