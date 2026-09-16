//! The app brand sits in the sidebar at app-bar size, not as a display heading
//! that wraps and outranks the folder title beside it.

use fission::layout::LayoutSize;
use fission::render::LayoutRect;
use fission_test::TestHarness;
use inbox::{create_env, InboxApp, InboxState};

fn text_rects(h: &TestHarness<InboxState>, needle: &str) -> Vec<LayoutRect> {
    let ir = h.last_ir.as_ref().expect("pump should produce IR");
    let snapshot = h.last_snapshot.as_ref().expect("pump should lay out");
    ir.nodes
        .iter()
        .filter(|(_, node)| node.op.text().as_deref() == Some(needle))
        .filter_map(|(id, _)| snapshot.get_node_rect(*id))
        .filter(|rect| rect.size.width > 0.0 && rect.size.height > 0.0)
        .collect()
}

fn pump(state: InboxState) -> TestHarness<InboxState> {
    let mut h = TestHarness::new(state).with_root_widget(InboxApp);
    h.env = create_env();
    h.env.viewport_size = LayoutSize::new(880.0, 660.0);
    h.pump().expect("inbox should build and lay out");
    h
}

#[test]
fn standalone_brand_is_app_bar_sized_and_smaller_than_the_folder_title() {
    let h = pump(InboxState::default());
    let brand = text_rects(&h, "Fission Inbox");
    assert_eq!(brand.len(), 1, "brand should render once");
    // "Inbox" is both a sidebar item and the folder title; the title is the taller one.
    let folder = text_rects(&h, "Inbox")
        .into_iter()
        .max_by(|a, b| a.size.height.total_cmp(&b.size.height))
        .expect("folder title is laid out");
    let folder = &folder;
    assert!(
        brand[0].size.height <= folder.size.height + 0.5,
        "brand ({:?}) outranks the folder title ({folder:?})",
        brand[0]
    );
}

/// Embedded in the showcase, whose header already names the example, the
/// sidebar drops the brand; the folder title stays.
#[test]
fn embedded_inbox_drops_the_brand() {
    let h = pump(inbox::embedded_state());
    assert!(text_rects(&h, "Fission Inbox").is_empty());
    assert!(!text_rects(&h, "Inbox").is_empty());
}
