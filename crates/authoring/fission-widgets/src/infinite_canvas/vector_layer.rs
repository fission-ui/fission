use std::hash::{Hash, Hasher};
use std::sync::Arc;

use fission_core::internal::{
    InternalIrBuilder, InternalLowerer, InternalLoweringCx, InternalRenderNode,
};
use fission_core::ui::Widget;
use fission_core::{LayoutOp, Op, WidgetId};
use fission_ir::op::{BoxStyle, Fill, Length, PaintOp, Stroke};

#[derive(Clone)]
pub(crate) struct CanvasVectorLayer {
    pub id: WidgetId,
    pub path: String,
    pub width: f32,
    pub height: f32,
    pub fill: Option<Fill>,
    pub stroke: Option<Stroke>,
}

impl std::fmt::Debug for CanvasVectorLayer {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CanvasVectorLayer")
            .field("id", &self.id)
            .field("width", &self.width)
            .field("height", &self.height)
            .finish_non_exhaustive()
    }
}

impl InternalLowerer for CanvasVectorLayer {
    fn lower_dyn(&self, cx: &mut InternalLoweringCx) -> WidgetId {
        let paint = InternalIrBuilder::new(
            WidgetId::derived(self.id.as_u128(), &[1]),
            Op::Paint(PaintOp::DrawPath {
                path: self.path.clone(),
                fill: self.fill.clone(),
                stroke: self.stroke.clone(),
            }),
        )
        .build(cx);

        let mut layout = InternalIrBuilder::new(
            WidgetId::derived(self.id.as_u128(), &[0]),
            Op::Layout(LayoutOp::StyledBox {
                style: BoxStyle {
                    width: Some(Length::points(self.width.max(0.0))),
                    height: Some(Length::points(self.height.max(0.0))),
                    ..Default::default()
                },
                flex_grow: 0.0,
                flex_shrink: 0.0,
            }),
        );
        layout.add_child(paint);
        layout.build(cx)
    }

    fn widget_id(&self) -> Option<WidgetId> {
        Some(self.id)
    }

    fn stable_key(&self) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.id.hash(&mut hasher);
        self.path.hash(&mut hasher);
        self.width.to_bits().hash(&mut hasher);
        self.height.to_bits().hash(&mut hasher);
        hasher.finish()
    }
}

impl From<CanvasVectorLayer> for Widget {
    fn from(layer: CanvasVectorLayer) -> Self {
        fission_core::internal::custom_render_widget(InternalRenderNode {
            debug_tag: "InfiniteCanvasVectorLayer".into(),
            lowerer: Some(Arc::new(layer)),
            render_object: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fission_core::env::{Env, RuntimeState};
    use fission_ir::op::{Color, LineCap, LineJoin};

    #[test]
    fn vector_layer_has_distinct_stable_wrapper_layout_and_paint_nodes() {
        let id = WidgetId::explicit("canvas-vector-test");
        let widget: Widget = CanvasVectorLayer {
            id,
            path: "M0 0 L10 10".into(),
            width: 20.0,
            height: 20.0,
            fill: None,
            stroke: Some(Stroke {
                fill: Fill::Solid(Color::BLACK),
                width: 1.0,
                dash_array: None,
                line_cap: LineCap::Butt,
                line_join: LineJoin::Miter,
            }),
        }
        .into();
        let env = Env::default();
        let runtime = RuntimeState::default();
        let mut cx = InternalLoweringCx::new(&env, &runtime, None, None);
        let root = fission_core::internal::lower_widget(&widget, &mut cx);

        assert_eq!(root, id);
        let wrapper = cx.ir.nodes.get(&root).expect("wrapper");
        assert_eq!(wrapper.children.len(), 1);
        assert_ne!(wrapper.children[0], root);
        let layout = cx.ir.nodes.get(&wrapper.children[0]).expect("layout");
        assert_eq!(layout.children.len(), 1);
        assert!(matches!(
            cx.ir.nodes[&layout.children[0]].op,
            Op::Paint(PaintOp::DrawPath { .. })
        ));
    }
}
