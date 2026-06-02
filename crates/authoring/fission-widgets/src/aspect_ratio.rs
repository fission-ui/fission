use fission_core::internal::{InternalIrBuilder, InternalLowerer, InternalLoweringCx};
use fission_core::ui::Widget;
use fission_ir::{LayoutOp, Op, WidgetId};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AspectRatio {
    pub ratio: f32,
    pub child: Widget,
}

impl From<AspectRatio> for Widget {
    fn from(component: AspectRatio) -> Self {
        let this = &component;

        fission_core::internal::custom_render_widget(fission_core::internal::InternalRenderNode {
            debug_tag: "AspectRatio".into(),
            lowerer: Some(std::sync::Arc::new(AspectRatioLowerer {
                ratio: this.ratio,
                child: this.child.clone(),
            })),
            render_object: None,
        })
    }
}

#[derive(Debug)]
struct AspectRatioLowerer {
    ratio: f32,
    child: Widget,
}

impl InternalLowerer for AspectRatioLowerer {
    fn lower_dyn(&self, cx: &mut InternalLoweringCx) -> WidgetId {
        let child_id = fission_core::internal::lower_widget(&self.child, cx);
        let id = cx.next_node_id();

        let mut builder = InternalIrBuilder::new(
            id,
            Op::Layout(LayoutOp::Box {
                width: None,
                height: None,
                min_width: None,
                max_width: None,
                min_height: None,
                max_height: None,
                padding: [0.0; 4],
                flex_grow: 0.0,
                flex_shrink: 0.0,
                aspect_ratio: Some(self.ratio),
            }),
        );
        builder.add_child(child_id);
        builder.build(cx)
    }

    fn stable_key(&self) -> u64 {
        // Hash the ratio (unsafe float hash, but okay for MVP UI)
        let bits = self.ratio.to_bits();
        bits as u64
    }
}
