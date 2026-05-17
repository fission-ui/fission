use crate::lowering::{LoweringContext, NodeBuilder};
use crate::ui::traits::Lower;
use fission_ir::{
    op::{EmbedKind, LayoutOp, Op},
    NodeId, WidgetNodeId,
};
use serde::{Deserialize, Serialize};

/// A platform-native video player widget.
///
/// The video is rendered by the platform's native player and embedded into the
/// Fission layout as an opaque surface. Use [`BuildCtx::video_controls`] to
/// create play/pause/seek action envelopes.
///
/// # Example
///
/// ```rust,ignore
/// Video {
///     source: "https://example.com/clip.mp4".into(),
///     width: Some(640.0),
///     height: Some(360.0),
///     autoplay: true,
///     loop_playback: false,
///     ..Default::default()
/// }
/// .build(ctx, view);
/// ```
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Video {
    /// Stable widget identity (auto-derived from `source` if `None`).
    pub id: Option<WidgetNodeId>,
    /// URL or asset path to the video file.
    pub source: String,
    /// Fixed width in layout points.
    pub width: Option<f32>,
    /// Fixed height in layout points.
    pub height: Option<f32>,
    /// Whether to start playing immediately.
    pub autoplay: bool,
    /// Whether to loop playback when the video ends.
    pub loop_playback: bool,
}

impl Video {
    pub fn into_node(self) -> crate::ui::Node {
        crate::ui::Node::Video(self)
    }
}

impl Lower for Video {
    fn lower(&self, cx: &mut LoweringContext) -> NodeId {
        let widget_id = self
            .id
            .unwrap_or_else(|| WidgetNodeId::explicit(&self.source));
        let layout_id = cx.widget_node_id(widget_id);

        let embed_id = NodeBuilder::new(
            cx.next_node_id(),
            Op::Layout(LayoutOp::Embed {
                kind: EmbedKind::Video,
                widget_id,
                width: self.width,
                height: self.height,
            }),
        )
        .build(cx);

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
        layout_builder.add_child(embed_id);
        layout_builder.build(cx)
    }
}
