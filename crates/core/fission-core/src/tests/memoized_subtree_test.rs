use crate::authoring::BuildCtx;
use crate::build;
use crate::ui::{Button, SemanticsRegion, Text, Widget, WidgetKind};
use crate::{Action, ActionEnvelope, ActionId, Env, GlobalState, Runtime, View, WidgetId};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Default)]
struct MemoTestState;
impl GlobalState for MemoTestState {}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct IncrementLocal;

impl Action for IncrementLocal {
    fn static_id() -> ActionId {
        ActionId::from_name("memoized_subtree_test::IncrementLocal")
    }
}

#[derive(Debug, Clone)]
struct LocalCount(usize);
impl GlobalState for LocalCount {}

fn increment_local<'a, 'b, 'c>(
    count: &mut LocalCount,
    _: IncrementLocal,
    _: &mut crate::ReducerContext<'a, 'b, 'c, LocalCount>,
) {
    count.0 += 1;
}

fn new_runtime() -> Runtime {
    let mut runtime = Runtime::default();
    runtime
        .add_global_state(Box::new(MemoTestState))
        .expect("memo test state should register");
    runtime
}

fn build_pass(
    runtime: &mut Runtime,
    owner: WidgetId,
    dependencies: u32,
    body_count: Arc<AtomicUsize>,
) -> Widget {
    let env = Env::default();
    let state = runtime
        .get_global_state::<MemoTestState>()
        .expect("memo test state should exist");
    let view = View::new(state, &runtime.runtime_state, &env, None);
    let mut ctx = BuildCtx::new();
    let widget = build::enter(&mut ctx, &view, || {
        let (build, _) = build::current::<MemoTestState>();
        build.memoized_subtree(owner, dependencies, move || {
            body_count.fetch_add(1, Ordering::SeqCst);
            SemanticsRegion {
                identifier: Some("retained-label".into()),
                label: Some("Retained label".into()),
                child: Some(Text::new(format!("dependency-{dependencies}")).into()),
                ..Default::default()
            }
            .into()
        })
    });
    runtime.clear_reducers();
    runtime.absorb_registry(ctx.registry);
    widget
}

#[test]
fn same_dependencies_skip_conversion_and_reuse_the_retained_widget() {
    let mut runtime = new_runtime();
    let owner = WidgetId::from_u128(101);
    let body_count = Arc::new(AtomicUsize::new(0));

    let first = build_pass(&mut runtime, owner, 4, body_count.clone());
    let second = build_pass(&mut runtime, owner, 4, body_count.clone());

    assert_eq!(body_count.load(Ordering::SeqCst), 1);
    let serialized_first = serde_json::to_value(&first).unwrap();
    assert_eq!(serialized_first, serde_json::to_value(&second).unwrap());
    assert!(serialized_first.get("identities_resolved").is_none());
    let (WidgetKind::Composite(first), WidgetKind::Composite(second)) =
        (first.kind(), second.kind())
    else {
        panic!("memoized subtree should preserve its composite owner");
    };
    assert!(std::ptr::eq(first.child.kind(), second.child.kind()));
    let WidgetKind::SemanticsRegion(region) = first.child.kind() else {
        panic!("retained semantic region should remain present");
    };
    assert_eq!(region.identifier.as_deref(), Some("retained-label"));
    assert_eq!(region.label.as_deref(), Some("Retained label"));
    assert_eq!(
        crate::authoring::widget_id(&first.child),
        crate::authoring::widget_id(&second.child)
    );
}

#[test]
fn changed_dependencies_rebuild_the_subtree() {
    let mut runtime = new_runtime();
    let owner = WidgetId::from_u128(102);
    let body_count = Arc::new(AtomicUsize::new(0));

    let first = build_pass(&mut runtime, owner, 4, body_count.clone());
    let changed = build_pass(&mut runtime, owner, 5, body_count.clone());

    assert_eq!(body_count.load(Ordering::SeqCst), 2);
    assert_ne!(
        serde_json::to_value(first).unwrap(),
        serde_json::to_value(changed).unwrap()
    );
}

#[test]
fn cache_hits_replay_scroll_build_dependencies() {
    let mut runtime = new_runtime();
    let env = Env::default();
    let owner = WidgetId::from_u128(103);
    let scroll_id = WidgetId::from_u128(104);
    let mut dependency_sets = Vec::new();

    for _ in 0..2 {
        let state = runtime.get_global_state::<MemoTestState>().unwrap();
        let view = View::new(state, &runtime.runtime_state, &env, None);
        let mut ctx = BuildCtx::new();
        let _widget = build::enter(&mut ctx, &view, || {
            let (build, _) = build::current::<MemoTestState>();
            build.memoized_subtree(owner, (), || {
                let (child_build, _) = build::current::<MemoTestState>();
                child_build.mark_scroll_build_dependency(scroll_id);
                Text::new("virtualized").into()
            })
        });
        dependency_sets.push(ctx.take_scroll_build_dependencies());
    }

    assert!(dependency_sets[0].contains(&scroll_id));
    assert!(dependency_sets[1].contains(&scroll_id));
}

#[test]
fn sibling_owners_keep_independent_cache_entries() {
    let mut runtime = new_runtime();
    let first_owner = WidgetId::from_u128(201);
    let second_owner = WidgetId::from_u128(202);
    let body_count = Arc::new(AtomicUsize::new(0));

    let build_siblings = |runtime: &mut Runtime, first_dep, second_dep| {
        let env = Env::default();
        let state = runtime.get_global_state::<MemoTestState>().unwrap();
        let view = View::new(state, &runtime.runtime_state, &env, None);
        let mut ctx = BuildCtx::new();
        let first_count = body_count.clone();
        let second_count = body_count.clone();
        let widget = build::enter(&mut ctx, &view, || {
            let (build, _) = build::current::<MemoTestState>();
            let first = build.memoized_subtree(first_owner, first_dep, move || {
                first_count.fetch_add(1, Ordering::SeqCst);
                Text::new(format!("first-{first_dep}")).into()
            });
            let second = build.memoized_subtree(second_owner, second_dep, move || {
                second_count.fetch_add(1, Ordering::SeqCst);
                Text::new(format!("second-{second_dep}")).into()
            });
            crate::ui::Column {
                children: vec![first, second],
                ..Default::default()
            }
            .into()
        });
        runtime.clear_reducers();
        runtime.absorb_registry(ctx.registry);
        widget
    };

    let first = build_siblings(&mut runtime, 1, 1);
    let second = build_siblings(&mut runtime, 2, 1);
    let third = build_siblings(&mut runtime, 2, 1);

    assert_eq!(body_count.load(Ordering::SeqCst), 3);
    assert_ne!(
        serde_json::to_value(first).unwrap(),
        serde_json::to_value(second).unwrap()
    );
    assert_eq!(
        serde_json::to_value(second).unwrap(),
        serde_json::to_value(third).unwrap()
    );
}

#[test]
fn cached_bindings_work_and_local_state_changes_invalidate_the_view() {
    let mut runtime = new_runtime();
    let env = Env::default();
    let owner = WidgetId::from_u128(301);
    let sibling_owner = WidgetId::from_u128(302);
    let builds = Arc::new(AtomicUsize::new(0));
    let sibling_builds = Arc::new(AtomicUsize::new(0));
    let observed = Arc::new(Mutex::new(Vec::new()));
    let action_envelope = Arc::new(Mutex::new(None::<ActionEnvelope>));

    let build_local = |runtime: &mut Runtime| {
        let state = runtime.get_global_state::<MemoTestState>().unwrap();
        let view = View::new(state, &runtime.runtime_state, &env, None);
        let mut ctx = BuildCtx::new();
        let builds_for_body = builds.clone();
        let sibling_builds_for_body = sibling_builds.clone();
        let observed_for_body = observed.clone();
        let action_for_body = action_envelope.clone();
        let widget = build::enter(&mut ctx, &view, || {
            let (build, _) = build::current::<MemoTestState>();
            let active = build.memoized_subtree(owner, "fixed-dependencies", move || {
                builds_for_body.fetch_add(1, Ordering::SeqCst);
                let field = crate::StateField::new(
                    "memoized_subtree_test::LocalButton",
                    "count",
                    LocalCount(0),
                );
                let count = field.get().0;
                observed_for_body.lock().unwrap().push(count);
                let action =
                    build.bind_local(IncrementLocal, field, crate::reduce!(increment_local));
                *action_for_body.lock().unwrap() = Some(action.clone());
                Button {
                    child: Some(Text::new(format!("count-{count}")).into()),
                    on_press: Some(action),
                    ..Default::default()
                }
                .into()
            });
            let sibling = build.memoized_subtree(sibling_owner, "fixed-dependencies", move || {
                sibling_builds_for_body.fetch_add(1, Ordering::SeqCst);
                Text::new("unrelated sibling").into()
            });
            crate::ui::Column {
                children: vec![active, sibling],
                ..Default::default()
            }
            .into()
        });
        runtime.clear_reducers();
        runtime.absorb_registry(ctx.registry);
        widget
    };

    let _first = build_local(&mut runtime);
    let _cached = build_local(&mut runtime);
    assert_eq!(builds.load(Ordering::SeqCst), 1);

    runtime
        .dispatch(action_envelope.lock().unwrap().clone().unwrap(), owner)
        .expect("replayed local-state binding should dispatch");

    let updated = build_local(&mut runtime);
    assert_eq!(builds.load(Ordering::SeqCst), 2);
    assert_eq!(sibling_builds.load(Ordering::SeqCst), 1);
    assert_eq!(*observed.lock().unwrap(), vec![0, 1]);
    assert!(serde_json::to_string(&updated).unwrap().contains("count-1"));
}
