use fission_core::ui::{Column, Container, Row, Text, Widget};
use fission_core::GlobalState;
use fission_test::TestHarness;

#[derive(Debug, Default, Clone)]
struct State;
impl GlobalState for State {}

#[test]
fn text_wrap_increases_layout_height() {
    // Regression: text could render wrapped (paint) but be measured as 1-line (layout),
    // causing it to overlap the next line.
    //
    // We assert that when the text is width-constrained below its full width, its
    // layout height grows beyond a single line.

    #[derive(Clone)]
    struct Root;
    impl From<Root> for Widget {
        fn from(_component: Root) -> Self {
            let (_ctx, _view) = fission_core::build::current::<State>();
            let long = "This is a very long subject line that should wrap into multiple lines";
            Container::new(Row::default().children(vec![
                        Container::new(fission_core::ui::widgets::spacer::Spacer::default())
                            .width(40.0)
                            .height(40.0)
                            .into(),
                        Container::new(
                            Column::default()
                                .children(vec![
                                    Text::new(long).max_width(120.0).into(),
                                    Text::new("Preview").into(),
                                ])
                        )
                        .flex_grow(1.0)
                        .into(),
                        Text::new("10:00 AM").into(),
                    ]))
            .width(160.0)
            .into()
        }
    }
    let mut h = TestHarness::new(State).with_root_widget(Root);
    h.pump().unwrap();

    let snap = h.last_snapshot.as_ref().unwrap();
    let ir = h.last_ir.as_ref().unwrap();

    let mut subject_rect = None;
    for (id, node) in &ir.nodes {
        if let fission_ir::Op::Paint(fission_ir::PaintOp::DrawText { text, .. }) = &node.op {
            if text.starts_with("This is a very long subject") {
                subject_rect = Some(snap.get_node_rect(*id).unwrap());
                break;
            }
        }
    }
    let subject_rect = subject_rect.expect("subject text node not found");

    let font_size = h.env.theme.tokens.typography.body_medium_size;
    let long = "This is a very long subject line that should wrap into multiple lines";
    let (full_w, single_line_h) = h.measurer.measure(long, font_size, None);
    let _ = full_w;
    assert!(
        subject_rect.height() > single_line_h,
        "expected wrapped height > single line height {}, got {}",
        single_line_h,
        subject_rect.height()
    );
}
