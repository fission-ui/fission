//! Each newer component page opens on its own and shows its interactive controls.

use fission::layout::LayoutSize;
use fission_test::{TestDriver, TestHarness};
use widget_gallery::{GalleryApp, GalleryPage, GalleryState};

/// Opens the gallery on `page` alone.
fn driver(page: GalleryPage) -> TestDriver<GalleryState> {
    let state = GalleryState {
        page,
        ..GalleryState::default()
    };
    let mut driver = TestDriver::new(TestHarness::new(state).with_root_widget(GalleryApp));
    driver.harness.env.viewport_size = LayoutSize::new(1200.0, 2000.0);
    driver.pump().expect("first frame");
    driver
}

#[test]
fn component_pages_render_their_controls() {
    let cases: &[(GalleryPage, &[&str])] = &[
        (GalleryPage::AspectRatio, &[]),
        (GalleryPage::Calendar, &[]),
        (GalleryPage::Combobox, &["gallery.combobox.input"]),
        (GalleryPage::DataTable, &[]),
        (GalleryPage::DatePicker, &[]),
        (GalleryPage::DateRangePicker, &[]),
        (GalleryPage::Divider, &[]),
        (GalleryPage::Dropdown, &[]),
        (GalleryPage::Dropzone, &["gallery.dropzone.target"]),
        (GalleryPage::Editable, &[]),
        (GalleryPage::Field, &["gallery.field.input"]),
        (GalleryPage::FileUpload, &["gallery.file_upload.browse"]),
        (GalleryPage::Image, &[]),
        (GalleryPage::Markdown, &[]),
        (GalleryPage::Popover, &["gallery.popover.trigger"]),
        (
            GalleryPage::Radio,
            &["gallery.radio.standard", "gallery.radio.drone"],
        ),
        (GalleryPage::RangeSlider, &["gallery.range_slider"]),
        (
            GalleryPage::RefreshIndicator,
            &["gallery.refresh_indicator.action"],
        ),
        (GalleryPage::SplitView, &["gallery.split_view.half"]),
        (GalleryPage::TimePicker, &[]),
    ];
    for (page, identifiers) in cases.iter() {
        let driver = driver(*page);
        for identifier in *identifiers {
            let node = driver
                .find_semantics_identifier(identifier)
                .unwrap_or_else(|| panic!("{page:?} should show {identifier}"));
            assert!(
                node.bounds.width() > 0.0 && node.bounds.height() > 0.0,
                "{identifier} on {page:?} should have a visible size, got {:?}",
                node.bounds
            );
        }
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
fn the_share_button_opens_its_popover() {
    let mut driver = driver(GalleryPage::Popover);
    driver.assert_text_not_visible("Share this page");
    tap_centre(&mut driver, "gallery.popover.trigger");
    driver.assert_text_visible("Share this page");
}

#[test]
fn the_dropdown_opens_its_list_and_shows_the_choice() {
    let mut driver = driver(GalleryPage::Dropdown);
    driver.tap_text("Select an option").expect("open dropdown");
    driver.pump().expect("pump");
    tap_centre(&mut driver, "gallery.dropdown.asia_pacific");
    driver.assert_text_visible("Asia Pacific");
    assert!(
        driver
            .find_semantics_identifier("gallery.dropdown.europe")
            .is_none(),
        "choosing an option should close the list"
    );
}

#[test]
fn typing_in_the_combobox_narrows_its_options() {
    let mut driver = driver(GalleryPage::Combobox);
    tap_centre(&mut driver, "gallery.combobox.input");
    driver.type_text("ap").expect("type");
    driver.pump().expect("pump");
    driver.assert_text_visible("Grape");
    driver.assert_text_not_visible("Banana");
}

#[test]
fn pressing_the_editable_name_offers_save() {
    let mut driver = driver(GalleryPage::Editable);
    assert!(driver
        .find_semantics_identifier("gallery.editable.save")
        .is_none());
    driver.tap_text("Quarterly report").expect("start editing");
    driver.pump().expect("pump");
    assert!(driver
        .find_semantics_identifier("gallery.editable.save")
        .is_some());
}

#[test]
fn the_calendar_keeps_to_its_own_width() {
    let driver = driver(GalleryPage::Calendar);
    let calendar = driver
        .find_role(fission::core::Role::Table)
        .into_iter()
        .find(|node| node.label.as_deref() == Some("Calendar"))
        .expect("calendar table");
    assert!(
        calendar.bounds.width() > 200.0 && calendar.bounds.width() < 500.0,
        "the calendar should hug its seven day columns, got {:?}",
        calendar.bounds
    );
}
