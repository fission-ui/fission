use fission_core::internal::BuildCtx;
use fission_core::{
    build, reduce, ui::TextContent, widgets, Action, ActionEnvelope, ActionId, Button, Column, Env,
    GlobalState, Provider, ReducerContext, Runtime, RuntimeState, Text, View, Widget, WidgetId,
    WidgetIdExt,
};
use fission_macros::{fission_component, fission_reducer, FissionGlobalState, FissionStateView};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Default)]
struct TestState {
    label: String,
    pressed: bool,
}

impl GlobalState for TestState {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Press;

impl Action for Press {
    fn static_id() -> ActionId {
        ActionId::from_name("authoring_api_v2::Press")
    }
}

fn press(state: &mut TestState, _action: Press, _ctx: &mut ReducerContext<TestState>) {
    state.pressed = true;
}

struct ReadsGlobalState;

impl From<ReadsGlobalState> for Widget {
    fn from(_: ReadsGlobalState) -> Self {
        let (ctx, view) = build::current::<TestState>();
        let on_press = ctx.bind(Press, reduce!(press));

        Column {
            children: widgets![
                Text::new(view.state().label.clone()),
                Button {
                    child: Some(Text::new("Press").into()),
                    on_press: Some(on_press),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }
        .into()
    }
}

#[fission_component]
struct LocalCounter {
    title: String,

    #[local_state(default = 0)]
    count: i32,
}

#[fission_reducer(IncrementLocal)]
fn increment_local(count: &mut i32) {
    *count += 1;
}

impl From<LocalCounter> for Widget {
    fn from(counter: LocalCounter) -> Self {
        let (ctx, _) = build::current::<()>();
        let count = counter.count();
        let increment = ctx.bind_local(IncrementLocal, count.clone(), reduce!(increment_local));

        Column {
            children: widgets![
                Text::new(counter.title),
                Text::new(format!("Count: {}", count.get())),
                Button {
                    child: Some(Text::new("Increment").into()),
                    on_press: Some(increment),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }
        .into()
    }
}

struct TwoLocalCounters;

impl From<TwoLocalCounters> for Widget {
    fn from(_: TwoLocalCounters) -> Self {
        Column {
            children: widgets![
                LocalCounter {
                    title: "First".to_string(),
                },
                LocalCounter {
                    title: "Second".to_string(),
                },
            ],
            ..Default::default()
        }
        .into()
    }
}

struct ConditionalLocalCounters {
    show_second: bool,
}

impl From<ConditionalLocalCounters> for Widget {
    fn from(component: ConditionalLocalCounters) -> Self {
        let mut children = widgets![LocalCounter {
            title: "First".to_string(),
        }];
        if component.show_second {
            children.push(
                LocalCounter {
                    title: "Second".to_string(),
                }
                .into(),
            );
        }

        Column {
            children,
            ..Default::default()
        }
        .into()
    }
}

struct ReorderedLocalCounters {
    reversed: bool,
}

impl From<ReorderedLocalCounters> for Widget {
    fn from(component: ReorderedLocalCounters) -> Self {
        let mut counters = vec![("first", "First"), ("second", "Second")];
        if component.reversed {
            counters.reverse();
        }
        let children = counters
            .into_iter()
            .map(|(key, title)| {
                LocalCounter {
                    title: title.to_string(),
                }
                .id(WidgetId::explicit(&format!("counter.{key}")))
            })
            .collect();

        Column {
            children,
            ..Default::default()
        }
        .into()
    }
}

#[derive(Clone, Debug)]
struct ProviderLabel(&'static str);

struct ProviderReader;

impl From<ProviderReader> for Widget {
    fn from(_: ProviderReader) -> Self {
        let label = build::read::<ProviderLabel>();
        Text::new(label.0).into()
    }
}

struct ProviderRoot;

impl From<ProviderRoot> for Widget {
    fn from(_: ProviderRoot) -> Self {
        Provider::new(ProviderLabel("outer"), || Column {
            children: widgets![
                ProviderReader,
                Provider::new(ProviderLabel("inner"), || ProviderReader),
                ProviderReader,
            ],
            ..Default::default()
        })
        .into()
    }
}

#[derive(Debug, Default, Clone, FissionGlobalState)]
struct StoreState {
    cart: CartState,
    session: SessionState,
    #[fission(skip_view)]
    #[allow(dead_code)]
    secret: String,
}

#[derive(Debug, Default, Clone, FissionStateView)]
struct CartState {
    items: Vec<u32>,
}

#[derive(Debug, Default, Clone, FissionStateView)]
struct SessionState {
    user_name: String,
}

struct ReadsGeneratedGlobalView;

impl From<ReadsGeneratedGlobalView> for Widget {
    fn from(_: ReadsGeneratedGlobalView) -> Self {
        let (_, view) = build::current::<StoreState>();
        let global = view.global();
        let item_count = global.cart().items().map(|items| items.len()).get();
        let user_name = global.session().user_name().get();
        let selected_count = view.select(|state| state.cart.items.len());
        assert_eq!(item_count, selected_count);

        Text::new(format!("{user_name}: {item_count}")).into()
    }
}

#[test]
fn typed_build_current_handles_work_during_build() {
    let state = TestState {
        label: "Typed handles".to_string(),
        pressed: false,
    };
    let runtime = RuntimeState::default();
    let env = Env::default();
    let view = View::new(&state, &runtime, &env, None);
    let mut ctx = BuildCtx::<TestState>::new();

    let tree = build::enter(&mut ctx, &view, || ReadsGlobalState.into());
    assert!(lowered_node_count(&tree) > 0);
    let mut state = state;
    ctx.registry
        .dispatch(
            &mut state,
            &ActionEnvelope {
                id: Press::static_id(),
                payload: serde_json::to_vec(&Press).unwrap(),
            },
            WidgetId::from_u128(0),
        )
        .unwrap();
    assert!(state.pressed);
}

#[test]
fn provider_scopes_resolve_nearest_value_without_losing_parent_scope() {
    let runtime = RuntimeState::default();
    let env = Env::default();
    let view = View::new(&(), &runtime, &env, None);
    let mut ctx = BuildCtx::<()>::new();

    let tree = build::enter(&mut ctx, &view, || ProviderRoot.into());
    assert_eq!(
        literal_texts(&tree),
        vec![
            "outer".to_string(),
            "inner".to_string(),
            "outer".to_string()
        ]
    );
}

#[test]
fn provider_read_outside_scope_reports_clear_error() {
    let error = std::panic::catch_unwind(|| build::read::<ProviderLabel>())
        .expect_err("provider read outside build scope should panic");
    let message = panic_message(error);
    assert!(
        message.contains("Fission build provider"),
        "unexpected panic message: {message}"
    );
}

#[test]
fn generated_global_state_views_read_nested_fields() {
    let state = StoreState {
        cart: CartState {
            items: vec![1, 2, 3],
        },
        session: SessionState {
            user_name: "Ada".to_string(),
        },
        secret: "not exposed".to_string(),
    };
    let runtime = RuntimeState::default();
    let env = Env::default();
    let view = View::new(&state, &runtime, &env, None);
    let mut ctx = BuildCtx::<StoreState>::new();

    let tree = build::enter(&mut ctx, &view, || ReadsGeneratedGlobalView.into());
    assert_eq!(literal_texts(&tree), vec!["Ada: 3".to_string()]);
}

#[test]
fn fission_component_local_state_authoring_shape_compiles() {
    let runtime = RuntimeState::default();
    let env = Env::default();
    let view = View::new(&(), &runtime, &env, None);
    let mut ctx = BuildCtx::<()>::new();

    let tree = build::enter(&mut ctx, &view, || {
        LocalCounter {
            title: "Counter".to_string(),
        }
        .into()
    });

    assert!(lowered_node_count(&tree) > 0);
}

#[test]
fn local_state_persists_across_rebuilds() {
    let mut runtime = Runtime::default();
    let env = Env::default();

    let (tree, ctx) = build_local_counter(&runtime, &env);
    assert!(literal_texts(&tree).contains(&"Count: 0".to_string()));
    let increment = button_actions(&tree).remove(0);

    runtime.clear_reducers();
    runtime.absorb_registry(ctx.registry);
    runtime
        .dispatch(increment, WidgetId::from_u128(0))
        .expect("local reducer dispatch should update retained state");

    let (tree, _) = build_local_counter(&runtime, &env);
    assert!(literal_texts(&tree).contains(&"Count: 1".to_string()));
}

#[test]
fn local_state_is_distinct_for_same_component_siblings() {
    let mut runtime = Runtime::default();
    let env = Env::default();

    let (tree, ctx) = build_two_local_counters(&runtime, &env);
    let mut actions = button_actions(&tree);
    let second_increment = actions.remove(1);

    runtime.clear_reducers();
    runtime.absorb_registry(ctx.registry);
    runtime
        .dispatch(second_increment, WidgetId::from_u128(1))
        .expect("second local reducer dispatch should update only second field");

    let (tree, _) = build_two_local_counters(&runtime, &env);
    let texts = literal_texts(&tree);
    let counts: Vec<_> = texts
        .into_iter()
        .filter(|text| text.starts_with("Count:"))
        .collect();
    assert_eq!(counts, vec!["Count: 0".to_string(), "Count: 1".to_string()]);
}

#[test]
fn widget_id_scopes_local_state_for_reordered_dynamic_children() {
    let mut runtime = Runtime::default();
    let env = Env::default();

    let (tree, ctx) = build_reordered_local_counters(&runtime, &env, false);
    let second_increment = button_actions(&tree).remove(1);

    runtime.clear_reducers();
    runtime.absorb_registry(ctx.registry);
    runtime
        .dispatch(second_increment, WidgetId::from_u128(1))
        .expect("second local reducer dispatch should update retained state");

    let (tree, _) = build_reordered_local_counters(&runtime, &env, true);
    assert_eq!(
        literal_texts(&tree),
        vec![
            "Second".to_string(),
            "Count: 1".to_string(),
            "Increment".to_string(),
            "First".to_string(),
            "Count: 0".to_string(),
            "Increment".to_string(),
        ]
    );
}

#[test]
fn removed_local_state_identity_is_pruned_after_rebuild() {
    let mut runtime = Runtime::default();
    let env = Env::default();

    let (tree, ctx) = build_maybe_second_counter(&runtime, &env, true);
    let second_increment = button_actions(&tree).remove(1);

    runtime.clear_reducers();
    runtime.absorb_registry(ctx.registry);
    runtime
        .dispatch(second_increment.clone(), WidgetId::from_u128(1))
        .expect("second local reducer dispatch should update retained state");
    assert_eq!(runtime.runtime_state.local_widget_state.len(), 2);

    let (tree, ctx) = build_maybe_second_counter(&runtime, &env, false);
    assert_eq!(
        literal_texts(&tree)
            .into_iter()
            .filter(|text| text.starts_with("Count:"))
            .collect::<Vec<_>>(),
        vec!["Count: 0".to_string()]
    );
    assert_eq!(runtime.runtime_state.local_widget_state.len(), 1);

    runtime.clear_reducers();
    runtime.absorb_registry(ctx.registry);
    runtime
        .dispatch(second_increment, WidgetId::from_u128(1))
        .expect("stale local state action should be ignored once its reducer is gone");
    assert_eq!(runtime.runtime_state.local_widget_state.len(), 1);
}

#[test]
fn public_authoring_api_does_not_reintroduce_removed_escape_hatches() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo = manifest
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .expect("fission-core should live under crates/core/fission-core");
    let scan_roots = [
        repo.join("crates/core/fission-core/src"),
        repo.join("crates/authoring/fission/src"),
        repo.join("crates/authoring/fission-widgets/src"),
        repo.join("crates/authoring/fission-charts/src"),
    ];
    let forbidden = [
        ["Any", "Widget"].concat(),
        ["Any", "BuildCtxHandle"].concat(),
        ["Any", "ViewHandle"].concat(),
        ["current", "_any"].concat(),
        ["build", "_node"].concat(),
        ["into", "_node"].concat(),
        ["from", "_node"].concat(),
        ["internal", "_node_widget"].concat(),
        ["pub trait ", "Widget", "<"].concat(),
        ["pub trait ", "Widget", " "].concat(),
        ["pub trait ", "Widget", ":"].concat(),
        ["impl ", "Widget", "<"].concat(),
    ];

    let mut violations = Vec::new();
    for root in scan_roots {
        visit_rs_files(&root, &mut |path| {
            let text = fs::read_to_string(path).expect("source file should be readable");
            for needle in &forbidden {
                if text.contains(needle) {
                    violations.push(format!("{} contains `{}`", path.display(), needle));
                }
            }
        });
    }

    assert!(
        violations.is_empty(),
        "removed v2 authoring API names leaked back into public sources:\n{}",
        violations.join("\n")
    );
}

fn visit_rs_files(root: &Path, f: &mut impl FnMut(&Path)) {
    for entry in fs::read_dir(root).expect("scan root should exist") {
        let entry = entry.expect("source directory entry should be readable");
        let path = entry.path();
        if path.is_dir() {
            visit_rs_files(&path, f);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            f(&path);
        }
    }
}

fn build_local_counter(runtime: &Runtime, env: &Env) -> (Widget, BuildCtx<()>) {
    let view = View::new(&(), &runtime.runtime_state, env, None);
    let mut ctx = BuildCtx::<()>::new();
    let tree = build::enter(&mut ctx, &view, || {
        LocalCounter {
            title: "Counter".to_string(),
        }
        .into()
    });
    (tree, ctx)
}

fn build_two_local_counters(runtime: &Runtime, env: &Env) -> (Widget, BuildCtx<()>) {
    let view = View::new(&(), &runtime.runtime_state, env, None);
    let mut ctx = BuildCtx::<()>::new();
    let tree = build::enter(&mut ctx, &view, || TwoLocalCounters.into());
    (tree, ctx)
}

fn build_maybe_second_counter(
    runtime: &Runtime,
    env: &Env,
    show_second: bool,
) -> (Widget, BuildCtx<()>) {
    let view = View::new(&(), &runtime.runtime_state, env, None);
    let mut ctx = BuildCtx::<()>::new();
    let tree = build::enter(&mut ctx, &view, || {
        ConditionalLocalCounters { show_second }.into()
    });
    (tree, ctx)
}

fn build_reordered_local_counters(
    runtime: &Runtime,
    env: &Env,
    reversed: bool,
) -> (Widget, BuildCtx<()>) {
    let view = View::new(&(), &runtime.runtime_state, env, None);
    let mut ctx = BuildCtx::<()>::new();
    let tree = build::enter(&mut ctx, &view, || {
        ReorderedLocalCounters { reversed }.into()
    });
    (tree, ctx)
}

fn literal_texts(widget: &Widget) -> Vec<String> {
    let mut out = Vec::new();
    collect_literal_texts(widget, &mut out);
    out
}

fn button_actions(widget: &Widget) -> Vec<ActionEnvelope> {
    let mut out = Vec::new();
    collect_button_actions(widget, &mut out);
    out
}

fn collect_literal_texts(widget: &Widget, out: &mut Vec<String>) {
    if let Some(text) = fission_core::internal::widget_as_text(widget) {
        if let TextContent::Literal(value) = &text.content {
            out.push(value.clone());
        }
        return;
    }

    if let Some(column) = fission_core::internal::widget_as_column(widget) {
        for child in &column.children {
            collect_literal_texts(child, out);
        }
        return;
    }

    if let Some(row) = fission_core::internal::widget_as_row(widget) {
        for child in &row.children {
            collect_literal_texts(child, out);
        }
        return;
    }

    if let Some(container) = fission_core::internal::widget_as_container(widget) {
        if let Some(child) = &container.child {
            collect_literal_texts(child, out);
        }
        return;
    }

    if let Some(button) = fission_core::internal::widget_as_button(widget) {
        if let Some(child) = &button.child {
            collect_literal_texts(child, out);
        }
    }
}

fn collect_button_actions(widget: &Widget, out: &mut Vec<ActionEnvelope>) {
    if let Some(button) = fission_core::internal::widget_as_button(widget) {
        if let Some(action) = &button.on_press {
            out.push(action.clone());
        }
        if let Some(child) = &button.child {
            collect_button_actions(child, out);
        }
        return;
    }

    if let Some(column) = fission_core::internal::widget_as_column(widget) {
        for child in &column.children {
            collect_button_actions(child, out);
        }
        return;
    }

    if let Some(row) = fission_core::internal::widget_as_row(widget) {
        for child in &row.children {
            collect_button_actions(child, out);
        }
        return;
    }

    if let Some(container) = fission_core::internal::widget_as_container(widget) {
        if let Some(child) = &container.child {
            collect_button_actions(child, out);
        }
    }
}

fn lowered_node_count(widget: &Widget) -> usize {
    let lower = fission_core::internal::lower_widget_to_ir(widget);
    lower.nodes.len()
}

fn panic_message(error: Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = error.downcast_ref::<String>() {
        message.clone()
    } else if let Some(message) = error.downcast_ref::<&'static str>() {
        (*message).to_string()
    } else {
        "<non-string panic>".to_string()
    }
}
