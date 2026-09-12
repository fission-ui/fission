//! Accessible names and scrollability for widgets that carry no text.
//!
//! A button can fall back on the text inside it. An image, an icon, a video and
//! a scroll view cannot: if they do not declare what they are, assistive
//! technology has nothing at all to work with.

use fission_core::authoring::LoweringCx;
use fission_core::env::{Env, RuntimeState};
use fission_core::ui::{Icon, Image, Scroll, Text, TextContent, Video};
use fission_core::Widget;
use fission_ir::{op::FlexDirection, Op, Role, Semantics};

fn semantics_of(widget: Widget) -> Vec<Semantics> {
    let env = Env::default();
    let runtime = RuntimeState::default();
    let mut cx = LoweringCx::new(&env, &runtime, None, None);
    let root = fission_core::internal::lower_widget(&widget, &mut cx);
    cx.ir.root = Some(root);
    cx.ir
        .nodes
        .values()
        .filter_map(|node| match &node.op {
            Op::Semantics(semantics) => Some(semantics.clone()),
            _ => None,
        })
        .collect()
}

fn find(widget: Widget, role: Role) -> Option<Semantics> {
    semantics_of(widget).into_iter().find(|s| s.role == role)
}

#[test]
fn a_labelled_image_is_announced_on_every_target() {
    // The site shell reads the label off the paint op to emit `alt`. AccessKit
    // only walks semantic nodes, so the label has to exist as semantics too or
    // the image is named on the web and silent everywhere else.
    let image = Image::network("https://example.com/cat.png").semantic_label("A sleeping cat");
    let semantics = find(image.into(), Role::Image).expect("image semantics");
    assert_eq!(semantics.label.as_deref(), Some("A sleeping cat"));
}

#[test]
fn an_unlabelled_image_adds_no_semantics() {
    // A decorative image must stay out of the accessibility tree rather than
    // appear in it as an unnamed node the reader has to step through.
    let image = Image::network("https://example.com/texture.png");
    assert!(find(image.into(), Role::Image).is_none());
}

#[test]
fn a_meaningful_icon_can_be_named() {
    let icon = Icon::path("M12 2L2 22h20L12 2z").semantic_label("Warning");
    let semantics = find(icon.into(), Role::Image).expect("icon semantics");
    assert_eq!(semantics.label.as_deref(), Some("Warning"));
}

#[test]
fn a_decorative_icon_adds_no_semantics() {
    // Icons inside labelled controls are the common case; naming them would
    // make the control announce itself twice.
    let icon = Icon::path("M12 2L2 22h20L12 2z");
    assert!(find(icon.into(), Role::Image).is_none());
}

#[test]
fn a_video_announces_itself_even_unnamed() {
    // Unlike an image, there is no decorative video. A media surface with no
    // semantics is simply invisible to a screen reader.
    let semantics = find(Video::asset("clip.mp4").into(), Role::Video).expect("video semantics");
    assert_eq!(semantics.label, None);

    let named = Video::asset("clip.mp4").semantic_label("Product tour");
    let semantics = find(named.into(), Role::Video).expect("video semantics");
    assert_eq!(semantics.label.as_deref(), Some("Product tour"));
}

#[test]
fn a_vertical_scroll_declares_its_axis() {
    // The shells can report offsets and accept scroll actions, but they key off
    // these two flags. Nothing set them, so screen readers could not scroll a
    // Fission scroll view on any target.
    let scroll = Scroll {
        direction: FlexDirection::Column,
        child: Some(
            Text {
                content: TextContent::Literal("content".into()),
                ..Default::default()
            }
            .into(),
        ),
        ..Default::default()
    };
    let semantics = find(scroll.into(), Role::Generic).expect("scroll semantics");
    assert!(semantics.scrollable_y);
    assert!(!semantics.scrollable_x);
}

#[test]
fn a_horizontal_scroll_declares_the_other_axis() {
    let scroll = Scroll {
        direction: FlexDirection::Row,
        child: Some(
            Text {
                content: TextContent::Literal("content".into()),
                ..Default::default()
            }
            .into(),
        ),
        ..Default::default()
    };
    let semantics = find(scroll.into(), Role::Generic).expect("scroll semantics");
    assert!(semantics.scrollable_x);
    assert!(!semantics.scrollable_y);
}

#[test]
fn a_scroll_region_can_be_named() {
    let scroll = Scroll {
        direction: FlexDirection::Column,
        ..Default::default()
    }
    .semantic_label("Search results");
    let semantics = find(scroll.into(), Role::Generic).expect("scroll semantics");
    assert_eq!(semantics.label.as_deref(), Some("Search results"));
}
