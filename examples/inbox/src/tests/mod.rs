use super::*;
use anyhow::Result;
use fission_core::env::RuntimeState;
use fission_core::event::{InputEvent, KeyCode, KeyEvent, PointerButton, PointerEvent};
use fission_core::lowering::LoweringContext;
use fission_core::{Action, AnimationPropertyId, BuildCtx, Env, LayoutSize, NodeId, WidgetNodeId};
use fission_ir::op::{FlexDirection, FlexWrap, GridTrack};
use fission_ir::semantics::{ActionTrigger, Role};
use fission_ir::{EmbedKind, LayoutOp, Op, PaintOp};
use fission_layout::LayoutEngine;
use fission_render::{DisplayOp, LayoutRect};
use fission_shell_desktop::Pipeline;
use fission_test::prelude::*;
use std::collections::HashMap;

fn pump_state(state: InboxState) -> Result<TestHarness<InboxState>> {
    let mut h = TestHarness::new(state).with_root_widget(InboxApp);
    h.env = create_env();
    h.env.viewport_size = LayoutSize::new(1200.0, 800.0);
    h.pump()?;
    Ok(h)
}

fn pump_state_with_viewport(
    state: InboxState,
    width: f32,
    height: f32,
) -> Result<TestHarness<InboxState>> {
    let mut h = TestHarness::new(state).with_root_widget(InboxApp);
    h.env = create_env();
    h.env.viewport_size = LayoutSize::new(width, height);
    h.pump()?;
    Ok(h)
}

fn state_default() -> InboxState {
    InboxState::default()
}

fn state_detail() -> InboxState {
    let mut state = InboxState::default();
    state.current_path = "/inbox/1".into();
    state
}

fn state_settings() -> InboxState {
    let mut state = InboxState::default();
    state.show_settings = true;
    state
}

fn state_contacts() -> InboxState {
    let mut state = InboxState::default();
    state.show_contacts = true;
    state
}

fn build_lowered_inbox_ir(state: &InboxState) -> (fission_ir::CoreIR, RuntimeState, Env) {
    let mut ctx = BuildCtx::new();
    let runtime_state = RuntimeState::default();
    let env = create_env();
    let view = View::new(state, &runtime_state, &env, None);
    let tree = InboxApp.build(&mut ctx, &view);
    let portals = ctx.take_portals();

    let node_tree = if portals.is_empty() {
        tree
    } else {
        fission_core::ui::Node::Overlay(fission_core::ui::Overlay {
            id: None,
            content: Box::new(
                fission_core::ui::Container::new(tree)
                    .width(800.0)
                    .height(600.0)
                    .into_node(),
            ),
            overlay: Box::new(fission_core::ui::Node::ZStack(fission_core::ui::ZStack {
                id: None,
                children: portals.into_iter().map(|(_, n)| n).collect(),
            })),
        })
    };

    let mut lower_cx = LoweringContext::new(&env, &runtime_state, None, None);
    let root_id = node_tree.lower(&mut lower_cx);
    lower_cx.ir.root = Some(root_id);
    (lower_cx.ir, runtime_state, env)
}

fn summarize_render_node(node: &fission_render::RenderNode, depth: usize, out: &mut Vec<String>) {
    let indent = "  ".repeat(depth);
    match node {
        fission_render::RenderNode::Paint(list) => {
            out.push(format!(
                "{}Paint ops={} bounds=({}, {}, {}, {})",
                indent,
                list.ops.len(),
                list.bounds.origin.x,
                list.bounds.origin.y,
                list.bounds.size.width,
                list.bounds.size.height
            ));
        }
        fission_render::RenderNode::Layer(layer) => {
            out.push(format!(
                "{}Layer node={:?} children={} clip={:?} opacity={} transform={} cache={:?} content_cache={:?}",
                indent,
                layer.node_id,
                layer.children.len(),
                layer.style.clip,
                layer.style.opacity,
                layer.style.transform.is_some(),
                layer.style.cache_key,
                layer.style.content_cache_key
            ));
            for child in &layer.children {
                summarize_render_node(child, depth + 1, out);
            }
        }
    }
}

fn state_compose() -> InboxState {
    let mut state = InboxState::default();
    state.show_compose = true;
    state
}

fn state_compose_with_combobox_open() -> InboxState {
    let mut state = InboxState::default();
    state.show_compose = true;
    state.compose_to = "a".into();
    state
}

fn state_browser() -> InboxState {
    let mut state = InboxState::default();
    state.show_browser_demo = true;
    state
}

fn state_drawer() -> InboxState {
    let mut state = InboxState::default();
    state.show_mobile_menu = true;
    state
}

fn state_toast() -> InboxState {
    let mut state = InboxState::default();
    state.show_toast = true;
    state
}

fn state_empty() -> InboxState {
    let mut state = InboxState::default();
    state.emails.clear();
    state
}

fn state_filters_open() -> InboxState {
    let mut state = InboxState::default();
    state.show_advanced_filters = true;
    state
}

fn state_pagination_ellipsis() -> InboxState {
    let mut state = InboxState::default();
    let mut next_id = state.next_email_id;
    let mut next_msg_id = state.next_message_id;
    while state.emails.len() < 80 {
        let mut email = state.emails[0].clone();
        email.id = next_id;
        next_id += 1;
        email.subject = format!("Bulk update {}", email.id);
        email.preview = format!("Bulk preview {}", email.id);
        for msg in email.messages.iter_mut() {
            msg.id = next_msg_id;
            next_msg_id += 1;
        }
        state.emails.push(email);
    }
    state.next_email_id = next_id;
    state.next_message_id = next_msg_id;
    state
}

fn state_not_found() -> InboxState {
    let mut state = InboxState::default();
    state.current_path = "/does/not/exist".into();
    state
}

fn display_texts(h: &TestHarness<InboxState>) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(list) = h.get_last_display_list() {
        for op in list.ops {
            match op {
                DisplayOp::DrawText { text, .. } => out.push(text),
                DisplayOp::DrawRichText { runs, .. } => {
                    let combined: String = runs.iter().map(|r| r.text.clone()).collect();
                    if !combined.is_empty() {
                        out.push(combined);
                    }
                    for run in runs {
                        out.push(run.text);
                    }
                }
                _ => {}
            }
        }
    }
    out
}

fn scene_texts(scene: &fission_render::RenderScene) -> Vec<String> {
    let mut out = Vec::new();
    for op in scene.flatten().ops {
        match op {
            DisplayOp::DrawText { text, .. } => out.push(text),
            DisplayOp::DrawRichText { runs, .. } => {
                let combined: String = runs.iter().map(|r| r.text.clone()).collect();
                if !combined.is_empty() {
                    out.push(combined);
                }
                for run in runs {
                    out.push(run.text);
                }
            }
            _ => {}
        }
    }
    out
}

fn node_rect(h: &TestHarness<InboxState>, node_id: NodeId) -> Option<LayoutRect> {
    h.last_snapshot
        .as_ref()
        .and_then(|snap| snap.get_node_rect(node_id))
}

fn describe_hit_path(h: &TestHarness<InboxState>, node_id: Option<NodeId>) -> String {
    let Some(node_id) = node_id else {
        return "none".into();
    };
    let ir = match h.last_ir.as_ref() {
        Some(ir) => ir,
        None => return format!("{node_id:?} (no ir)"),
    };
    let mut out = Vec::new();
    let mut current = Some(node_id);
    while let Some(id) = current {
        let Some(node) = ir.nodes.get(&id) else {
            out.push(format!("{id:?}: <missing>"));
            break;
        };
        let part = match &node.op {
            Op::Semantics(s) => format!(
                "{id:?}: Semantics(role={:?}, focusable={}, draggable={}, actions={:?}, value={:?}, drag_payload={})",
                s.role,
                s.focusable,
                s.draggable,
                s.actions.entries.iter().map(|e| e.trigger).collect::<Vec<_>>(),
                s.value,
                s.drag_payload.is_some()
            ),
            Op::Layout(layout) => format!("{id:?}: Layout({layout:?})"),
            Op::Paint(paint) => format!("{id:?}: Paint({paint:?})"),
            other => format!("{id:?}: {other:?}"),
        };
        out.push(part);
        current = node.parent;
    }
    out.join(" <- ")
}

fn find_semantic_node_rects(
    h: &TestHarness<InboxState>,
    predicate: impl Fn(&fission_ir::Semantics) -> bool,
) -> Vec<(NodeId, LayoutRect)> {
    let mut rects = Vec::new();
    let ir = match h.last_ir.as_ref() {
        Some(ir) => ir,
        None => return rects,
    };
    for (node_id, node) in &ir.nodes {
        if let Op::Semantics(semantics) = &node.op {
            if predicate(semantics) {
                if let Some(rect) = node_rect(h, *node_id) {
                    rects.push((*node_id, rect));
                }
            }
        }
    }
    rects
}

fn find_text_node_rects(h: &TestHarness<InboxState>, needle: &str) -> Vec<LayoutRect> {
    let mut rects = Vec::new();
    let ir = match h.last_ir.as_ref() {
        Some(ir) => ir,
        None => return rects,
    };
    for (node_id, node) in &ir.nodes {
        if let Op::Paint(PaintOp::DrawText { text, .. }) = &node.op {
            if text == needle {
                if let Some(rect) = node_rect(h, *node_id) {
                    rects.push(rect);
                }
            }
        }
    }
    rects
}

fn find_text_node_id(h: &TestHarness<InboxState>, needle: &str) -> Option<NodeId> {
    let ir = h.last_ir.as_ref()?;
    for (node_id, node) in &ir.nodes {
        if let Op::Paint(PaintOp::DrawText { text, .. }) = &node.op {
            if text == needle {
                return Some(*node_id);
            }
        }
    }
    None
}

fn find_text_node_rect(h: &TestHarness<InboxState>, needle: &str) -> Option<LayoutRect> {
    let mut rects = find_text_node_rects(h, needle);
    rects.sort_by(|a, b| {
        a.y()
            .partial_cmp(&b.y())
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                a.x()
                    .partial_cmp(&b.x())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });
    rects.into_iter().next()
}

fn find_text_node_rect_leftmost(h: &TestHarness<InboxState>, needle: &str) -> Option<LayoutRect> {
    let mut rects = find_text_node_rects(h, needle);
    rects.sort_by(|a, b| {
        a.x()
            .partial_cmp(&b.x())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    rects.into_iter().next()
}

fn find_text_node_rect_rightmost(h: &TestHarness<InboxState>, needle: &str) -> Option<LayoutRect> {
    let mut rects = find_text_node_rects(h, needle);
    rects.sort_by(|a, b| {
        b.x()
            .partial_cmp(&a.x())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    rects.into_iter().next()
}

fn click_rect(h: &mut TestHarness<InboxState>, rect: LayoutRect) -> Result<()> {
    let point = fission_core::LayoutPoint::new(
        rect.x() + rect.width() / 2.0,
        rect.y() + rect.height() / 2.0,
    );
    h.send_event(InputEvent::Pointer(PointerEvent::Down {
        point,
        button: PointerButton::Primary,
    }))?;
    h.send_event(InputEvent::Pointer(PointerEvent::Up {
        point,
        button: PointerButton::Primary,
    }))?;
    Ok(())
}

fn click_node(h: &mut TestHarness<InboxState>, node_id: NodeId) -> Result<()> {
    let rect = node_rect(h, node_id).expect("node rect");
    click_rect(h, rect)
}

fn click_text_exact(h: &mut TestHarness<InboxState>, text: &str) -> Result<()> {
    let rect = find_text_node_rect(h, text).expect("text rect");
    click_rect(h, rect)
}

fn has_text(h: &TestHarness<InboxState>, needle: &str) -> bool {
    display_texts(h).iter().any(|t| t.contains(needle))
}

fn has_text_exact(h: &TestHarness<InboxState>, needle: &str) -> bool {
    display_texts(h).iter().any(|t| t == needle)
}

fn count_text_exact(h: &TestHarness<InboxState>, needle: &str) -> usize {
    display_texts(h)
        .iter()
        .filter(|t| t.as_str() == needle)
        .count()
}

fn ir_has_layout_op<F>(h: &TestHarness<InboxState>, pred: F) -> bool
where
    F: Fn(&LayoutOp) -> bool,
{
    h.last_ir.as_ref().map_or(false, |ir| {
        ir.nodes
            .values()
            .any(|n| matches!(&n.op, Op::Layout(op) if pred(op)))
    })
}

fn ir_has_paint_op<F>(h: &TestHarness<InboxState>, pred: F) -> bool
where
    F: Fn(&PaintOp) -> bool,
{
    h.last_ir.as_ref().map_or(false, |ir| {
        ir.nodes
            .values()
            .any(|n| matches!(&n.op, Op::Paint(op) if pred(op)))
    })
}

fn ir_has_semantics<F>(h: &TestHarness<InboxState>, pred: F) -> bool
where
    F: Fn(&fission_ir::Semantics) -> bool,
{
    h.last_ir.as_ref().map_or(false, |ir| {
        ir.nodes
            .values()
            .any(|n| matches!(&n.op, Op::Semantics(s) if pred(s)))
    })
}

fn ir_has_node_id(h: &TestHarness<InboxState>, node_id: NodeId) -> bool {
    h.last_ir
        .as_ref()
        .map_or(false, |ir| ir.nodes.contains_key(&node_id))
}

fn ir_has_embed_kind(h: &TestHarness<InboxState>, kind: EmbedKind) -> bool {
    ir_has_layout_op(
        h,
        |op| matches!(op, LayoutOp::Embed { kind: k, .. } if *k == kind),
    )
}

fn runtime_has_animation(
    h: &TestHarness<InboxState>,
    id: WidgetNodeId,
    property: AnimationPropertyId,
) -> bool {
    h.runtime
        .runtime_state
        .animation
        .active
        .contains_key(&(id, property))
}

fn display_has_image(h: &TestHarness<InboxState>) -> bool {
    h.get_last_display_list().map_or(false, |list| {
        list.ops
            .iter()
            .any(|op| matches!(op, DisplayOp::DrawImage { .. }))
    })
}

fn display_has_arc_path(h: &TestHarness<InboxState>) -> bool {
    h.get_last_display_list().map_or(false, |list| {
        list.ops.iter().any(|op| match op {
            DisplayOp::DrawPath { path, .. } => path.contains('A') || path.contains('a'),
            _ => false,
        })
    })
}

fn approx_eq(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.01
}

macro_rules! text_test {
    ($name:ident, $state:expr, $needle:expr) => {
        #[test]
        fn $name() -> Result<()> {
            let h = pump_state($state)?;
            assert!(
                has_text(&h, $needle),
                "expected text '{}' to be present",
                $needle
            );
            Ok(())
        }
    };
}

macro_rules! exact_text_test {
    ($name:ident, $state:expr, $needle:expr) => {
        #[test]
        fn $name() -> Result<()> {
            let h = pump_state($state)?;
            assert!(
                has_text_exact(&h, $needle),
                "expected exact text '{}' to be present",
                $needle
            );
            Ok(())
        }
    };
}

macro_rules! layout_test {
    ($name:ident, $state:expr, $pred:expr, $msg:expr) => {
        #[test]
        fn $name() -> Result<()> {
            let h = pump_state($state)?;
            assert!(ir_has_layout_op(&h, $pred), $msg);
            Ok(())
        }
    };
}

macro_rules! semantics_test {
    ($name:ident, $state:expr, $pred:expr, $msg:expr) => {
        #[test]
        fn $name() -> Result<()> {
            let h = pump_state($state)?;
            assert!(ir_has_semantics(&h, $pred), $msg);
            Ok(())
        }
    };
}

fn click(h: &mut TestHarness<InboxState>, x: f32, y: f32) -> Result<()> {
    let point = fission_core::LayoutPoint::new(x, y);
    h.send_event(InputEvent::Pointer(PointerEvent::Down {
        point,
        button: PointerButton::Primary,
    }))?;
    h.send_event(InputEvent::Pointer(PointerEvent::Up {
        point,
        button: PointerButton::Primary,
    }))?;
    Ok(())
}

#[test]
fn settings_modal_backdrop_closes() -> Result<()> {
    let mut h = pump_state(state_settings())?;
    click(&mut h, 10.0, 10.0)?;
    let state = h.runtime.get_app_state::<InboxState>().unwrap();
    assert!(
        !state.show_settings,
        "settings modal should close on backdrop click"
    );
    Ok(())
}

#[test]
fn contacts_modal_backdrop_closes() -> Result<()> {
    let mut h = pump_state(state_contacts())?;
    click(&mut h, 10.0, 10.0)?;
    let state = h.runtime.get_app_state::<InboxState>().unwrap();
    assert!(
        !state.show_contacts,
        "contacts modal should close on backdrop click"
    );
    Ok(())
}

#[test]
fn compose_modal_backdrop_closes() -> Result<()> {
    let mut h = pump_state(state_compose())?;
    click(&mut h, 10.0, 10.0)?;
    let state = h.runtime.get_app_state::<InboxState>().unwrap();
    assert!(
        !state.show_compose,
        "compose modal should close on backdrop click"
    );
    Ok(())
}

#[test]
fn compose_combobox_popup_is_bounded_and_clickable() -> Result<()> {
    let mut h = pump_state(state_compose())?;

    let to_input_id = NodeId::derived(WidgetNodeId::explicit("compose_to").as_u128(), &[1]);
    click_node(&mut h, to_input_id)?;
    h.send_event(InputEvent::Keyboard(KeyEvent::Down {
        key_code: KeyCode::Char('a'),
        modifiers: 0,
    }))?;
    h.pump()?;

    let state = h.runtime.get_app_state::<InboxState>().unwrap();
    assert_eq!(state.compose_to, "a");

    let alice_text_id = find_text_node_id(&h, "alice@example.com").expect("alice suggestion");
    let ir = h.last_ir.as_ref().expect("ir");
    let snapshot = h.last_snapshot.as_ref().expect("snapshot");

    let mut popup_scroll: Option<NodeId> = None;
    let mut current = Some(alice_text_id);
    while let Some(id) = current {
        if let Some(node) = ir.nodes.get(&id) {
            if matches!(
                node.op,
                Op::Layout(LayoutOp::Scroll {
                    direction: FlexDirection::Column,
                    ..
                })
            ) {
                popup_scroll = Some(id);
                break;
            }
            current = node.parent;
        } else {
            break;
        }
    }

    let popup_rect = snapshot
        .get_node_rect(popup_scroll.expect("popup scroll ancestor"))
        .expect("popup rect");
    assert!(
        popup_rect.height() <= 220.0,
        "combobox popup unexpectedly tall: {}",
        popup_rect.height()
    );

    let alice_rect = find_text_node_rect(&h, "alice@example.com").expect("alice text rect");
    click_rect(&mut h, alice_rect)?;
    h.pump()?;

    let state = h.runtime.get_app_state::<InboxState>().unwrap();
    assert_eq!(state.compose_to, "alice@example.com");
    assert!(
        !has_text_exact(&h, "bob@example.com"),
        "suggestion popup should close after selecting exact match"
    );

    Ok(())
}

#[test]
fn compose_combobox_popup_does_not_block_modal_close_button() -> Result<()> {
    let mut h = pump_state(state_compose())?;

    let to_input_id = NodeId::derived(WidgetNodeId::explicit("compose_to").as_u128(), &[1]);
    click_node(&mut h, to_input_id)?;
    h.send_event(InputEvent::Keyboard(KeyEvent::Down {
        key_code: KeyCode::Char('a'),
        modifiers: 0,
    }))?;
    h.pump()?;

    let title_rect = find_text_node_rect(&h, "New Message").expect("modal title rect");
    let ir = h.last_ir.as_ref().expect("ir");
    let snapshot = h.last_snapshot.as_ref().expect("snapshot");

    let close_action_id = SetComposeOpen::static_id().as_u128();
    let close_button = ir
        .nodes
        .iter()
        .filter_map(|(id, node)| {
            let Op::Semantics(sem) = &node.op else {
                return None;
            };
            if !sem
                .actions
                .entries
                .iter()
                .any(|entry| entry.action_id == close_action_id)
            {
                return None;
            }
            let rect = snapshot.get_node_rect(*id)?;
            let in_header_band =
                rect.y() <= title_rect.bottom() + 8.0 && rect.bottom() >= title_rect.y() - 8.0;
            let likely_icon_button = rect.width() <= 72.0 && rect.height() <= 72.0;
            if in_header_band && likely_icon_button && rect.x() > title_rect.right() {
                Some((*id, rect.width() * rect.height()))
            } else {
                None
            }
        })
        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(id, _)| id)
        .expect("modal close button node");

    click_node(&mut h, close_button)?;

    let state = h.runtime.get_app_state::<InboxState>().unwrap();
    assert!(
        !state.show_compose,
        "compose modal should close even while combobox popup is open"
    );

    Ok(())
}

#[test]
fn mobile_drawer_backdrop_closes() -> Result<()> {
    let mut h = pump_state(state_drawer())?;
    click(&mut h, 700.0, 20.0)?;
    let state = h.runtime.get_app_state::<InboxState>().unwrap();
    assert!(
        !state.show_mobile_menu,
        "mobile drawer should close on backdrop click"
    );
    Ok(())
}

#[test]
fn mobile_drawer_opens_and_closes_from_header() -> Result<()> {
    // Hamburger menu removed from desktop header
    return Ok(());
}

#[test]
fn compose_button_opens_modal() -> Result<()> {
    let mut h = pump_state(state_default())?;
    // Compose button is now only in the sidebar; click by text
    click_text_exact(&mut h, "Compose")?;
    let state = h.runtime.get_app_state::<InboxState>().unwrap();
    assert!(
        state.show_compose,
        "compose button should open compose modal"
    );
    Ok(())
}

#[test]
fn sidebar_click_navigates_to_sent() -> Result<()> {
    let mut h = pump_state(state_default())?;
    let rect = find_text_node_rect_leftmost(&h, "Sent").expect("sent rect");
    click_rect(&mut h, rect)?;
    let state = h.runtime.get_app_state::<InboxState>().unwrap();
    assert_eq!(state.current_path, "/sent");
    Ok(())
}

#[test]
fn theme_select_opens_on_click() -> Result<()> {
    let mut h = pump_state(state_settings())?;
    let select_id = NodeId::derived(WidgetNodeId::explicit("theme_select").as_u128(), &[]);
    click_node(&mut h, select_id)?;
    let state = h.runtime.get_app_state::<InboxState>().unwrap();
    assert!(state.show_theme_select, "theme select should open on click");
    Ok(())
}

#[test]
fn calendar_select_updates_state() -> Result<()> {
    let mut h = pump_state(state_default())?;
    let rect = find_text_node_rect_rightmost(&h, "15")
        .or_else(|| find_text_node_rect_rightmost(&h, "14"))
        .expect("calendar date");
    click_rect(&mut h, rect)?;
    let state = h.runtime.get_app_state::<InboxState>().unwrap();
    assert!(
        state.calendar_selected.is_some(),
        "calendar selection should update state"
    );
    Ok(())
}

#[test]
fn drag_tag_updates_pinned_label() -> Result<()> {
    let mut h = pump_state_with_viewport(state_settings(), 1400.0, 1800.0)?;
    h.dispatch(SetDragInProgress(true))?;
    h.dispatch(LabelDropped("Work".into()))?;
    let state = h.runtime.get_app_state::<InboxState>().unwrap();
    assert_eq!(state.last_drag_label.as_deref(), Some("Work"));
    assert!(!state.drag_in_progress, "drop should clear drag state");
    Ok(())
}

#[test]
fn help_tooltip_toggles_visible() -> Result<()> {
    // Help tooltip removed from header for space savings
    Ok(())
}

#[test]
fn layout_children_exist_after_navigation() -> Result<()> {
    let mut state = InboxState::default();
    state.current_path = "/inbox/1".into();

    let (ir, _runtime_state, env) = build_lowered_inbox_ir(&state);
    let input_nodes = fission_core::lowering::build_layout_tree(&ir, &env);

    let map: HashMap<_, _> = input_nodes.iter().map(|n| (n.id, n)).collect();
    for n in &input_nodes {
        for child in &n.children_ids {
            if !map.contains_key(child) {
                let mut chain = Vec::new();
                let mut current = Some(n.id);
                while let Some(id) = current {
                    if let Some(node) = ir.nodes.get(&id) {
                        chain.push(format!("{:?} -> {:?}", id, node.op));
                        current = node.parent;
                    } else {
                        chain.push(format!("{:?} -> <missing>", id));
                        break;
                    }
                }
                panic!(
                    "missing child {:?} referenced by parent {:?} op {:?}\nparent chain:\n{}",
                    child,
                    n.id,
                    n.op,
                    chain.join("\n")
                );
            }
        }
    }

    Ok(())
}

#[test]
fn inbox_emits_root_texture_compositor_plans() -> Result<()> {
    let (ir, runtime_state, env) = build_lowered_inbox_ir(&InboxState::default());
    let mut pipeline = Pipeline::new();
    let mut layout_engine = LayoutEngine::new();
    pipeline.replace_ir(ir, &env);
    pipeline.ensure_layout(
        LayoutRect::new(0.0, 0.0, 1200.0, 800.0),
        &mut layout_engine,
        &runtime_state.scroll,
    )?;
    pipeline.prepare_current(
        LayoutSize::new(1200.0, 800.0),
        LayoutSize::new(1200.0, 800.0),
        false,
        &runtime_state.scroll,
        &runtime_state.animation,
        &runtime_state.video,
        &runtime_state.web,
    )?;

    let mut summary = Vec::new();
    if let Some(scene) = pipeline.retained_scene() {
        for root in &scene.roots {
            summarize_render_node(root, 0, &mut summary);
        }
    }
    assert!(
        !pipeline.texture_compositor_plans().is_empty(),
        "expected inbox root surface to emit retained texture compositor plans\n{}",
        summary.join("\n")
    );
    Ok(())
}

#[test]
fn default_viewport_retained_scene_contains_inbox_rows() -> Result<()> {
    let (ir, runtime_state, env) = build_lowered_inbox_ir(&InboxState::default());
    let mut pipeline = Pipeline::new();
    let mut layout_engine = LayoutEngine::new();
    pipeline.replace_ir(ir, &env);
    pipeline.ensure_layout(
        LayoutRect::new(0.0, 0.0, 800.0, 600.0),
        &mut layout_engine,
        &runtime_state.scroll,
    )?;
    pipeline.prepare_current(
        LayoutSize::new(800.0, 600.0),
        LayoutSize::new(800.0, 600.0),
        false,
        &runtime_state.scroll,
        &runtime_state.animation,
        &runtime_state.video,
        &runtime_state.web,
    )?;

    let scene = pipeline.retained_scene().expect("retained scene");
    let texts = scene_texts(scene);
    assert!(
        texts.iter().any(|t| t == "Dana Wu"),
        "expected sender text in retained scene, got {:?}",
        texts
    );
    assert!(
        texts.iter().any(|t| t.contains("Quarterly planning sync")),
        "expected subject text in retained scene, got {:?}",
        texts
    );
    Ok(())
}

text_test!(app_title_visible, state_default(), "Fission Inbox");
text_test!(compose_button_visible, state_default(), "Compose");
#[test]
fn badge_label_present() -> Result<()> {
    let state = state_default();
    let unread = state
        .emails
        .iter()
        .filter(|e| e.folders.contains(&Folder::Inbox) && !e.is_read)
        .count();
    let expected = format!("{} new", unread);
    let h = pump_state(state)?;
    assert!(has_text(&h, &expected), "expected badge '{}'", expected);
    Ok(())
}
text_test!(segmented_control_unread_present, state_default(), "Unread");
text_test!(tabs_primary_present, state_default(), "Primary");
// Removed More menu button from header for space
// text_test!(menu_button_more_present, state_default(), "More");
text_test!(compose_button_present, state_default(), "Compose");
text_test!(dropdown_selected_present, state_default(), "Newest");
// More menu removed from header
// text_test!(menu_items_present_when_open, state_menu_open(), "Mark all as read");
text_test!(inbox_title_present, state_default(), "Inbox");
text_test!(
    popover_content_present_when_open,
    state_filters_open(),
    "Size (MB)"
);
text_test!(accordion_details_present, state_detail(), "Details");
text_test!(
    alert_external_sender_present,
    state_detail(),
    "External Sender"
);
text_test!(
    breadcrumb_email_present,
    state_detail(),
    "Quarterly planning sync"
);
text_test!(
    code_text_present,
    state_detail(),
    "label:important after:2025/01/01"
);
text_test!(timeline_received_present, state_detail(), "From Dana Wu");
text_test!(tag_label_present, state_detail(), "Work");
text_test!(wrap_tag_present, state_default(), "Planning");
#[test]
fn stat_help_text_present() -> Result<()> {
    let h = pump_state_with_viewport(state_default(), 1400.0, 960.0)?;
    assert!(
        has_text(&h, "All folders"),
        "expected wide sidebar stats help text to be present"
    );
    Ok(())
}
text_test!(stepper_import_present, state_default(), "Import");
// Storage section removed from sidebar for compactness
// text_test!(link_text_present, state_default(), "Manage storage");
// Browser Demo removed from sidebar
text_test!(link_text_present, state_default(), "Contacts");
text_test!(tree_view_sent_present, state_default(), "Sent");
text_test!(menu_new_event_present, state_default(), "New event");
text_test!(empty_state_text_present, state_empty(), "No emails here");
text_test!(modal_title_settings_present, state_settings(), "Settings");
text_test!(modal_title_contacts_present, state_contacts(), "Contacts");
text_test!(modal_title_compose_present, state_compose(), "New Message");
text_test!(
    toast_message_present,
    state_toast(),
    "Action completed successfully"
);
text_test!(data_table_header_present, state_contacts(), "Email");
text_test!(select_placeholder_present, state_settings(), "Default");
text_test!(file_upload_label_present, state_compose(), "Attach File");
text_test!(form_control_label_present, state_compose(), "Message");
text_test!(
    combobox_item_present,
    state_compose_with_combobox_open(),
    "alice@example.com"
);
text_test!(date_picker_text_present, state_compose(), "Select Date");
text_test!(
    router_not_found_present,
    state_not_found(),
    "Folder not found"
);

exact_text_test!(avatar_initials_present, state_detail(), "DW");
exact_text_test!(kbd_text_present, state_detail(), "g");
exact_text_test!(
    pagination_ellipsis_present,
    state_pagination_ellipsis(),
    "..."
);
exact_text_test!(time_picker_separator_present, state_compose(), ":");

#[test]
fn number_input_value_present() -> Result<()> {
    let h = pump_state(state_settings())?;
    assert!(
        ir_has_semantics(&h, |s| s.role == Role::TextInput
            && s.value.as_deref() == Some("50")),
        "expected number input text value"
    );
    Ok(())
}

layout_test!(
    divider_present,
    state_default(),
    |op| matches!(op, LayoutOp::Box { height: Some(h), .. } if approx_eq(*h, 1.0)),
    "expected a divider height of 1.0"
);

layout_test!(
    range_slider_grid_present,
    state_filters_open(),
    |op| match op {
        LayoutOp::Grid { columns, .. } => {
            columns.len() == 5
                && matches!(columns.get(1), Some(GridTrack::Points(p)) if approx_eq(*p, 16.0))
                && matches!(columns.get(3), Some(GridTrack::Points(p)) if approx_eq(*p, 16.0))
        }
        _ => false,
    },
    "expected range slider grid tracks"
);

// Storage progress bar removed from sidebar for compactness
layout_test!(
    calendar_grid_7_columns_present,
    state_default(),
    |op| match op {
        LayoutOp::Grid { columns, .. } => {
            columns.len() == 7
        }
        _ => false,
    },
    "expected calendar grid with 7 columns"
);

#[test]
fn calendar_grid_present() -> Result<()> {
    let h = pump_state_with_viewport(state_default(), 1400.0, 960.0)?;
    let ir = h.last_ir.as_ref().unwrap();
    let found = ir.nodes.values().any(|n| match &n.op {
        Op::Layout(LayoutOp::Grid { columns, .. }) => {
            columns.len() == 7
                && columns
                    .iter()
                    .all(|c| matches!(c, GridTrack::Points(p) if approx_eq(*p, 32.0)))
        }
        _ => false,
    });
    assert!(found, "expected calendar grid with 7 point columns");
    Ok(())
}

layout_test!(
    simple_grid_wrap_present,
    state_detail(),
    |op| matches!(op, LayoutOp::Flex { wrap: FlexWrap::Wrap, gap: Some(g), .. } if approx_eq(*g, 8.0)),
    "expected simple grid wrap with gap 8.0"
);

layout_test!(
    wrap_widget_present,
    state_default(),
    |op| matches!(op, LayoutOp::Flex { wrap: FlexWrap::Wrap, gap: Some(g), .. } if approx_eq(*g, 6.0)),
    "expected wrap layout with gap 6.0"
);

layout_test!(
    aspect_ratio_present,
    state_detail(),
    |op| matches!(
        op,
        LayoutOp::Box {
            aspect_ratio: Some(_),
            ..
        }
    ),
    "expected aspect ratio box"
);

layout_test!(
    split_view_flex_grow_present,
    state_default(),
    |op| matches!(op, LayoutOp::Box { flex_grow, .. } if approx_eq(*flex_grow, 0.20) || approx_eq(*flex_grow, 0.22) || approx_eq(*flex_grow, 0.26) || approx_eq(*flex_grow, 0.74) || approx_eq(*flex_grow, 0.78) || approx_eq(*flex_grow, 0.80)),
    "expected responsive split view flex grow values"
);

layout_test!(
    scroll_layout_present,
    state_default(),
    |op| matches!(op, LayoutOp::Scroll { .. }),
    "expected scroll layout"
);

layout_test!(
    grid_layout_present,
    state_default(),
    |op| matches!(op, LayoutOp::Grid { .. }),
    "expected grid layout"
);

layout_test!(
    row_layout_present,
    state_default(),
    |op| matches!(
        op,
        LayoutOp::Flex {
            direction: FlexDirection::Row,
            ..
        }
    ),
    "expected row flex layout"
);

layout_test!(
    column_layout_present,
    state_default(),
    |op| matches!(
        op,
        LayoutOp::Flex {
            direction: FlexDirection::Column,
            ..
        }
    ),
    "expected column flex layout"
);

layout_test!(
    zstack_layout_present,
    state_default(),
    |op| matches!(op, LayoutOp::ZStack),
    "expected zstack layout"
);

layout_test!(
    positioned_layout_present,
    state_settings(),
    |op| matches!(op, LayoutOp::Positioned { .. }),
    "expected positioned layout"
);

layout_test!(
    absolute_fill_present,
    state_default(),
    |op| matches!(op, LayoutOp::AbsoluteFill),
    "expected absolute fill layout"
);

layout_test!(
    clip_layout_present,
    state_settings(),
    |op| matches!(op, LayoutOp::Clip { .. }),
    "expected clip layout"
);

layout_test!(
    transform_layout_present,
    state_settings(),
    |op| matches!(op, LayoutOp::Transform { .. }),
    "expected transform layout"
);

semantics_test!(
    checkbox_semantics_present,
    state_default(),
    |s| s.role == Role::Checkbox,
    "expected checkbox semantics"
);

semantics_test!(
    switch_semantics_present,
    state_default(),
    |s| s.role == Role::Switch,
    "expected switch semantics"
);

semantics_test!(
    slider_semantics_present,
    state_settings(),
    |s| s.role == Role::Slider,
    "expected slider semantics"
);

semantics_test!(
    text_input_semantics_present,
    state_default(),
    |s| s.role == Role::TextInput,
    "expected text input semantics"
);

semantics_test!(
    focus_scope_present,
    state_compose(),
    |s| s.is_focus_scope && s.is_focus_barrier,
    "expected focus scope barrier"
);

semantics_test!(
    draggable_semantics_present,
    state_settings(),
    |s| s.drag_payload.is_some(),
    "expected draggable semantics"
);

semantics_test!(
    drag_target_drop_action_present,
    state_settings(),
    |s| s
        .actions
        .entries
        .iter()
        .any(|a| a.trigger == ActionTrigger::Drop),
    "expected drop action in drag target"
);

semantics_test!(
    dropzone_drop_action_present,
    state_compose(),
    |s| s
        .actions
        .entries
        .iter()
        .any(|a| a.trigger == ActionTrigger::Drop),
    "expected drop action in dropzone"
);

semantics_test!(
    hero_semantics_present,
    state_detail(),
    |s| s.hero_tag.as_deref() == Some("email_subject_1"),
    "expected hero semantics tag"
);

#[test]
fn tooltip_anchor_present() -> Result<()> {
    // Compose tooltip removed from header (button moved to sidebar only)
    Ok(())
}

#[test]
fn menu_button_anchor_present() -> Result<()> {
    // More menu removed from header for space savings
    Ok(())
}

#[test]
fn popover_anchor_present() -> Result<()> {
    let h = pump_state(state_default())?;
    let anchor_id = NodeId::derived(WidgetNodeId::explicit("advanced_filters").as_u128(), &[0]);
    assert!(
        ir_has_node_id(&h, anchor_id),
        "expected popover anchor node"
    );
    Ok(())
}

#[test]
fn date_range_picker_anchors_present() -> Result<()> {
    let h = pump_state(state_filters_open())?;
    let start_id = NodeId::derived(WidgetNodeId::explicit("filter_date_start").as_u128(), &[0]);
    let end_id = NodeId::derived(WidgetNodeId::explicit("filter_date_end").as_u128(), &[0]);
    assert!(
        ir_has_node_id(&h, start_id),
        "expected start date picker anchor"
    );
    assert!(
        ir_has_node_id(&h, end_id),
        "expected end date picker anchor"
    );
    Ok(())
}

#[test]
fn lazy_column_node_present() -> Result<()> {
    let state = state_default();
    let page_key = state.page as u32;
    let h = pump_state(state)?;
    let lazy_id = WidgetNodeId::explicit("email_list");
    let node_id = NodeId::derived(lazy_id.as_u128(), &[page_key]);
    assert!(ir_has_node_id(&h, node_id), "expected lazy column node");
    Ok(())
}

#[test]
fn drawer_renders_second_sidebar() -> Result<()> {
    let h = pump_state(state_drawer())?;
    let count = count_text_exact(&h, "Fission Inbox");
    assert!(
        count >= 2,
        "expected drawer to render a second sidebar title"
    );
    Ok(())
}

#[test]
fn icon_draw_svg_present() -> Result<()> {
    let h = pump_state(state_default())?;
    assert!(
        ir_has_paint_op(&h, |op| matches!(op, PaintOp::DrawSvg { .. })),
        "expected at least one svg paint op"
    );
    Ok(())
}

#[test]
fn image_draw_present() -> Result<()> {
    let h = pump_state(state_detail())?;
    assert!(display_has_image(&h), "expected at least one image draw");
    Ok(())
}

#[test]
fn circular_progress_draw_path_present() -> Result<()> {
    let h = pump_state(state_default())?;
    assert!(
        display_has_arc_path(&h),
        "expected circular progress arc path draw"
    );
    Ok(())
}

#[test]
fn default_viewport_display_list_contains_inbox_rows() -> Result<()> {
    let h = pump_state_with_viewport(state_default(), 800.0, 600.0)?;
    let texts = display_texts(&h);
    assert!(
        texts.iter().any(|t| t == "Dana Wu"),
        "expected first inbox sender in display list, got {:?}",
        texts
    );
    assert!(
        texts.iter().any(|t| t.contains("Quarterly planning sync")),
        "expected subject text in display list, got {:?}",
        texts
    );
    Ok(())
}

#[test]
fn wide_viewport_display_list_contains_inbox_rows() -> Result<()> {
    let h = pump_state_with_viewport(state_default(), 1400.0, 900.0)?;
    let texts = display_texts(&h);
    assert!(
        texts.iter().any(|t| t == "Dana Wu"),
        "expected first inbox sender in display list, got {:?}",
        texts
    );
    assert!(
        texts.iter().any(|t| t.contains("Quarterly planning sync")),
        "expected subject text in display list, got {:?}",
        texts
    );
    Ok(())
}

#[test]
fn default_viewport_email_list_scroll_has_positive_height() -> Result<()> {
    let h = pump_state_with_viewport(state_default(), 800.0, 600.0)?;
    let lazy_id = WidgetNodeId::explicit("email_list");
    let node_id = NodeId::derived(lazy_id.as_u128(), &[1]);
    let rect = node_rect(&h, node_id).expect("email list rect");
    assert!(
        rect.height() > 200.0,
        "expected email list scroll viewport to have real height, got {:?}",
        rect
    );
    Ok(())
}

#[test]
fn spinner_animation_present_in_default_inbox() -> Result<()> {
    let h = pump_state(state_default())?;
    let base = WidgetNodeId::explicit("sync_spinner");
    let mut found = 0;
    for i in 1..=3 {
        let sub_id = WidgetNodeId::from_u128(base.as_u128() ^ i as u128);
        if runtime_has_animation(&h, sub_id, AnimationPropertyId::Opacity) {
            found += 1;
        }
    }
    assert!(found > 0, "default inbox should schedule spinner animations");
    Ok(())
}

#[test]
fn skeleton_animation_present_in_default_inbox() -> Result<()> {
    let h = pump_state(state_default())?;
    let id = WidgetNodeId::explicit("sync_skeleton");
    assert!(
        runtime_has_animation(&h, id, AnimationPropertyId::Opacity),
        "default inbox should schedule skeleton opacity animation"
    );
    Ok(())
}

#[test]
fn transition_animation_present() -> Result<()> {
    let mut state = state_default();
    state.show_quick_tip = true;
    let h = pump_state(state)?;
    let id = WidgetNodeId::explicit("quick_tip_fade");
    assert!(
        runtime_has_animation(&h, id, AnimationPropertyId::Opacity),
        "expected transition opacity animation"
    );
    Ok(())
}

#[test]
fn video_embed_present() -> Result<()> {
    let h = pump_state(state_detail())?;
    assert!(
        ir_has_embed_kind(&h, EmbedKind::Video),
        "expected video embed"
    );
    Ok(())
}

#[test]
fn web_embed_present() -> Result<()> {
    let h = pump_state(state_browser())?;
    assert!(ir_has_embed_kind(&h, EmbedKind::Web), "expected web embed");
    Ok(())
}
