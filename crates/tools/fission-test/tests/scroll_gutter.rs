//! Content in a scroll that shows a scrollbar stays clear of the bar.

use fission_core::scrollbar::{SCROLLBAR_INSET, SCROLLBAR_THICKNESS};
use fission_core::ui::{Container, Scroll, Text, Widget};
use fission_core::{GlobalState, WidgetId};
use fission_test::{TestDriver, TestHarness};

#[derive(Debug, Default, Clone)]
struct State;

impl GlobalState for State {}

const LONG: &str = "A long line of text that wraps across several lines inside the scroll so that \
it runs up against the end edge where the scrollbar is drawn, and keeps going long enough to \
overflow the viewport and make the scrollbar appear.";

#[derive(Clone)]
struct Page;

impl From<Page> for Widget {
    fn from(_: Page) -> Self {
        Container::new(Scroll {
            id: Some(WidgetId::explicit("scroll")),
            child: Some(Text::new(LONG).into()),
            height: Some(60.0),
            width: Some(200.0),
            show_scrollbar: true,
            ..Default::default()
        })
        .into()
    }
}

#[test]
fn content_ends_before_the_scrollbar() {
    let mut driver = TestDriver::new(TestHarness::new(State).with_root_widget(Page));
    driver.pump().expect("first frame");
    let ir = driver.harness.last_ir.as_ref().expect("ir");
    let snapshot = driver.harness.last_snapshot.as_ref().expect("snapshot");
    let text = ir
        .nodes
        .iter()
        .find(|(_, node)| {
            node.op
                .text()
                .is_some_and(|text| text.starts_with("A long line"))
        })
        .and_then(|(id, _)| snapshot.get_node_rect(*id))
        .expect("text is laid out");
    let scroll = snapshot
        .get_node_rect(WidgetId::explicit("scroll"))
        .expect("scroll is laid out");

    let strip_start = scroll.x() + scroll.width() - (SCROLLBAR_THICKNESS + SCROLLBAR_INSET * 2.0);
    let text_right = text.x() + text.width();
    assert!(
        text_right <= strip_start + 0.5,
        "text ends at {text_right}, but the scrollbar strip starts at {strip_start}"
    );
}
