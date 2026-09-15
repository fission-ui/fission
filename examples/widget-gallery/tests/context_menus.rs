//! Right-clicking the gallery's custom region or its selectable text opens a visible menu.

use fission::core::event::{InputEvent, KeyCode, KeyEvent, PointerButton, PointerEvent};
use fission::layout::{LayoutPoint, LayoutSize};
use fission_test::{TestDriver, TestHarness};
use widget_gallery::{GalleryApp, GalleryState};

fn right_click(driver: &mut TestDriver<GalleryState>, point: LayoutPoint) {
    driver
        .harness
        .send_event(InputEvent::Pointer(PointerEvent::Down {
            pointer_id: Default::default(),
            kind: Default::default(),
            point,
            button: PointerButton::Secondary,
            modifiers: 0,
        }))
        .expect("pointer down");
    driver
        .harness
        .send_event(InputEvent::Pointer(PointerEvent::Up {
            pointer_id: Default::default(),
            kind: Default::default(),
            point,
            button: PointerButton::Secondary,
            modifiers: 0,
        }))
        .expect("pointer up");
    driver.pump().expect("pump after right-click");
    driver.pump().expect("second pump after right-click");
}

fn driver() -> TestDriver<GalleryState> {
    let mut driver =
        TestDriver::new(TestHarness::new(GalleryState::default()).with_root_widget(GalleryApp));
    driver.harness.env.viewport_size = LayoutSize::new(1000.0, 3000.0);
    driver.pump().expect("first frame");
    driver
}

#[test]
fn right_clicking_the_custom_region_opens_its_menu() {
    let mut driver = driver();
    let region = driver
        .find_text("Right-click this custom region for a widget-backed context menu.")
        .expect("custom region text")
        .bounds;
    right_click(
        &mut driver,
        LayoutPoint::new(region.x() + 20.0, region.y() + region.height() / 2.0),
    );
    assert!(
        driver
            .find_text("Menu items can be arbitrary widgets")
            .is_some(),
        "the custom region's menu should be visible after a right-click"
    );
}

#[test]
fn right_clicking_selectable_text_opens_its_menu() {
    let mut driver = driver();
    let text = driver
        .find_text("Selectable Text: drag across this sentence, then use Ctrl/Cmd+C or right-click for Copy and Select All.")
        .expect("selectable text")
        .bounds;
    right_click(
        &mut driver,
        LayoutPoint::new(text.x() + 40.0, text.y() + text.height() / 2.0),
    );
    assert!(
        driver
            .harness
            .runtime
            .runtime_state
            .context_menu
            .owner
            .is_some(),
        "the selectable text's menu should stay open after a right-click"
    );
}

#[test]
fn right_clicking_a_text_field_opens_a_menu_whose_copy_runs_and_closes() {
    use fission::core::event::{KeyCode, MOD_CTRL};
    use fission::core::Role;

    let state = GalleryState {
        text_value: "copy me".into(),
        ..GalleryState::default()
    };
    let mut driver = TestDriver::new(TestHarness::new(state).with_root_widget(GalleryApp));
    driver.harness.env.viewport_size = LayoutSize::new(1000.0, 3000.0);
    driver.pump().expect("first frame");

    let field = driver
        .find_semantics_identifier("gallery.text_input")
        .expect("gallery text input");
    let centre = LayoutPoint::new(
        field.bounds.x() + 20.0,
        field.bounds.y() + field.bounds.height() / 2.0,
    );
    driver
        .tap_point(centre.x, centre.y)
        .expect("focus the field");
    driver
        .press_key(KeyCode::Char('a'), MOD_CTRL)
        .expect("select all");
    right_click(&mut driver, centre);

    let owner = driver.harness.runtime.runtime_state.context_menu.owner;
    assert_eq!(
        owner,
        Some(field.node_id),
        "a right-click in the text field should open its context menu"
    );
    let copy = driver
        .find_role(Role::Button)
        .into_iter()
        .find(|found| found.label.as_deref() == Some("Copy"))
        .expect("Copy item in the field's menu")
        .bounds;
    let point = LayoutPoint::new(
        copy.x() + copy.width() / 2.0,
        copy.y() + copy.height() / 2.0,
    );
    for event in [
        InputEvent::Pointer(PointerEvent::Down {
            pointer_id: Default::default(),
            kind: Default::default(),
            point,
            button: PointerButton::Primary,
            modifiers: 0,
        }),
        InputEvent::Pointer(PointerEvent::Up {
            pointer_id: Default::default(),
            kind: Default::default(),
            point,
            button: PointerButton::Primary,
            modifiers: 0,
        }),
    ] {
        driver.harness.send_event(event).expect("click Copy");
    }
    driver.pump().expect("pump after Copy");
    assert!(
        driver
            .harness
            .runtime
            .runtime_state
            .context_menu
            .owner
            .is_none(),
        "choosing Copy should close the text field's menu"
    );
}

#[test]
fn escape_closes_an_open_text_menu() {
    let mut driver = driver();
    let text = driver
        .find_text("drag across this sentence")
        .expect("selectable text")
        .bounds;
    right_click(
        &mut driver,
        LayoutPoint::new(text.x() + 20.0, text.y() + text.height() / 2.0),
    );
    assert!(
        driver
            .harness
            .runtime
            .runtime_state
            .context_menu
            .owner
            .is_some(),
        "right-clicking selectable text should open its menu"
    );
    driver
        .harness
        .send_event(InputEvent::Keyboard(KeyEvent::Down {
            key_code: KeyCode::Escape,
            modifiers: 0,
        }))
        .expect("escape");
    driver.pump().expect("pump after escape");
    assert!(
        driver
            .harness
            .runtime
            .runtime_state
            .context_menu
            .owner
            .is_none(),
        "Escape should close the menu"
    );
}
