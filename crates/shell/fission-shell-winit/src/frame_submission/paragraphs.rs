use fission_ir::{CoreIR, Op};
use fission_layout::ParagraphResultStore;
use fission_render::paragraph::ParagraphFrameBindings;

pub(super) fn bindings(
    ir: &CoreIR,
    store: Option<&ParagraphResultStore>,
) -> ParagraphFrameBindings {
    let Some(store) = store else {
        return ParagraphFrameBindings::new();
    };
    let mut bindings = ParagraphFrameBindings::new();
    for (node_id, node) in &ir.nodes {
        if !matches!(
            &node.op,
            Op::Paint(fission_ir::PaintOp::DrawText { .. })
                | Op::Paint(fission_ir::PaintOp::DrawRichText { .. })
        ) {
            continue;
        }
        if let Some(result) = store.get(*node_id) {
            bindings.insert(*node_id, result);
        }
    }
    bindings
}
