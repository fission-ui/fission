//! The gallery's dropdown and menu triggers size to their content instead of filling the row.

use fission::core::Role;
use fission::layout::LayoutSize;
use fission_test::{TestDriver, TestHarness};
use widget_gallery::{GalleryApp, GalleryPage, GalleryState};

/// Opens the gallery on `page`, which holds the triggers under test.
fn driver(page: GalleryPage) -> TestDriver<GalleryState> {
    let state = GalleryState {
        page,
        ..GalleryState::default()
    };
    let mut driver = TestDriver::new(TestHarness::new(state).with_root_widget(GalleryApp));
    driver.harness.env.viewport_size = LayoutSize::new(1000.0, 3000.0);
    driver.pump().expect("first frame");
    driver
}

#[test]
fn menu_trigger_hugs_its_label() {
    let driver = driver(GalleryPage::Menu);
    let trigger = driver
        .find_semantics_identifier("gallery.menu.trigger")
        .expect("menu trigger")
        .bounds;
    assert!(
        trigger.width() < 240.0,
        "the Actions menu trigger should hug its label, got {trigger:?}"
    );
}

#[test]
fn select_trigger_hugs_its_value() {
    let driver = driver(GalleryPage::Select);
    let trigger = driver
        .find_semantics_identifier("gallery.select.trigger")
        .expect("select trigger")
        .bounds;
    assert!(
        trigger.width() < 320.0,
        "the select trigger should hug its value, got {trigger:?}"
    );
}

#[test]
fn dropdown_triggers_hug_their_values() {
    let mut wide = Vec::new();
    for page in [GalleryPage::Menu, GalleryPage::Select] {
        let driver = driver(page);
        wide.extend(
            driver
                .find_role(Role::ComboBox)
                .into_iter()
                .filter(|found| found.bounds.width() >= 600.0)
                .map(|found| (found.label.clone(), found.bounds)),
        );
    }
    assert!(
        wide.is_empty(),
        "dropdown triggers should hug their values, these fill the row: {wide:?}"
    );
}
