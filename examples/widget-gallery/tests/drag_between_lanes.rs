//! Dragging a gallery task card onto another lane moves it there, even though each lane swaps to
//! its active and hover panels while the drag runs.

use fission::core::event::{InputEvent, PointerButton, PointerEvent};
use fission::layout::{LayoutPoint, LayoutSize};
use fission_test::{TestDriver, TestHarness};
use widget_gallery::{GalleryApp, GalleryPage, GalleryState};

fn pointer(driver: &mut TestDriver<GalleryState>, phase: &str, point: LayoutPoint) {
    let event = match phase {
        "down" => PointerEvent::Down {
            pointer_id: Default::default(),
            kind: Default::default(),
            point,
            button: PointerButton::Primary,
            modifiers: 0,
        },
        "up" => PointerEvent::Up {
            pointer_id: Default::default(),
            kind: Default::default(),
            point,
            button: PointerButton::Primary,
            modifiers: 0,
        },
        _ => PointerEvent::Move {
            pointer_id: Default::default(),
            kind: Default::default(),
            point,
            modifiers: 0,
        },
    };
    driver
        .harness
        .send_event(InputEvent::Pointer(event))
        .expect("pointer event");
    driver.pump().expect("pump after pointer event");
}

#[test]
fn a_card_dragged_onto_the_done_lane_moves_there() {
    let mut driver = TestDriver::new(
        TestHarness::new(GalleryState {
            page: GalleryPage::DragAndDrop,
            ..GalleryState::default()
        })
        .with_root_widget(GalleryApp),
    );
    driver.harness.env.viewport_size = LayoutSize::new(1000.0, 8000.0);
    driver.pump().expect("first frame");

    let card = driver
        .find_semantics_identifier("gallery.drag.card.backlog.Inbox triage")
        .expect("backlog card")
        .bounds;
    let lane = driver
        .find_semantics_identifier("gallery.drag.zone.Done")
        .expect("done lane")
        .bounds;
    let start = LayoutPoint::new(card.x() + 12.0, card.y() + card.height() / 2.0);
    let end = LayoutPoint::new(
        lane.x() + lane.width() / 2.0,
        lane.y() + lane.height() / 2.0,
    );

    pointer(&mut driver, "down", start);
    for step in 1..=12 {
        let t = step as f32 / 12.0;
        pointer(
            &mut driver,
            "move",
            LayoutPoint::new(
                start.x + (end.x - start.x) * t,
                start.y + (end.y - start.y) * t,
            ),
        );
    }
    assert!(
        driver
            .harness
            .runtime
            .runtime_state
            .gesture
            .drag_session
            .is_some(),
        "the drag should still be running over the Done lane"
    );
    pointer(&mut driver, "up", end);

    assert!(
        driver
            .find_semantics_identifier("gallery.drag.card.done.Inbox triage")
            .is_some(),
        "the card should now be in the Done lane"
    );
    assert!(
        driver
            .find_semantics_identifier("gallery.drag.card.backlog.Inbox triage")
            .is_none(),
        "the card should have left the backlog"
    );
}
