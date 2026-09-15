//! The gallery's modal surface hugs its content instead of filling the window.

use fission::layout::LayoutSize;
use fission_test::{TestDriver, TestHarness};
use widget_gallery::{GalleryApp, GalleryPage, GalleryState};

#[test]
fn gallery_modal_surface_hugs_its_content() {
    let state = GalleryState {
        page: GalleryPage::Modal,
        modal_open: true,
        ..GalleryState::default()
    };
    let mut driver = TestDriver::new(TestHarness::new(state).with_root_widget(GalleryApp));
    driver.harness.env.viewport_size = LayoutSize::new(1000.0, 760.0);
    driver.pump().expect("first frame");
    driver.pump().expect("second frame");
    let surface = driver
        .find_semantics_identifier("gallery.modal.surface")
        .expect("gallery modal surface");

    if surface.bounds.height() >= 300.0 {
        let ir = driver.harness.last_ir.as_ref().expect("ir");
        let snapshot = driver.harness.last_snapshot.as_ref().expect("snapshot");
        let mut stack = vec![(surface.node_id, 0usize)];
        while let Some((id, depth)) = stack.pop() {
            if depth > 8 {
                continue;
            }
            let node = &ir.nodes[&id];
            let op: String = format!("{:?}", node.op).chars().take(110).collect();
            eprintln!(
                "DBG {}{} rect={:?}",
                "  ".repeat(depth),
                op,
                snapshot
                    .get_node_rect(id)
                    .map(|r| (r.origin.y, r.size.width, r.size.height))
            );
            for child in node.children.iter().rev() {
                stack.push((*child, depth + 1));
            }
        }
    }
    assert!(
        surface.bounds.height() < 300.0,
        "the gallery modal surface should hug its content, got {:?}",
        surface.bounds
    );
}
