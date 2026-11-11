use fission::op::StructuralOp;
use fission::prelude::*;

#[derive(Debug)]
struct CompactRow;

impl LowerWidget for CompactRow {
    fn lower_dyn(&self, cx: &mut LoweringContext) -> WidgetId {
        let id = cx.next_node_id();
        IrBuilder::new(id, Op::Structural(StructuralOp::Group { stable_hash: 1 })).build(cx)
    }
}

#[test]
fn custom_widget_authoring_is_available_from_the_prelude() {
    let _widget: Widget = custom_widget("CompactRow", CompactRow);
}
