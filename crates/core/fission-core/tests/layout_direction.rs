use fission_core::authoring::LoweringContext;
use fission_core::env::{Env, LayoutDirection, RuntimeState};
use fission_core::internal::build_layout_tree;
use fission_core::ui::{Container, Positioned, Row, SemanticsRegion, Widget, ZStack};
use fission_core::{Role, WidgetId};
use fission_layout::{LayoutEngine, LayoutSize};

fn lower_and_layout(
    env: &Env,
    widget: Widget,
) -> (fission_ir::CoreIR, fission_layout::LayoutSnapshot) {
    let runtime = RuntimeState::default();
    let mut lowering = LoweringContext::new(env, &runtime, None, None);
    let root = fission_core::internal::lower_widget(&widget, &mut lowering);
    lowering.ir.set_root(root);

    let input = build_layout_tree(&lowering.ir, env);
    let mut engine = LayoutEngine::new().with_layout_direction(lowering.ir.layout_direction);
    let snapshot = engine
        .compute_layout(&input, root, LayoutSize::new(100.0, 40.0), &|_| 0.0)
        .expect("directional row layout");
    (lowering.ir, snapshot)
}

fn focusable_cell(id: WidgetId) -> SemanticsRegion {
    SemanticsRegion {
        id: Some(id),
        role: Role::Button,
        focusable: Some(true),
        child: Some(Container::default().width(20.0).height(20.0).into()),
        ..Default::default()
    }
}

#[test]
fn row_uses_logical_source_order_for_layout_and_focus() {
    let first = WidgetId::explicit("direction.first");
    let second = WidgetId::explicit("direction.second");
    let make_row = || Row {
        children: vec![focusable_cell(first).into(), focusable_cell(second).into()],
        ..Default::default()
    };

    let (ltr_ir, ltr_layout) = lower_and_layout(&Env::default(), make_row().into());
    assert_eq!(ltr_ir.layout_direction, LayoutDirection::LeftToRight);
    assert_eq!(
        fission_core::hit_test::get_all_focusable_nodes(&ltr_ir),
        vec![first, second]
    );
    assert_eq!(ltr_layout.get_node_rect(first).unwrap().origin.x, 0.0);
    assert_eq!(ltr_layout.get_node_rect(second).unwrap().origin.x, 20.0);

    let mut rtl_env = Env::default();
    rtl_env.layout_direction = LayoutDirection::RightToLeft;
    let (rtl_ir, rtl_layout) = lower_and_layout(&rtl_env, make_row().into());
    assert_eq!(rtl_ir.layout_direction, LayoutDirection::RightToLeft);
    assert_eq!(
        fission_core::hit_test::get_all_focusable_nodes(&rtl_ir),
        vec![first, second],
        "sequential focus follows logical source order, not physical x order"
    );
    assert_eq!(rtl_layout.get_node_rect(first).unwrap().origin.x, 80.0);
    assert_eq!(rtl_layout.get_node_rect(second).unwrap().origin.x, 60.0);
}

#[test]
fn positioned_left_and_right_offsets_remain_physical_in_rtl() {
    let left_child = WidgetId::explicit("direction.physical-left");
    let right_child = WidgetId::explicit("direction.physical-right");
    let mut env = Env::default();
    env.layout_direction = LayoutDirection::RightToLeft;
    let root = ZStack {
        children: vec![
            Positioned {
                left: Some(10.0),
                top: Some(0.0),
                child: Some(
                    Container {
                        id: Some(left_child),
                        width: Some(20.0),
                        height: Some(20.0),
                        ..Default::default()
                    }
                    .into(),
                ),
                ..Default::default()
            }
            .into(),
            Positioned {
                right: Some(10.0),
                top: Some(0.0),
                child: Some(
                    Container {
                        id: Some(right_child),
                        width: Some(20.0),
                        height: Some(20.0),
                        ..Default::default()
                    }
                    .into(),
                ),
                ..Default::default()
            }
            .into(),
        ],
        ..Default::default()
    };

    let (_, layout) = lower_and_layout(&env, root.into());
    assert_eq!(layout.get_node_rect(left_child).unwrap().origin.x, 10.0);
    assert_eq!(layout.get_node_rect(right_child).unwrap().origin.x, 70.0);
}

#[test]
fn directional_padding_mirrors_its_inline_edges() {
    // A leading inset of 24 and a trailing inset of 4, written once.
    let child_id = WidgetId::explicit("directional.padding.child");
    let widget = || -> Widget {
        Container::new(Widget::from(Container {
            id: Some(child_id),
            ..Container::default().width(10.0).height(10.0)
        }))
        .padding_directional([24.0, 4.0, 0.0, 0.0])
        .width(100.0)
        .height(40.0)
        .into()
    };

    let mut ltr = Env::default();
    ltr.layout_direction = LayoutDirection::LeftToRight;
    let (_, ltr_snapshot) = lower_and_layout(&ltr, widget());
    let ltr_x = ltr_snapshot
        .get_node_rect(child_id)
        .expect("child rect in a left-to-right layout")
        .origin
        .x;

    let mut rtl = Env::default();
    rtl.layout_direction = LayoutDirection::RightToLeft;
    let (_, rtl_snapshot) = lower_and_layout(&rtl, widget());
    let rtl_x = rtl_snapshot
        .get_node_rect(child_id)
        .expect("child rect in a right-to-left layout")
        .origin
        .x;

    assert_eq!(
        ltr_x, 24.0,
        "start padding should sit on the left edge in a left-to-right layout"
    );
    assert_eq!(
        rtl_x, 4.0,
        "start padding should sit on the right edge in a right-to-left layout, \
         leaving the trailing inset on the left"
    );
}

#[test]
fn physical_padding_does_not_mirror() {
    // The physical escape hatch stays physical, so a caller that genuinely
    // means "left" keeps it under either reading order.
    let child_id = WidgetId::explicit("physical.padding.child");
    let widget = || -> Widget {
        Container::new(Widget::from(Container {
            id: Some(child_id),
            ..Container::default().width(10.0).height(10.0)
        }))
        .padding([24.0, 4.0, 0.0, 0.0])
        .width(100.0)
        .height(40.0)
        .into()
    };

    let mut rtl = Env::default();
    rtl.layout_direction = LayoutDirection::RightToLeft;
    let (_, snapshot) = lower_and_layout(&rtl, widget());
    assert_eq!(
        snapshot
            .get_node_rect(child_id)
            .expect("child rect")
            .origin
            .x,
        24.0,
        "physical padding should keep its left inset under right-to-left"
    );
}
