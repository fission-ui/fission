//! A field's helper or error message stays inside the field's width, wrapping
//! onto more lines, instead of running past its edge into whatever sits beside it.

use fission_core::ui::{Column, Container, TextInput, Widget};
use fission_core::GlobalState;
use fission_test::TestHarness;

#[derive(Debug, Default, Clone)]
struct State;

impl GlobalState for State {}

const FIELD_WIDTH: f32 = 180.0;
const MESSAGE: &str = "Use letters, numbers and spaces only, and keep it short";

#[derive(Clone)]
struct Page;

impl From<Page> for Widget {
    fn from(_: Page) -> Self {
        Column {
            children: vec![Container::new(TextInput {
                label: Some("Team name".into()),
                error_text: Some(MESSAGE.into()),
                ..Default::default()
            })
            .width(FIELD_WIDTH)
            .into()],
            align_items: fission_core::op::AlignItems::Start,
            ..Default::default()
        }
        .into()
    }
}

#[test]
fn a_long_message_wraps_within_the_field() {
    let mut harness = TestHarness::new(State).with_root_widget(Page);
    harness.pump().expect("first frame");
    let ir = harness.last_ir.as_ref().expect("ir");
    let snapshot = harness.last_snapshot.as_ref().expect("snapshot");

    let message = ir
        .nodes
        .iter()
        .find(|(_, node)| node.op.text().is_some_and(|text| text == MESSAGE))
        .map(|(id, _)| {
            snapshot
                .get_node_geometry(*id)
                .expect("message geometry")
                .rect
        })
        .expect("message text");

    assert!(
        message.width() <= FIELD_WIDTH + 0.5,
        "the message is {} wide in a {FIELD_WIDTH}px field",
        message.width()
    );
    let label_line = 22.0;
    assert!(
        message.height() > label_line,
        "a message longer than the field wraps onto more lines, got {message:?}"
    );
}
