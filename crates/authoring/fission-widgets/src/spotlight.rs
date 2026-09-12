use fission_core::{
    internal::{IrBuilder, LowerWidget, LoweringContext},
    LayoutOp, Op, Widget,
};
use fission_ir::WidgetId;

/// Lays out an inverse overlay around an already-laid-out anchor.
///
/// The five children are ordered as top shade, bottom shade, left shade, right
/// shade, and focus ring. `Spotlight` owns only this geometry. Callers remain
/// responsible for paint, interaction, portal layering, dismissal, and
/// accessibility semantics.
#[derive(Debug)]
pub struct Spotlight {
    /// Stable ID of the already-laid-out widget to reveal.
    pub anchor: WidgetId,
    /// Logical-pixel clearance between the anchor bounds and focus ring.
    pub padding: f32,
    /// Top, bottom, left, right, and focus-ring children in that order.
    pub children: [Widget; 5],
}

impl From<Spotlight> for Widget {
    fn from(spotlight: Spotlight) -> Self {
        fission_core::authoring::custom_widget(
            "Spotlight",
            SpotlightLowerer {
                anchor: spotlight.anchor,
                padding: spotlight.padding,
                children: spotlight.children,
            },
        )
    }
}

#[derive(Debug)]
struct SpotlightLowerer {
    anchor: WidgetId,
    padding: f32,
    children: [Widget; 5],
}

impl LowerWidget for SpotlightLowerer {
    fn lower_dyn(&self, cx: &mut LoweringContext) -> WidgetId {
        let children = self
            .children
            .iter()
            .map(|child| fission_core::internal::lower_widget(child, cx))
            .collect::<Vec<_>>();
        let mut builder = IrBuilder::new(
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
