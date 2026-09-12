//! Keys an application declares on a node, rather than keys a role implies.
//!
//! Fission's built-in contracts cover the keys a role implies. These tests cover
//! the ones only the application knows about: a shortcut that opens a palette, a
//! scoped key that a whole dialog responds to, a node that wants Enter for
//! something other than activation.

use fission_core::event::{InputEvent, KeyEvent};
use fission_core::event::{MOD_CTRL, MOD_SHIFT};
use fission_core::{ActionEnvelope, ActionId, GlobalState, KeyCode, Runtime};
use fission_ir::semantics::{ActionEntry, ActionTrigger, KeyAction};
use fission_ir::{
    ActionSet, CompositeStyle, CoreIR, CoreNode, Op, Role, Semantics, StructuralOp, WidgetId,
};
use fission_layout::{LayoutSize, LayoutSnapshot};

#[derive(Debug, Default)]
struct Log {
    palette: usize,
    scoped: usize,
    activated: usize,
}

impl GlobalState for Log {}

fn palette_id() -> ActionId {
    ActionId::from_name("declared_key_actions::Palette")
}

fn scoped_id() -> ActionId {
    ActionId::from_name("declared_key_actions::Scoped")
}

fn activate_id() -> ActionId {
    ActionId::from_name("declared_key_actions::Activate")
}

fn node(ir: &mut CoreIR, id: WidgetId, op: Op, children: Vec<WidgetId>, parent: Option<WidgetId>) {
    for child in &children {
        if let Some(existing) = ir.nodes.get_mut(child) {
            existing.parent = Some(id);
        }
    }
    ir.nodes.insert(
        id,
        CoreNode {
            id,
            op,
            composite: CompositeStyle::default(),
            children,
            parent,
            hash: 0,
        },
    );
}

/// A focused leaf inside a container, each able to declare key actions.
fn tree(leaf: Semantics, container: Option<Semantics>) -> (Runtime, CoreIR) {
    let leaf_id = WidgetId::explicit("leaf");
    let container_id = WidgetId::explicit("container");
    let mut ir = CoreIR::default();
    node(&mut ir, leaf_id, Op::Semantics(leaf), Vec::new(), None);
    let container_op = match container {
        Some(semantics) => Op::Semantics(semantics),
        None => Op::Structural(StructuralOp::Group { stable_hash: 1 }),
    };
    node(&mut ir, container_id, container_op, vec![leaf_id], None);
    ir.root = Some(container_id);

    let mut runtime = Runtime::default();
    runtime.add_app_state(Box::new(Log::default())).unwrap();
    runtime
        .register_reducer::<Log>(
            palette_id(),
            |state: &mut Log, _: &ActionEnvelope, _: WidgetId| {
                state.palette += 1;
                Ok(())
            },
        )
        .unwrap();
    runtime
        .register_reducer::<Log>(
            scoped_id(),
            |state: &mut Log, _: &ActionEnvelope, _: WidgetId| {
                state.scoped += 1;
                Ok(())
            },
        )
        .unwrap();
    runtime
        .register_reducer::<Log>(
            activate_id(),
            |state: &mut Log, _: &ActionEnvelope, _: WidgetId| {
                state.activated += 1;
                Ok(())
            },
        )
        .unwrap();
    runtime.runtime_state.interaction.set_focused(Some(leaf_id));
    (runtime, ir)
}

fn press(runtime: &mut Runtime, ir: &CoreIR, key_code: KeyCode, modifiers: u8) {
    runtime
        .handle_input(
            InputEvent::Keyboard(KeyEvent::Down {
                key_code,
                modifiers,
            }),
            ir,
            &LayoutSnapshot::new(LayoutSize::new(640.0, 480.0)),
        )
        .unwrap();
}

fn log(runtime: &Runtime) -> &Log {
    runtime.get_app_state::<Log>().expect("log state")
}

#[test]
fn declared_shortcut_dispatches_its_action() {
    let (mut runtime, ir) = tree(
        Semantics {
            role: Role::Button,
            focusable: true,
            key_actions: vec![KeyAction::with_modifiers(
                KeyCode::Char('k'),
                MOD_CTRL,
                palette_id().as_u128(),
            )],
            ..Default::default()
        },
        None,
    );

    press(&mut runtime, &ir, KeyCode::Char('k'), MOD_CTRL);
    assert_eq!(log(&runtime).palette, 1);
}

#[test]
fn modifiers_must_match_exactly() {
    let (mut runtime, ir) = tree(
        Semantics {
            role: Role::Button,
            focusable: true,
            key_actions: vec![KeyAction::with_modifiers(
                KeyCode::Char('k'),
                MOD_CTRL,
                palette_id().as_u128(),
            )],
            ..Default::default()
        },
        None,
    );

    // A binding for Ctrl+K is not a binding for K, and not one for
    // Ctrl+Shift+K either. Anything looser would make it impossible to bind two
    // shortcuts on the same key.
    press(&mut runtime, &ir, KeyCode::Char('k'), 0);
    press(&mut runtime, &ir, KeyCode::Char('k'), MOD_CTRL | MOD_SHIFT);
    assert_eq!(log(&runtime).palette, 0);
}

#[test]
fn a_binding_on_an_ancestor_covers_its_whole_subtree() {
    let (mut runtime, ir) = tree(
        Semantics {
            role: Role::Button,
            focusable: true,
            ..Default::default()
        },
        Some(Semantics {
            role: Role::Dialog,
            key_actions: vec![KeyAction::new(KeyCode::Char('s'), scoped_id().as_u128())],
            ..Default::default()
        }),
    );

    press(&mut runtime, &ir, KeyCode::Char('s'), 0);
    assert_eq!(
        log(&runtime).scoped,
        1,
        "a dialog binding a key must cover the focused control inside it"
    );
}

#[test]
fn the_innermost_binding_wins() {
    let (mut runtime, ir) = tree(
        Semantics {
            role: Role::Button,
            focusable: true,
            key_actions: vec![KeyAction::new(KeyCode::Char('s'), palette_id().as_u128())],
            ..Default::default()
        },
        Some(Semantics {
            role: Role::Dialog,
            key_actions: vec![KeyAction::new(KeyCode::Char('s'), scoped_id().as_u128())],
            ..Default::default()
        }),
    );

    press(&mut runtime, &ir, KeyCode::Char('s'), 0);
    assert_eq!(log(&runtime).palette, 1);
    assert_eq!(
        log(&runtime).scoped,
        0,
        "the ancestor scope must not also fire once a descendant has claimed the key"
    );
}

#[test]
fn a_declared_key_wins_over_built_in_activation() {
    let (mut runtime, ir) = tree(
        Semantics {
            role: Role::Button,
            focusable: true,
            actions: ActionSet {
                entries: vec![ActionEntry {
                    trigger: ActionTrigger::Default,
                    action_id: activate_id().as_u128(),
                    payload_data: None,
                }],
            },
            key_actions: vec![KeyAction::new(KeyCode::Enter, palette_id().as_u128())],
            ..Default::default()
        },
        None,
    );

    press(&mut runtime, &ir, KeyCode::Enter, 0);
    assert_eq!(log(&runtime).palette, 1);
    assert_eq!(
        log(&runtime).activated,
        0,
        "a node that explicitly asked for Enter meant it"
    );

    // Space is untouched, so the role's own contract still applies.
    press(&mut runtime, &ir, KeyCode::Space, 0);
    assert_eq!(log(&runtime).activated, 1);
}

#[test]
fn a_disabled_node_ignores_its_declared_keys() {
    let (mut runtime, ir) = tree(
        Semantics {
            role: Role::Button,
            focusable: true,
            disabled: true,
            key_actions: vec![KeyAction::new(KeyCode::Char('s'), palette_id().as_u128())],
            ..Default::default()
        },
        None,
    );

    press(&mut runtime, &ir, KeyCode::Char('s'), 0);
    assert_eq!(log(&runtime).palette, 0);
}

#[test]
fn tab_stays_with_focus_traversal() {
    let (mut runtime, ir) = tree(
        Semantics {
            role: Role::Button,
            focusable: true,
            key_actions: vec![KeyAction::new(KeyCode::Tab, palette_id().as_u128())],
            ..Default::default()
        },
        None,
    );

    press(&mut runtime, &ir, KeyCode::Tab, 0);
    assert_eq!(
        log(&runtime).palette,
        0,
        "letting a widget claim Tab would let it trap focus for the whole app"
    );
}
