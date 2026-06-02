use crate::env::{Env, RuntimeState};
use crate::internal::InternalLower;
use crate::lowering::InternalLoweringCx;
use crate::ui::widgets::container::Container;
use crate::ui::widgets::transform::Transform;
use fission_ir::{LayoutOp, Op};

#[test]
fn test_transform_lowering() {
    let env = Env::default();
    let runtime_state = RuntimeState::default();

    // Identity matrix
    let matrix = [
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ];

    let transform = Transform {
        transform: matrix,
        child: Container::default().into(),
        ..Default::default()
    };

    let mut cx = InternalLoweringCx::new(&env, &runtime_state, None, None);
    let root_id = transform.lower(&mut cx);

    let node = cx.ir.nodes.get(&root_id).unwrap();
    if let Op::Layout(LayoutOp::Transform { transform: m }) = &node.op {
        assert_eq!(*m, matrix);
    } else {
        panic!("Expected LayoutOp::Transform, got {:?}", node.op);
    }
}
