use fission_core::internal::{InternalIrBuilder, InternalLowerer, InternalLoweringCx};
use fission_core::ui::Widget;
use fission_ir::{LayoutOp, Op, WidgetId};
use serde::{Deserialize, Serialize};

/// Constrains one child to a width-to-height ratio.
///
/// The ratio must be positive; `16.0 / 9.0`, for example, produces a widescreen
/// box while allowing the surrounding layout to choose its final size.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AspectRatio {
    /// Desired width divided by height.
    pub ratio: f32,
    /// Content laid out inside the ratio-constrained box.
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
