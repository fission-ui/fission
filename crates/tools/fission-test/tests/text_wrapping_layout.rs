use fission_core::ui::{Column, Container, Node, Text};
use fission_core::{AppState, BuildCtx, View, Widget};
use fission_layout::LayoutRect;
use fission_test::TestHarness;

#[derive(Debug, Default, Clone)]
struct State;
impl AppState for State {}

fn text_rect(h: &TestHarness<State>, needle: &str) -> LayoutRect {
    let snap = h.last_snapshot.as_ref().unwrap();
    let ir = h.last_ir.as_ref().unwrap();
    for (id, node) in &ir.nodes {
        if let fission_ir::Op::Paint(fission_ir::PaintOp::DrawText { text, .. }) = &node.op {
            if text == needle {
                return snap.get_node_geometry(*id).unwrap().rect;
            }
        }
    }
    panic!("text '{}' not found", needle);
}

#[test]
fn text_wrap_pushes_siblings() {
    struct WrapDemo;
    impl Widget<State> for WrapDemo {
        fn build(&self, _ctx: &mut BuildCtx<State>, _view: &View<State>) -> Node {
            let subject = "Design review: Inbox refresh";
            Container::new(
                Column::default()
                    .gap(Some(6.0))
                    .children(vec![
                        Text::new(subject).size(16.0).into_node(),
                        Text::new("Preview line").size(12.0).into_node(),
                    ])
                    .into_node(),
            )
            .width(160.0)
            .into_node()
        }
    }

    let mut h = TestHarness::new(State::default());
    h = h.with_root_widget(WrapDemo);
    h.pump().unwrap();

    let subject = "Design review: Inbox refresh";
    let subject_rect = text_rect(&h, subject);
    let preview_rect = text_rect(&h, "Preview line");

    let (_, single_h) = h.measurer.measure("A", 16.0, None);
    let (_, wrapped_h) = h.measurer.measure(subject, 16.0, Some(160.0));

    assert!(
        wrapped_h > single_h + 1.0,
        "expected subject to wrap at 160px"
    );
    assert!(
        subject_rect.height() >= wrapped_h - 1.0,
        "subject height should reflect wrapping"
    );
    assert!(
        preview_rect.y() >= subject_rect.y() + subject_rect.height() + 5.0,
        "preview should be placed below wrapped subject"
    );
}

#[test]
fn menu_item_text_stays_single_line() {
    use fission_widgets::{Menu, MenuItem};

    struct MenuDemo;
    impl Widget<State> for MenuDemo {
        fn build(&self, ctx: &mut BuildCtx<State>, view: &View<State>) -> Node {
            Menu {
                items: vec![MenuItem {
                    label: "New event".into(),
                    icon: None,
                    on_select: None,
                }],
                width: Some(220.0),
                max_height: Some(200.0),
            }
            .build(ctx, view)
        }
    }

    let mut h = TestHarness::new(State::default());
    h = h.with_root_widget(MenuDemo);
    h.pump().unwrap();

    let rect = text_rect(&h, "New event");
    let (_, line_h) = h.measurer.measure("New event", 14.0, Some(220.0));
    assert!(
        rect.height() <= line_h * 1.3,
        "menu item text should remain single-line"
    );
}
