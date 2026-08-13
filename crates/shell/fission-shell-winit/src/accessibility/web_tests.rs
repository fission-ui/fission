use super::{
    clamp_boundary, directed_dom_selection, keyboard_intent, replacement_delta, safe_scale,
    scroll_semantic, KeyboardIntent, ScrollCommand,
};
use fission_core::Runtime;
use fission_ir::{CoreIR, CoreNode, FlexDirection, LayoutOp, Op, Role, Semantics, WidgetId};
use fission_layout::{LayoutNodeGeometry, LayoutRect, LayoutSize, LayoutSnapshot};

#[test]
fn replacement_delta_preserves_unicode_boundaries() {
    assert_eq!(
        replacement_delta("hi 🌍!", "hi 🚀!"),
        (3, 7, "🚀".to_string())
    );
    assert_eq!(replacement_delta("abc", "ac"), (1, 2, String::new()));
    assert_eq!(replacement_delta("abc", "xyz"), (0, 3, "xyz".to_string()));
}

#[test]
fn clamps_dom_selection_to_utf8_boundary() {
    assert_eq!(clamp_boundary("🌍", 2), 0);
    assert_eq!(clamp_boundary("🌍", 4), 4);
}

#[test]
fn canvas_scale_is_css_to_logical() {
    assert_eq!(safe_scale(600.0, 300.0), 2.0);
    assert_eq!(safe_scale(f64::NAN, 300.0), 1.0);
}

#[test]
fn browser_selection_direction_preserves_anchor_and_caret() {
    assert_eq!(directed_dom_selection(2, 7, Some("forward")), (2, 7));
    assert_eq!(directed_dom_selection(2, 7, Some("backward")), (7, 2));
    assert_eq!(directed_dom_selection(4, 4, None), (4, 4));
}

#[test]
fn semantic_keyboard_controls_have_native_activation_equivalents() {
    assert_eq!(
        key(Some("button"), true, false, false, false, "Enter", false),
        Some(KeyboardIntent::Activate)
    );
    assert_eq!(
        key(Some("checkbox"), true, false, false, false, " ", false),
        Some(KeyboardIntent::Activate)
    );
    assert_eq!(
        key(Some("slider"), false, false, false, false, "Home", false),
        Some(KeyboardIntent::Minimum)
    );
    assert_eq!(
        key(
            Some("slider"),
            false,
            false,
            false,
            false,
            "ArrowRight",
            true,
        ),
        Some(KeyboardIntent::Increase)
    );
}

#[test]
fn composition_and_key_repeat_do_not_submit_or_reactivate_controls() {
    assert_eq!(
        keyboard_intent(
            Some("presentation"),
            false,
            false,
            false,
            true,
            "Enter",
            true,
            false,
        ),
        None
    );
    assert_eq!(
        key(Some("button"), true, false, false, false, "Enter", true),
        None
    );
    assert_eq!(
        key(
            Some("presentation"),
            false,
            false,
            false,
            true,
            "Enter",
            false,
        ),
        Some(KeyboardIntent::Submit)
    );
}

#[test]
fn scrollable_semantics_map_keyboard_navigation_to_the_owned_axis() {
    assert_eq!(
        key(Some("list"), false, false, true, false, "PageDown", false),
        Some(KeyboardIntent::Scroll {
            horizontal: false,
            command: ScrollCommand::Forward,
        })
    );
    assert_eq!(
        key(Some("group"), false, true, false, false, "Home", false),
        Some(KeyboardIntent::Scroll {
            horizontal: true,
            command: ScrollCommand::Start,
        })
    );
}

#[test]
fn semantic_scroll_updates_the_framework_scroll_authority() {
    let semantic = WidgetId::from_u128(40);
    let scroll = WidgetId::from_u128(41);
    let mut ir = CoreIR::new();
    add_node(
        &mut ir,
        scroll,
        Op::Layout(LayoutOp::Scroll {
            direction: FlexDirection::Column,
            show_scrollbar: true,
            width: None,
            height: None,
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            padding: [0.0; 4],
            flex_grow: 0.0,
            flex_shrink: 1.0,
        }),
        Vec::new(),
    );
    add_node(
        &mut ir,
        semantic,
        Op::Semantics(Semantics {
            role: Role::List,
            scrollable_y: true,
            ..Semantics::default()
        }),
        vec![scroll],
    );
    ir.root = Some(semantic);
    let mut layout = LayoutSnapshot::new(LayoutSize::new(100.0, 100.0));
    layout.nodes.insert(
        scroll,
        LayoutNodeGeometry {
            rect: LayoutRect::new(0.0, 0.0, 100.0, 100.0),
            content_size: LayoutSize::new(100.0, 400.0),
        },
    );
    let mut runtime = Runtime::default();

    assert!(scroll_semantic(
        &mut runtime,
        &ir,
        &layout,
        semantic,
        false,
        ScrollCommand::Forward,
    ));
    assert_eq!(runtime.runtime_state.scroll.get_offset(scroll), 80.0);
    assert!(scroll_semantic(
        &mut runtime,
        &ir,
        &layout,
        semantic,
        false,
        ScrollCommand::End,
    ));
    assert_eq!(runtime.runtime_state.scroll.get_offset(scroll), 300.0);
}

fn key(
    role: Option<&str>,
    activatable: bool,
    scrollable_x: bool,
    scrollable_y: bool,
    single_line_text: bool,
    key: &str,
    repeat: bool,
) -> Option<KeyboardIntent> {
    keyboard_intent(
        role,
        activatable,
        scrollable_x,
        scrollable_y,
        single_line_text,
        key,
        false,
        repeat,
    )
}

fn add_node(ir: &mut CoreIR, id: WidgetId, op: Op, children: Vec<WidgetId>) {
    ir.nodes.insert(
        id,
        CoreNode {
            id,
            op,
            composite: Default::default(),
            children: children.clone(),
            parent: None,
            hash: 0,
        },
    );
    for child in children {
        ir.nodes.get_mut(&child).unwrap().parent = Some(id);
    }
}
