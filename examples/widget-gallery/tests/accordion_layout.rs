//! The gallery accordion's headers span the accordion and show a legible chevron.

use fission::core::Role;
use fission::layout::LayoutSize;
use fission_test::{TestDriver, TestHarness};
use widget_gallery::{GalleryApp, GalleryPage, GalleryState};

#[test]
fn accordion_headers_span_the_accordion_with_a_legible_chevron() {
    let mut driver = TestDriver::new(
        TestHarness::new(GalleryState {
            page: GalleryPage::Accordion,
            ..GalleryState::default()
        })
        .with_root_widget(GalleryApp),
    );
    driver.harness.env.viewport_size = LayoutSize::new(1000.0, 3000.0);
    driver.pump().expect("first frame");

    let header = [Role::Generic, Role::Button]
        .into_iter()
        .flat_map(|role| driver.find_role(role))
        .find(|found| found.label.as_deref() == Some("Section 1"))
        .expect("Section 1 header")
        .bounds;
    assert!(
        header.width() >= 600.0,
        "an accordion header should span the section width, got {header:?}"
    );

    let ir = driver.harness.last_ir.as_ref().expect("ir");
    let snapshot = driver.harness.last_snapshot.as_ref().expect("snapshot");
    let smallest_icon = ir
        .nodes
        .iter()
        .filter(|(_, node)| format!("{:?}", node.op).starts_with("Paint(DrawSvg"))
        .filter_map(|(id, _)| snapshot.get_node_rect(*id))
        .filter(|rect| {
            rect.y() >= header.y() - 1.0
                && rect.y() + rect.height() <= header.y() + header.height() + 1.0
        })
        .map(|rect| rect.width().min(rect.height()))
        .fold(f32::INFINITY, f32::min);
    let widest_bordered_surface = ir
        .nodes
        .iter()
        .filter(|(_, node)| {
            let op = format!("{:?}", node.op);
            op.starts_with("Paint(DrawRect") && op.contains("stroke: Some")
        })
        .filter_map(|(id, _)| snapshot.get_node_rect(*id))
        .filter(|rect| rect.y() >= header.y() - 1.0 && rect.y() <= header.y() + 1.0)
        .map(|rect| rect.width())
        .fold(0.0f32, f32::max);
    assert!(
        widest_bordered_surface >= header.width() - 2.0,
        "the bordered header surface should span the header, got {widest_bordered_surface} of {}",
        header.width()
    );
    assert!(
        smallest_icon >= 16.0,
        "the accordion chevron should be at least 16px, got {smallest_icon}"
    );
}

#[test]
fn an_expanded_accordion_panel_spans_the_accordion() {
    let mut driver = TestDriver::new(
        TestHarness::new(GalleryState {
            page: GalleryPage::Accordion,
            accordion_open: 1,
            ..GalleryState::default()
        })
        .with_root_widget(GalleryApp),
    );
    driver.harness.env.viewport_size = LayoutSize::new(1000.0, 3000.0);
    driver.pump().expect("first frame");

    let content = driver
        .find_text("Content of section 2")
        .expect("expanded panel content")
        .bounds;
    let ir = driver.harness.last_ir.as_ref().expect("ir");
    let snapshot = driver.harness.last_snapshot.as_ref().expect("snapshot");
    let panel_width = ir
        .nodes
        .iter()
        .filter(|(_, node)| {
            let op = format!("{:?}", node.op);
            op.starts_with("Paint(DrawRect") && op.contains("stroke: Some")
        })
        .filter_map(|(id, _)| snapshot.get_node_rect(*id))
        .filter(|rect| {
            rect.y() <= content.y()
                && rect.y() + rect.height() >= content.y() + content.height()
                && rect.height() < 200.0
        })
        .map(|rect| rect.width())
        .fold(0.0f32, f32::max);
    assert!(
        panel_width >= 600.0,
        "the expanded panel should span the accordion, got {panel_width}"
    );

    let header = driver
        .find_text("Section 2")
        .expect("expanded section header")
        .bounds;
    let bordered = |contains: &dyn Fn(fission::layout::LayoutRect) -> bool| {
        ir.nodes
            .iter()
            .filter(|(_, node)| {
                let op = format!("{:?}", node.op);
                op.starts_with("Paint(DrawRect") && op.contains("stroke: Some")
            })
            .filter_map(|(id, _)| snapshot.get_node_rect(*id))
            .filter(|rect| contains(*rect) && rect.height() < 200.0)
            .max_by(|a, b| a.width().total_cmp(&b.width()))
    };
    let header_surface = bordered(&|rect| {
        rect.y() <= header.y() && rect.y() + rect.height() >= header.y() + header.height()
    })
    .expect("bordered header surface");
    let panel_surface = bordered(&|rect| {
        rect.y() <= content.y() && rect.y() + rect.height() >= content.y() + content.height()
    })
    .expect("bordered panel surface");
    assert!(
        (header_surface.x() - panel_surface.x()).abs() < 1.0
            && (header_surface.width() - panel_surface.width()).abs() < 1.0,
        "the header and its panel should share edges, got header {header_surface:?} and panel {panel_surface:?}"
    );
}

#[test]
fn an_accordion_header_is_a_button_that_states_its_expansion() {
    let mut driver = TestDriver::new(
        TestHarness::new(GalleryState {
            page: GalleryPage::Accordion,
            accordion_open: 1,
            ..GalleryState::default()
        })
        .with_root_widget(GalleryApp),
    );
    driver.harness.env.viewport_size = LayoutSize::new(1000.0, 3000.0);
    driver.pump().expect("first frame");
    let ir = driver.harness.last_ir.as_ref().expect("ir");
    let header = ir
        .nodes
        .values()
        .find_map(|node| match &node.op {
            fission::core::op::Op::Semantics(semantics)
                if semantics.label.as_deref() == Some("Section 2") =>
            {
                Some(semantics.clone())
            }
            _ => None,
        })
        .expect("Section 2 header");
    assert_eq!(header.role, Role::Button);
    assert_eq!(header.expanded, Some(true));
    assert_eq!(
        header.controls.len(),
        1,
        "the header names the panel it controls"
    );
}
