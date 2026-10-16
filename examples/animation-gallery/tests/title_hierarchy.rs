//! The gallery names itself once: repeated app titles push content down and
//! blur the heading hierarchy.

use animation_gallery::{AnimationGalleryApp, AnimationGalleryState};
use fission_test::{op_text, TestHarness};

fn title_count(state: AnimationGalleryState, width: f32, height: f32) -> usize {
    let mut harness = TestHarness::new(state).with_root_widget(AnimationGalleryApp);
    harness.env.viewport_size = fission::layout::LayoutSize::new(width, height);
    harness.pump().expect("pump animation gallery");
    let ir = harness.last_ir.as_ref().expect("ir");
    ir.nodes
        .iter()
        .filter(|(_, node)| {
            op_text(&node.op).is_some_and(|text| text.contains("Animation Gallery"))
        })
        .count()
}

#[test]
fn standalone_gallery_shows_its_title_once() {
    for (width, height) in [(880.0, 660.0), (1280.0, 860.0)] {
        assert_eq!(
            title_count(AnimationGalleryState::default(), width, height),
            1,
            "at {width}x{height}"
        );
    }
}

/// Embedded in the showcase, whose header already names the example, the
/// gallery drops its brand card and hero.
#[test]
fn embedded_gallery_drops_its_brand_and_hero() {
    for (width, height) in [(880.0, 660.0), (1280.0, 860.0)] {
        assert_eq!(
            title_count(animation_gallery::embedded_state(), width, height),
            0,
            "at {width}x{height}"
        );
    }
}
