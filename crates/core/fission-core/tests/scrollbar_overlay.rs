//! A scroll view's bar is a thin overlay, not a rule between two panes.
//!
//! The bar used to paint a fixed grey rail behind an opaque thumb at every
//! moment, six points wide, which beside a split pane read as a divider. It is
//! now thin, takes its colours from the theme, and only brings the rail
//! forward for the scroll view the pointer is in or the thumb being dragged.

use fission_core::scrollbar::{
    active_scrollbar_nodes, ScrollbarDragState, ScrollbarPalette, ScrollbarStyle,
    SCROLLBAR_HIT_SLOP, SCROLLBAR_THICKNESS,
};
use fission_ir::{CompositeStyle, CoreIR, CoreNode, FlexDirection, LayoutOp, Op, WidgetId};
use fission_theme::{DesignMode, DesignSystem, FissionDefaultDesignSystem, Tokens};

#[test]
fn the_bar_is_thin_but_still_grabbable() {
    assert!(
        SCROLLBAR_THICKNESS <= 4.0,
        "an overlay bar reads as a thumb, not a rule: {SCROLLBAR_THICKNESS}pt"
    );
    assert!(
        SCROLLBAR_THICKNESS + SCROLLBAR_HIT_SLOP >= 10.0,
        "the pointer target stays wider than the paint"
    );
}

#[test]
fn a_resting_bar_has_no_rail_and_a_quiet_thumb() {
    let tokens = Tokens::default();
    let resting = ScrollbarStyle::from_tokens(&tokens, false);
    let active = ScrollbarStyle::from_tokens(&tokens, true);

    assert_eq!(
        resting.rail.a, 0,
        "the rail is what read as a divider, so a resting bar draws none"
    );
    assert!(active.rail.a > 0, "an active bar shows its track");
    assert!(
        resting.thumb.a > 0 && resting.thumb.a < active.thumb.a,
        "the resting thumb stays visible but quieter than the active one: \
         {} vs {}",
        resting.thumb.a,
        active.thumb.a
    );
}

#[test]
fn the_bar_follows_the_theme_rather_than_a_fixed_grey() {
    let light = ScrollbarStyle::from_tokens(
        &FissionDefaultDesignSystem::theme(DesignMode::Light).tokens,
        true,
    );
    let dark = ScrollbarStyle::from_tokens(
        &FissionDefaultDesignSystem::theme(DesignMode::Dark).tokens,
        true,
    );
    assert_ne!(
        (light.thumb.r, light.thumb.g, light.thumb.b),
        (dark.thumb.r, dark.thumb.g, dark.thumb.b),
        "a dark design system gets a light bar"
    );
}

fn scroll_ir() -> (CoreIR, WidgetId, WidgetId) {
    let scroll = WidgetId::explicit("scroll");
    let child = WidgetId::explicit("child");
    let mut ir = CoreIR::default();
    ir.nodes.insert(
        scroll,
        CoreNode {
            id: scroll,
            parent: None,
            children: vec![child],
            op: Op::Layout(LayoutOp::Scroll {
                direction: FlexDirection::Column,
                show_scrollbar: true,
                width: Some(100.0),
                height: Some(200.0),
                min_width: None,
                max_width: None,
                min_height: None,
                max_height: None,
                padding: [0.0; 4],
                flex_grow: 0.0,
                flex_shrink: 0.0,
            }),
            composite: CompositeStyle::default(),
            hash: 0,
        },
    );
    ir.nodes.insert(
        child,
        CoreNode {
            id: child,
            parent: Some(scroll),
            children: Vec::new(),
            op: Op::Layout(LayoutOp::Box {
                width: None,
                height: None,
                min_width: None,
                max_width: None,
                min_height: None,
                max_height: None,
                padding: [0.0; 4],
                flex_grow: 0.0,
                flex_shrink: 0.0,
                aspect_ratio: None,
            }),
            composite: CompositeStyle::default(),
            hash: 0,
        },
    );
    ir.set_root(scroll);
    (ir, scroll, child)
}

#[test]
fn pointing_into_a_scroll_view_brings_its_bar_forward() {
    let (ir, scroll, child) = scroll_ir();

    // A hover path names the node under the pointer and its ancestors, so a
    // pointer anywhere in the content counts as pointing at the scroll view.
    let active = active_scrollbar_nodes(&ir, &[child, scroll], None);
    assert!(active.contains(&scroll));
    assert!(
        !active.contains(&child),
        "only scroll views get a bar, not everything the pointer passes over"
    );
    assert!(active_scrollbar_nodes(&ir, &[], None).is_empty());

    let mut palette = ScrollbarPalette::default();
    palette.set_theme(&Tokens::default());
    palette.set_active_nodes(active);
    assert!(
        palette.style_for(scroll).rail.a > 0,
        "the scroll view under the pointer shows its rail"
    );
    assert_eq!(
        palette.style_for(WidgetId::explicit("elsewhere")).rail.a,
        0,
        "every other scroll view stays at rest"
    );
}

#[test]
fn dragging_a_thumb_keeps_its_own_bar_forward() {
    let (ir, scroll, _) = scroll_ir();
    // The pointer leaves the pane while the thumb is still held, which is
    // exactly when the bar must not fade back out.
    let active = active_scrollbar_nodes(
        &ir,
        &[],
        Some(ScrollbarDragState {
            node_id: scroll,
            pointer_to_thumb_start: 4.0,
        }),
    );
    assert!(active.contains(&scroll));
}
