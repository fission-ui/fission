use fission_core::internal::{lower_widget, InternalLoweringCx};
use fission_core::{Env, RuntimeState};
use fission_ir::{LayoutOp, Op, WidgetId};
use fission_widgets::{Spotlight, Text, Widget};

#[test]
fn spotlight_lowers_five_ordered_regions() {
    let anchor = WidgetId::explicit("tour.anchor");
    let regions: [Widget; 5] =
        std::array::from_fn(|index| Text::new(format!("region-{index}")).into());
    let spotlight: Widget = Spotlight {
        anchor,
        padding: 12.0,
        children: regions,
    }
    .into();

    let env = Env::default();
    let runtime_state = RuntimeState::default();
    let mut cx = InternalLoweringCx::new(&env, &runtime_state, None, None);
    let _root = lower_widget(&spotlight, &mut cx);
    let node = cx
        .ir
        .nodes
        .values()
        .find(|node| matches!(node.op, Op::Layout(LayoutOp::Spotlight { .. })))
        .expect("spotlight layout node");

    assert_eq!(
        node.op,
        Op::Layout(LayoutOp::Spotlight {
            anchor,
            padding: 12.0,
        })
    );
    assert_eq!(node.children.len(), 5);
}
