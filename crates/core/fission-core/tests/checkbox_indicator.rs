//! A checked checkbox draws a check mark, not a filled square.

use fission_core::authoring::LoweringContext;
use fission_core::env::{Env, RuntimeState};
use fission_core::ui::{Checkbox, Widget};
use fission_ir::op::{Op, PaintOp};
use fission_ir::CoreIR;

fn lower(node: Widget) -> CoreIR {
    let env = Env::default();
    let runtime = RuntimeState::default();
    let mut cx = LoweringContext::new(&env, &runtime, None, None);
    let root = fission_core::internal::lower_widget(&node, &mut cx);
    cx.set_root(root);
    cx.into_ir()
}

#[test]
fn a_checked_checkbox_draws_a_stroked_check_mark() {
    let ir = lower(
        Checkbox {
            checked: true,
            ..Default::default()
        }
        .into(),
    );
    let check_marks = ir
        .nodes
        .values()
        .filter(|node| {
            matches!(
                &node.op,
                Op::Paint(PaintOp::DrawPath {
                    stroke: Some(_),
                    fill: None,
                    ..
                })
            )
        })
        .count();
    assert_eq!(check_marks, 1, "the indicator is a stroked check mark");
    let small_filled_squares = ir
        .nodes
        .values()
        .filter(|node| {
            matches!(
                &node.op,
                Op::Paint(PaintOp::DrawRect {
                    fill: Some(_),
                    stroke: None,
                    ..
                })
            )
        })
        .count();
    assert_eq!(
        small_filled_squares, 0,
        "no filled square is drawn inside the box"
    );
}
