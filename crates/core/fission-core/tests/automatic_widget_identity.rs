use std::ops::ControlFlow;

use fission_core::internal::{self, BuildCtx};
use fission_core::ui::widgets::text::InlineWidgetSpan;
use fission_core::ui::{Container, Overlay, RichText, Scroll, TextContent, TextInput, ZStack};
use fission_core::{
    build, reduce, widgets, ActionEnvelope, ActionScope, ActionScopeId, Button, Column, Env,
    Runtime, Text, View, Widget, WidgetId, WidgetIdExt,
};
use fission_ir::op::Color;
use fission_ir::Op;
use fission_macros::{fission_component, fission_reducer};

fn resolved(widget: Widget) -> Widget {
    internal::resolve_widget_identities(&widget, WidgetId::app_root())
}

fn identity_snapshot(widget: &Widget) -> Vec<(&'static str, WidgetId)> {
    let mut snapshot = Vec::new();
    let _ = widget.visit(&mut |node| {
        let id = internal::widget_id(node).expect("every built-in widget should be identified");
        snapshot.push((internal::widget_kind_name(node), id));
        ControlFlow::Continue(())
    });
    snapshot
}

fn text_identities(widget: &Widget) -> Vec<(String, WidgetId)> {
    let mut identities = Vec::new();
    let _ = widget.visit(&mut |node| {
        if let Some(text) = internal::widget_as_text(node) {
            if let TextContent::Literal(value) = &text.content {
                identities.push((
                    value.clone(),
                    internal::widget_id(node).expect("resolved text should have an id"),
                ));
            }
        }
        ControlFlow::Continue(())
    });
    identities
}

fn typical_developer_tree(decorated: bool) -> Widget {
    let background = decorated.then_some(Color {
        r: 20,
        g: 30,
        b: 40,
        a: 255,
    });

    Column {
        children: widgets![
            Text::new("Dashboard"),
            Container {
                child: Some(Text::new("Summary").into()),
                background_color: background,
                ..Default::default()
            },
            Scroll {
                child: Some(
                    Column {
                        children: vec![
                            Text::new("First row").into(),
                            Text::new("Second row").into(),
                        ],
                        ..Default::default()
                    }
                    .into(),
                ),
                ..Default::default()
            },
            TextInput {
                value: "search".into(),
                ..Default::default()
            },
            Button {
                child: Some(Text::new("Continue").into()),
                ..Default::default()
            }
            .id(WidgetId::explicit("developer-randomly-identified-button")),
        ],
        ..Default::default()
    }
    .into()
}

#[test]
fn identical_developer_trees_receive_identical_complete_identity_snapshots() {
    let first = resolved(typical_developer_tree(false));
    let second = resolved(typical_developer_tree(false));

    let first_snapshot = identity_snapshot(&first);
    let second_snapshot = identity_snapshot(&second);

    assert_eq!(first_snapshot, second_snapshot);
    let unique = first_snapshot
        .iter()
        .map(|(_, id)| *id)
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(unique.len(), first_snapshot.len());
}

#[test]
fn embedded_widget_surfaces_receive_automatic_identities() {
    let tree = resolved(
        Column {
            children: vec![
                TextInput {
                    prefix: Some(Text::new("prefix").into()),
                    suffix: Some(Text::new("suffix").into()),
                    ..Default::default()
                }
                .into(),
                RichText {
                    inline_widgets: vec![InlineWidgetSpan::new(Text::new("inline"), 20.0, 20.0)],
                    ..Default::default()
                }
                .into(),
            ],
            ..Default::default()
        }
        .into(),
    );

    let embedded = text_identities(&tree);
    assert_eq!(
        embedded
            .iter()
            .map(|(text, _)| text.as_str())
            .collect::<Vec<_>>(),
        vec!["prefix", "suffix", "inline"]
    );
    let unique = embedded
        .iter()
        .map(|(_, id)| *id)
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(unique.len(), embedded.len());
}

#[test]
fn optional_sibling_slots_do_not_renumber_present_widgets() {
    let with_prefix = resolved(
        TextInput {
            prefix: Some(Text::new("prefix").into()),
            suffix: Some(Text::new("suffix").into()),
            ..Default::default()
        }
        .into(),
    );
    let without_prefix = resolved(
        TextInput {
            suffix: Some(Text::new("suffix").into()),
            ..Default::default()
        }
        .into(),
    );

    let suffix_with_prefix = text_identities(&with_prefix)
        .into_iter()
        .find(|(label, _)| label == "suffix")
        .unwrap();
    let suffix_without_prefix = text_identities(&without_prefix)
        .into_iter()
        .find(|(label, _)| label == "suffix")
        .unwrap();
    assert_eq!(suffix_with_prefix.1, suffix_without_prefix.1);
}

#[test]
fn internal_decoration_does_not_change_authored_widget_identities() {
    let plain = resolved(typical_developer_tree(false));
    let decorated = resolved(typical_developer_tree(true));

    assert_eq!(identity_snapshot(&plain), identity_snapshot(&decorated));

    let plain_ir = internal::lower_widget_to_ir(&plain);
    let decorated_ir = internal::lower_widget_to_ir(&decorated);
    for (_, id) in identity_snapshot(&plain) {
        assert!(plain_ir.nodes.contains_key(&id));
        assert!(decorated_ir.nodes.contains_key(&id));
    }
}

#[test]
fn shell_overlay_has_a_separate_root_without_reidentifying_the_authored_tree() {
    let authored_root = WidgetId::explicit("authored-app-root");
    let shell_root = internal::shell_root_id(authored_root);
    let authored = internal::resolve_widget_identities(
        &Column {
            children: vec![Text::new("content").into()],
            ..Default::default()
        }
        .into(),
        authored_root,
    );
    let shell: Widget = Overlay {
        content: authored,
        overlay: ZStack::default().into(),
        ..Default::default()
    }
    .into();
    let ir = internal::lower_widget_to_ir_with_root(&shell, shell_root);

    assert_eq!(ir.root, Some(shell_root));
    assert!(ir.nodes.contains_key(&authored_root));
    assert_ne!(shell_root, authored_root);
}

#[test]
fn identical_identity_trees_produce_an_empty_frame_diff() {
    let first = internal::lower_widget_to_ir(&typical_developer_tree(false));
    let second = internal::lower_widget_to_ir(&typical_developer_tree(false));
    let diff = fission_core::diff::diff_ir(&first, &second);

    assert!(diff.dirty_layout.is_empty());
    assert!(diff.dirty_paint.is_empty());
    assert!(diff.dirty_composite.is_empty());
}

#[test]
fn changing_one_leaf_does_not_change_an_unrelated_sibling_identity() {
    let before = resolved(
        Column {
            children: vec![Text::new("before").into(), Text::new("stable").into()],
            ..Default::default()
        }
        .into(),
    );
    let after = resolved(
        Column {
            children: vec![Text::new("after").into(), Text::new("stable").into()],
            ..Default::default()
        }
        .into(),
    );

    let before_text = text_identities(&before);
    let after_text = text_identities(&after);
    assert_eq!(before_text[0].1, after_text[0].1);
    assert_eq!(before_text[1], after_text[1]);

    let before_ir = internal::lower_widget_to_ir(&before);
    let after_ir = internal::lower_widget_to_ir(&after);
    let diff = fission_core::diff::diff_ir(&before_ir, &after_ir);
    assert!(!diff.dirty_layout.contains(&before_text[1].1));
    assert!(!diff.dirty_paint.contains(&before_text[1].1));
}

#[test]
fn resolved_widget_identity_survives_serialization() {
    let tree = resolved(typical_developer_tree(false));
    let encoded = serde_json::to_vec(&tree).expect("resolved widget tree should serialize");
    let decoded: Widget =
        serde_json::from_slice(&encoded).expect("resolved widget tree should deserialize");
    assert_eq!(identity_snapshot(&tree), identity_snapshot(&decoded));
}

#[test]
fn duplicate_manual_ids_are_rejected() {
    let duplicate = WidgetId::explicit("duplicate-manual-id");
    let tree: Widget = Column {
        children: vec![
            Text::new("first").id(duplicate),
            Text::new("second").id(duplicate),
        ],
        ..Default::default()
    }
    .into();

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        internal::lower_widget_to_ir(&tree)
    }));
    assert!(result.is_err());
}

#[test]
fn explicit_identity_is_preserved_while_automatic_siblings_are_generated() {
    let explicit = WidgetId::explicit("only-widget-the-developer-cared-about");
    let tree = resolved(
        Column {
            children: vec![
                Text::new("automatic-before").into(),
                Text::new("manual").id(explicit),
                Text::new("automatic-after").into(),
            ],
            ..Default::default()
        }
        .into(),
    );

    let identities = text_identities(&tree);
    assert_eq!(identities[1], ("manual".to_string(), explicit));
    assert_ne!(identities[0].1, explicit);
    assert_ne!(identities[2].1, explicit);
    assert_ne!(identities[0].1, identities[2].1);
}

#[test]
fn root_identity_namespaces_automatic_ids_but_not_explicit_ids() {
    let explicit = WidgetId::explicit("portable-explicit-child");
    let widget: Widget = Column {
        children: vec![
            Text::new("automatic").into(),
            Text::new("explicit").id(explicit),
        ],
        ..Default::default()
    }
    .into();

    let first = internal::resolve_widget_identities(&widget, WidgetId::explicit("mount-a"));
    let second = internal::resolve_widget_identities(&widget, WidgetId::explicit("mount-b"));
    let first_text = text_identities(&first);
    let second_text = text_identities(&second);

    assert_ne!(first_text[0].1, second_text[0].1);
    assert_eq!(first_text[1].1, explicit);
    assert_eq!(second_text[1].1, explicit);
}

#[test]
fn configured_root_is_the_authored_root_widget_identity() {
    let root = WidgetId::explicit("developer-chosen-app-root");
    let widget = internal::resolve_widget_identities(&Text::new("root").into(), root);

    assert_eq!(internal::widget_id(&widget), Some(root));
}

#[test]
fn explicit_root_widget_identity_overrides_the_app_namespace() {
    let explicit = WidgetId::explicit("root-widget-explicit-id");
    let widget = Text::new("root").id(explicit);
    let resolved = internal::resolve_widget_identities(
        &widget,
        WidgetId::explicit("configured-app-namespace"),
    );

    assert_eq!(internal::widget_id(&resolved), Some(explicit));
}

#[test]
fn explicit_subtree_identity_keeps_automatic_descendants_stable_when_reordered() {
    fn tree(order: &[&str]) -> Widget {
        Column {
            children: order
                .iter()
                .map(|label| {
                    Container::new(Text::new(format!("child-of-{label}")))
                        .id(WidgetId::explicit(&format!("subtree.{label}")))
                })
                .collect(),
            ..Default::default()
        }
        .into()
    }

    let before = text_identities(&resolved(tree(&["a", "b"])));
    let after = text_identities(&resolved(tree(&["b", "a"])));

    for (label, id) in before {
        let after_id = after
            .iter()
            .find_map(|(candidate, candidate_id)| (candidate == &label).then_some(*candidate_id))
            .unwrap();
        assert_eq!(id, after_id);
    }
}

#[test]
fn automatic_subtree_identity_and_descendants_follow_the_structural_position() {
    fn tree(order: &[&str]) -> Widget {
        Column {
            children: order
                .iter()
                .map(|label| Container::new(Text::new(format!("child-of-{label}"))).into())
                .collect(),
            ..Default::default()
        }
        .into()
    }

    let before = text_identities(&resolved(tree(&["a", "b"])));
    let after = text_identities(&resolved(tree(&["b", "a"])));

    assert_eq!(before[0].1, after[0].1);
    assert_eq!(before[1].1, after[1].1);
    assert_ne!(before[0].0, after[0].0);
}

#[test]
fn explicit_identity_stabilizes_internal_nodes_of_wrapper_widgets() {
    let scope = ActionScopeId::from_name("stable-action-scope");
    let explicit = WidgetId::explicit("stable-action-scope-widget");
    let make_tree = |scope_first: bool| {
        let scoped = ActionScope::new(scope, Text::new("scoped child")).id(explicit);
        let sibling: Widget = Text::new("ordinary sibling").into();
        Column {
            children: if scope_first {
                vec![scoped, sibling]
            } else {
                vec![sibling, scoped]
            },
            ..Default::default()
        }
        .into()
    };
    let scope_node = |ir: &fission_ir::CoreIR| {
        ir.nodes
            .iter()
            .find_map(|(id, node)| match &node.op {
                Op::Semantics(semantics) if semantics.action_scope_id == Some(scope.as_u128()) => {
                    Some(*id)
                }
                _ => None,
            })
            .expect("action scope semantics node should be lowered")
    };

    let before = internal::lower_widget_to_ir(&make_tree(true));
    let after = internal::lower_widget_to_ir(&make_tree(false));
    assert_eq!(scope_node(&before), scope_node(&after));
}

#[test]
fn widget_kind_participates_in_automatic_identity() {
    let text = resolved(
        Column {
            children: vec![Text::new("same-slot").into()],
            ..Default::default()
        }
        .into(),
    );
    let container = resolved(
        Column {
            children: vec![Container::new(Text::new("same-slot")).into()],
            ..Default::default()
        }
        .into(),
    );

    let text_child = internal::widget_as_column(&text).unwrap().children[0].clone();
    let container_child = internal::widget_as_column(&container).unwrap().children[0].clone();
    assert_ne!(
        internal::widget_id(&text_child),
        internal::widget_id(&container_child)
    );
}

fn unkeyed_text_collection(labels: &[&str]) -> Widget {
    Column {
        children: labels
            .iter()
            .map(|label| Text::new(*label).into())
            .collect(),
        ..Default::default()
    }
    .into()
}

fn identified_text_collection(labels: &[&str]) -> Widget {
    Column {
        children: labels
            .iter()
            .map(|label| Text::new(*label).id(WidgetId::explicit(&format!("item.{label}"))))
            .collect(),
        ..Default::default()
    }
    .into()
}

#[test]
fn unkeyed_collection_identity_is_stable_and_position_based() {
    let original = text_identities(&resolved(unkeyed_text_collection(&["a", "b", "c"])));
    let rebuilt = text_identities(&resolved(unkeyed_text_collection(&["a", "b", "c"])));
    let reordered = text_identities(&resolved(unkeyed_text_collection(&["c", "b", "a"])));

    assert_eq!(original, rebuilt);
    assert_eq!(
        original.iter().map(|(_, id)| *id).collect::<Vec<_>>(),
        reordered.iter().map(|(_, id)| *id).collect::<Vec<_>>()
    );
    assert_eq!(reordered[0].0, "c");
    assert_eq!(reordered[0].1, original[0].1);
}

#[test]
fn explicitly_identified_collection_items_follow_data_across_reorder() {
    let original = text_identities(&resolved(identified_text_collection(&["a", "b", "c"])));
    let reordered = text_identities(&resolved(identified_text_collection(&["c", "b", "a"])));

    for (label, id) in original {
        let reordered_id = reordered
            .iter()
            .find_map(|(candidate, candidate_id)| (candidate == &label).then_some(*candidate_id))
            .expect("the same logical item should remain present");
        assert_eq!(id, reordered_id);
    }
}

#[test]
fn inserted_unkeyed_items_keep_identity_with_positions() {
    let original = text_identities(&resolved(unkeyed_text_collection(&["a", "b"])));
    let inserted = text_identities(&resolved(unkeyed_text_collection(&["new", "a", "b"])));

    assert_eq!(inserted[0].1, original[0].1);
    assert_eq!(inserted[1].1, original[1].1);
    assert_ne!(inserted[2].1, original[1].1);
}

#[fission_reducer(IncrementCounter)]
fn increment_counter(value: &mut u32) {
    *value += 1;
}

#[fission_component]
struct StatefulItem {
    label: String,

    #[local_state(default = 0)]
    value: u32,
}

impl From<StatefulItem> for Widget {
    fn from(item: StatefulItem) -> Self {
        let (ctx, _) = build::current::<()>();
        let value = item.value();
        let increment = ctx.bind_local(IncrementCounter, value.clone(), reduce!(increment_counter));
        Column {
            children: widgets![
                Text::new(item.label),
                Text::new(format!("value={}", value.get())),
                Button {
                    child: Some(Text::new("increment").into()),
                    on_press: Some(increment),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }
        .into()
    }
}

fn build_stateful_collection(
    runtime: &Runtime,
    labels: &[&str],
    explicitly_identified: &[&str],
    root: WidgetId,
) -> (Widget, BuildCtx<()>) {
    let env = Env::default();
    let view = View::new(&(), &runtime.runtime_state, &env, None);
    let mut context = BuildCtx::new();
    let widget = build::enter_with_root(&mut context, &view, root, || {
        Column {
            children: labels
                .iter()
                .map(|label| {
                    let item = StatefulItem {
                        label: (*label).to_string(),
                    };
                    if explicitly_identified.contains(label) {
                        item.id(WidgetId::explicit(&format!("stateful-item.{label}")))
                    } else {
                        item.into()
                    }
                })
                .collect(),
            ..Default::default()
        }
        .into()
    });
    (widget, context)
}

fn button_actions(widget: &Widget) -> Vec<ActionEnvelope> {
    let mut actions = Vec::new();
    let _ = widget.visit(&mut |node| {
        if let Some(action) =
            internal::widget_as_button(node).and_then(|button| button.on_press.clone())
        {
            actions.push(action);
        }
        ControlFlow::Continue(())
    });
    actions
}

fn item_values(widget: &Widget) -> Vec<(String, u32)> {
    let texts = text_identities(&resolved(widget.clone()))
        .into_iter()
        .map(|(text, _)| text)
        .collect::<Vec<_>>();
    texts
        .chunks(3)
        .map(|chunk| {
            let value = chunk[1].strip_prefix("value=").unwrap().parse().unwrap();
            (chunk[0].clone(), value)
        })
        .collect()
}

fn dispatch_action(runtime: &mut Runtime, context: BuildCtx<()>, action: ActionEnvelope) {
    runtime.clear_reducers();
    runtime.absorb_registry(context.registry);
    runtime
        .dispatch(action, WidgetId::from_u128(0))
        .expect("local-state action should dispatch");
}

#[test]
fn local_state_in_unidentified_collections_is_stable_by_position() {
    let mut runtime = Runtime::default();
    let root = WidgetId::explicit("unkeyed-state-root");
    let (widget, context) = build_stateful_collection(&runtime, &["a", "b"], &[], root);
    let second = button_actions(&widget).remove(1);
    dispatch_action(&mut runtime, context, second);

    let (same_order, _) = build_stateful_collection(&runtime, &["a", "b"], &[], root);
    assert_eq!(
        item_values(&same_order),
        vec![("a".into(), 0), ("b".into(), 1)]
    );

    let (reordered, _) = build_stateful_collection(&runtime, &["b", "a"], &[], root);
    assert_eq!(
        item_values(&reordered),
        vec![("b".into(), 0), ("a".into(), 1)]
    );
}

#[test]
fn local_state_in_explicitly_identified_collections_follows_the_item() {
    let mut runtime = Runtime::default();
    let root = WidgetId::explicit("identified-state-root");
    let identified = ["a", "b"];
    let (widget, context) = build_stateful_collection(&runtime, &["a", "b"], &identified, root);
    let second = button_actions(&widget).remove(1);
    dispatch_action(&mut runtime, context, second);

    let (reordered, _) = build_stateful_collection(&runtime, &["b", "a"], &identified, root);
    assert_eq!(
        item_values(&reordered),
        vec![("b".into(), 1), ("a".into(), 0)]
    );
}

#[test]
fn one_randomly_identified_collection_item_coexists_with_automatic_items() {
    let mut runtime = Runtime::default();
    let root = WidgetId::explicit("mixed-state-root");
    let (widget, context) = build_stateful_collection(&runtime, &["a", "b", "c"], &["b"], root);
    let middle = button_actions(&widget).remove(1);
    dispatch_action(&mut runtime, context, middle);

    let (reordered, _) = build_stateful_collection(&runtime, &["b", "a", "c"], &["b"], root);
    assert_eq!(
        item_values(&reordered),
        vec![("b".into(), 1), ("a".into(), 0), ("c".into(), 0)]
    );
}

#[test]
fn automatic_local_state_remains_position_based_around_a_random_explicit_item() {
    let mut runtime = Runtime::default();
    let root = WidgetId::explicit("mixed-positional-state-root");
    let (widget, context) = build_stateful_collection(&runtime, &["a", "b", "c"], &["b"], root);
    let actions = button_actions(&widget);
    dispatch_action(&mut runtime, context, actions[0].clone());

    let (widget, context) = build_stateful_collection(&runtime, &["a", "b", "c"], &["b"], root);
    let actions = button_actions(&widget);
    dispatch_action(&mut runtime, context, actions[2].clone());

    let (reordered, _) = build_stateful_collection(&runtime, &["b", "a", "c"], &["b"], root);
    assert_eq!(
        item_values(&reordered),
        vec![("b".into(), 0), ("a".into(), 0), ("c".into(), 1)]
    );
}

#[test]
fn changing_the_app_root_resets_automatic_local_state_namespace() {
    let mut runtime = Runtime::default();
    let root_a = WidgetId::explicit("local-state-mount-a");
    let root_b = WidgetId::explicit("local-state-mount-b");
    let (widget, context) = build_stateful_collection(&runtime, &["a"], &[], root_a);
    let action = button_actions(&widget).remove(0);
    dispatch_action(&mut runtime, context, action);

    let (same_root, _) = build_stateful_collection(&runtime, &["a"], &[], root_a);
    assert_eq!(item_values(&same_root), vec![("a".into(), 1)]);

    let (other_root, _) = build_stateful_collection(&runtime, &["a"], &[], root_b);
    assert_eq!(item_values(&other_root), vec![("a".into(), 0)]);
}

#[test]
fn removing_an_item_prunes_its_local_state_before_reinsertion() {
    let mut runtime = Runtime::default();
    let root = WidgetId::explicit("pruned-item-state-root");
    let identified = ["a", "b"];
    let (widget, context) = build_stateful_collection(&runtime, &["a", "b"], &identified, root);
    let second = button_actions(&widget).remove(1);
    dispatch_action(&mut runtime, context, second);

    let (without_b, _) = build_stateful_collection(&runtime, &["a"], &identified, root);
    assert_eq!(item_values(&without_b), vec![("a".into(), 0)]);

    let (reinserted, _) = build_stateful_collection(&runtime, &["a", "b"], &identified, root);
    assert_eq!(
        item_values(&reinserted),
        vec![("a".into(), 0), ("b".into(), 0)]
    );
}
