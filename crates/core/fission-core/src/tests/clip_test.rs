use crate::env::{Env, RuntimeState};
use crate::lowering::{LoweringContext, build_layout_tree};
use crate::ui::traits::Lower;
use crate::ui::Node;
use crate::ui::widgets::container::Container;
use crate::ui::widgets::clip::Clip;
use fission_ir::{CoreIR, NodeId, Op, LayoutOp};
use fission_layout::{LayoutEngine, LayoutSize};
use std::sync::Arc;

#[test]
fn test_clip_lowering() {
    let env = Env::default();
    let runtime_state = RuntimeState::default();
    
    let clip = Clip {
        path: Some("M 0 0 L 100 0 L 100 100 L 0 100 Z".into()),
        child: Box::new(Container::default().into_node()),
        ..Default::default()
    };

    let mut cx = LoweringContext::new(&env, &runtime_state, None, None);
    let root_id = clip.lower(&mut cx);
    
    let node = cx.ir.nodes.get(&root_id).unwrap();
    if let Op::Layout(LayoutOp::Clip { path }) = &node.op {
        assert_eq!(path.as_deref(), Some("M 0 0 L 100 0 L 100 100 L 0 100 Z"));
    } else {
        panic!("Expected LayoutOp::Clip, got {:?}", node.op);
    }
}
