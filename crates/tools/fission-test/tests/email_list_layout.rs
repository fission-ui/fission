use fission_core::op::Color;
use fission_core::ui::{Column, Container, Row, Text};
use fission_core::{AppState, BuildCtx, View, Widget};
use fission_test::TestHarness;

#[derive(Debug, Default, Clone)]
struct State;
impl AppState for State {}

#[test]
fn test_email_list_row_layout() {
    // Reproduce EmailList item structure
    // Row
    //   -> Container (Avatar placeholder)
    //   -> Container (Text Column) -> Column -> Text, Text
    //   -> Text (Date)

    struct EmailRow;
    impl Widget<State> for EmailRow {
        fn build(
            &self,
            _ctx: &mut BuildCtx<State>,
            _view: &View<State>,
        ) -> impl fission_core::IntoWidget<State> {
            Container::new(
                Row::new()
                    .flex_grow(0.0)
                    .child(
                        Container::new(fission_core::ui::widgets::spacer::Spacer::default())
                            .width(40.0)
                            .height(40.0)
                            .bg(Color::BLUE),
                    )
                    .child(
                        Container::new(
                            Column::new()
                                .child(Text::new("Subject Line"))
                                .child(Text::new("Preview Text")),
                        )
                        .flex_grow(1.0),
                    )
                    .child(Text::new("10:00 AM")),
            )
            .width(300.0)
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
