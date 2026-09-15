//! On a phone-width window the sidebar folds into a page picker and the theme bar
//! stays inside the window; an embedded gallery leaves theming to its host.

use fission::layout::LayoutSize;
use fission_test::{TestDriver, TestHarness};
use widget_gallery::{embedded_state, GalleryApp, GalleryPage, GalleryState};

const PHONE_WIDTH: f32 = 390.0;

fn phone() -> LayoutSize {
    LayoutSize::new(PHONE_WIDTH, 800.0)
}

fn driver(state: GalleryState, viewport: LayoutSize) -> TestDriver<GalleryState> {
    let mut driver = TestDriver::new(TestHarness::new(state).with_root_widget(GalleryApp));
    driver.harness.env.viewport_size = viewport;
    driver.pump().expect("first frame");
    driver
}

fn on_page(page: GalleryPage) -> GalleryState {
    GalleryState {
        page,
        ..GalleryState::default()
    }
}

fn tap_centre(driver: &mut TestDriver<GalleryState>, identifier: &str) {
    let bounds = driver
        .find_semantics_identifier(identifier)
        .unwrap_or_else(|| panic!("{identifier} should be on the page"))
        .bounds;
    driver
        .tap_point(
            bounds.x() + bounds.width() / 2.0,
            bounds.y() + bounds.height() / 2.0,
        )
        .expect("tap");
    driver.pump().expect("pump after tap");
}

#[test]
fn a_phone_width_swaps_the_sidebar_for_a_page_picker() {
    let driver = driver(on_page(GalleryPage::Button), phone());

    assert!(
        driver.find_semantics_identifier("gallery.nav.button").is_none(),
        "the sidebar should not show at phone width"
    );
    let picker = driver
        .find_semantics_identifier("gallery.page_picker")
        .expect("a page picker should replace the sidebar")
        .bounds;
    assert!(
        picker.width() >= 300.0 && picker.x() + picker.width() <= PHONE_WIDTH + 0.5,
        "the picker should span the window without overflowing, got {picker:?}"
    );

    // With the sidebar gone the page text gets the window's width, not a sliver.
    let description = driver
        .find_text("Starts an action")
        .expect("the page description should show")
        .bounds;
    assert!(
        description.width() >= 250.0,
        "the description should wrap across the window, got {description:?}"
    );
}

#[test]
fn the_theme_bar_stays_inside_a_phone_width_window() {
    let driver = driver(on_page(GalleryPage::Button), phone());
    let bar = driver
        .find_semantics_identifier("gallery.theme_bar")
        .expect("the standalone gallery shows its theme bar")
        .bounds;
    assert!(
        bar.x() + bar.width() <= PHONE_WIDTH + 0.5,
        "the theme bar should fit the window, got {bar:?}"
    );
    for label in ["Spacious", "Dark", "Ember"] {
        let text = driver
            .find_text(label)
            .unwrap_or_else(|| panic!("{label} should be in the theme bar"))
            .bounds;
        assert!(
            text.x() + text.width() <= PHONE_WIDTH + 0.5,
            "{label} should wrap into view rather than be cut off, got {text:?}"
        );
    }
}

#[test]
fn the_page_picker_opens_the_chosen_page() {
    let mut driver = driver(on_page(GalleryPage::Button), phone());
    tap_centre(&mut driver, "gallery.page_picker");
    tap_centre(&mut driver, "gallery.picker.accordion");

    assert!(
        driver.find_text("Stacked sections that expand").is_some(),
        "choosing Accordion should open its page"
    );
    assert!(
        driver
            .find_semantics_identifier("gallery.picker.accordion")
            .is_none(),
        "the picker should close once a page is chosen"
    );
}

#[test]
fn a_wide_window_keeps_the_sidebar() {
    let driver = driver(
        on_page(GalleryPage::Button),
        LayoutSize::new(1280.0, 800.0),
    );
    assert!(driver.find_semantics_identifier("gallery.nav.button").is_some());
    assert!(driver
        .find_semantics_identifier("gallery.page_picker")
        .is_none());
}

#[test]
fn an_embedded_gallery_leaves_the_theme_to_its_host() {
    // The Button page, since Foundations shows density names in its own demos.
    let state = GalleryState {
        page: GalleryPage::Button,
        ..embedded_state()
    };
    let driver = driver(state, LayoutSize::new(1280.0, 800.0));
    assert!(
        driver.find_semantics_identifier("gallery.theme_bar").is_none(),
        "an embedded gallery should not show its own theme bar"
    );
    assert!(driver.find_text("Comfortable").is_none());
    assert!(driver.find_semantics_identifier("gallery.nav.button").is_some());
}
