use fission_core::op::Color;
use fission_core::ui::{Column, Container, Row, Text, Widget};
use fission_core::GlobalState;
use fission_test::TestHarness;

#[derive(Debug, Default, Clone)]
struct State;
impl GlobalState for State {}

#[test]
fn test_email_list_row_layout() {
    // Reproduce EmailList item structure
    // Row
    //   -> Container (Avatar placeholder)
    //   -> Container (Text Column) -> Column -> Text, Text
    //   -> Text (Date)

    #[derive(Clone)]
    struct EmailRow;
    impl From<EmailRow> for Widget {
        fn from(_component: EmailRow) -> Self {
            let (_ctx, _view) = fission_core::build::current::<State>();
            Container::new(
                Row::default()
                    .flex_grow(0.0) // Row itself
                    .children(vec![
                        // Avatar
                        Container::new(fission_core::ui::widgets::spacer::Spacer::default())
                            .width(40.0)
                            .height(40.0)
                            .bg(Color::BLUE)
                            .into(),
                        // Text Column
                        Container::new(Column::default().children(vec![
                            Text::new("Subject Line").into(),
                            Text::new("Preview Text").into(),
                        ]))
                        .flex_grow(1.0) // Grow to fill space
                        .into(),
                        // Date
                        Text::new("10:00 AM").into(),
                    ]),
            )
            .width(300.0)
            .into()
        }
    }
    let mut h = TestHarness::new(State);
    h = h.with_root_widget(EmailRow);
    h.pump().unwrap();

    let snap = h.last_snapshot.as_ref().unwrap();
    let ir = h.last_ir.as_ref().unwrap();

    // Find Text nodes
    let mut text_rects = Vec::new();
    for (id, node) in &ir.nodes {
        if let fission_ir::Op::Paint(fission_ir::PaintOp::DrawText { text, .. }) = &node.op {
            let geom = snap.get_node_geometry(*id).unwrap();
            text_rects.push((text.clone(), geom.rect));
        }
    }

    // Assert "Subject Line" and "Preview Text" are NOT overlapping
    let subject = text_rects
        .iter()
        .find(|(t, _)| t == "Subject Line")
        .unwrap();
    let preview = text_rects
        .iter()
        .find(|(t, _)| t == "Preview Text")
        .unwrap();

    // Vertical stacking means Y must differ by at least height
    let y_diff = (subject.1.y() - preview.1.y()).abs();

    assert!(
        y_diff >= subject.1.height(),
        "Subject and Preview overlap vertically!"
    );
}
