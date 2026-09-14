use std::hash::{Hash, Hasher};

use fission_core::authoring::{IrBuilder, LowerWidget, LoweringContext};
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

impl LowerWidget for CanvasVectorLayer {
    fn lower_dyn(&self, cx: &mut LoweringContext) -> WidgetId {
        let paint = IrBuilder::new(
            WidgetId::derived(self.id.as_u128(), &[1]),
            Op::Paint(PaintOp::DrawPath {
                path: self.path.clone(),
                fill: self.fill.clone(),
                stroke: self.stroke.clone(),
            }),
        )
        .build(cx);

        let mut layout = IrBuilder::new(
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
        fission_core::authoring::custom_widget("InfiniteCanvasVectorLayer", layer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fission_core::env::{Env, RuntimeState};
    use fission_core::ui::{Container, ZStack};
    use fission_ir::op::{Color, LineCap, LineJoin};
    use fission_layout::{LayoutEngine, LayoutSize};

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
        let mut cx = LoweringContext::new(&env, &runtime, None, None);
        let root = fission_core::internal::lower_widget(&widget, &mut cx);

        assert_eq!(root, id);
        let wrapper = cx.ir().nodes.get(&root).expect("wrapper");
        assert_eq!(wrapper.children.len(), 1);
        assert_ne!(wrapper.children[0], root);
        let layout = cx.ir().nodes.get(&wrapper.children[0]).expect("layout");
        assert_eq!(layout.children.len(), 1);
        assert!(matches!(
            cx.ir().nodes[&layout.children[0]].op,
            Op::Paint(PaintOp::DrawPath { .. })
        ));
    }

    #[test]
    fn positioned_vector_path_keeps_its_negative_world_origin() {
        let id = WidgetId::explicit("positioned-canvas-vector-test");
        let vector: Widget = CanvasVectorLayer {
            id,
            path: "M128 128 L228 128".into(),
            width: 1056.0,
            height: 856.0,
            fill: None,
            stroke: None,
        }
        .into();
        let widget: Widget = ZStack {
            id: None,
            children: vec![Container::new(vector)
                .positioned(Some(-128.0), Some(-128.0), None, None)
                .width(1056.0)
                .height(856.0)
                .into()],
        }
        .into();
        let env = Env::default();
        let runtime = RuntimeState::default();
        let mut cx = LoweringContext::new(&env, &runtime, None, None);
        let root = fission_core::internal::lower_widget(&widget, &mut cx);
        let nodes = fission_core::internal::build_layout_tree(cx.ir(), &env);
        let snapshot = LayoutEngine::new()
            .compute_layout(&nodes, root, LayoutSize::new(800.0, 600.0), &|_| 0.0)
            .expect("positioned vector layout");

        let paint_id = WidgetId::derived(id.as_u128(), &[1]);
        assert_eq!(snapshot.nodes[&paint_id].rect.x(), -128.0);
        assert_eq!(snapshot.nodes[&paint_id].rect.y(), -128.0);
    }
}
