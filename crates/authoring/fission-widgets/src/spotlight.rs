use fission_core::{
    internal::{InternalIrBuilder, InternalLowerer, InternalLoweringCx},
    LayoutOp, Op, Widget,
};
use fission_ir::WidgetId;
use std::sync::Arc;

/// Lays out an inverse overlay around an already-laid-out anchor.
///
/// The five children are ordered as top shade, bottom shade, left shade, right
/// shade, and focus ring. `Spotlight` owns only this geometry. Callers remain
/// responsible for paint, interaction, portal layering, dismissal, and
/// accessibility semantics.
#[derive(Debug)]
pub struct Spotlight {
    pub anchor: WidgetId,
    pub padding: f32,
    pub children: [Widget; 5],
}

impl From<Spotlight> for Widget {
    fn from(spotlight: Spotlight) -> Self {
        fission_core::internal::custom_render_widget(fission_core::CustomWidget {
            debug_tag: "Spotlight".into(),
            lowerer: Some(Arc::new(SpotlightLowerer {
                anchor: spotlight.anchor,
                padding: spotlight.padding,
                children: spotlight.children,
            })),
            render_object: None,
        })
    }
}

#[derive(Debug)]
struct SpotlightLowerer {
    anchor: WidgetId,
    padding: f32,
    children: [Widget; 5],
}

impl InternalLowerer for SpotlightLowerer {
    fn lower_dyn(&self, cx: &mut InternalLoweringCx) -> WidgetId {
        let children = self
            .children
            .iter()
            .map(|child| fission_core::internal::lower_widget(child, cx))
            .collect::<Vec<_>>();
        let mut builder = InternalIrBuilder::new(
            cx.next_node_id(),
            Op::Layout(LayoutOp::Spotlight {
                anchor: self.anchor,
                padding: self.padding,
            }),
        );
        for child in children {
            builder.add_child(child);
        }
        builder.build(cx)
    }
}
