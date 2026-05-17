use fission_core::{
    reduce_with, AppState, BuildCtx, JobRef, JobResource, ReducerContext, ResourceKey, Runtime,
    TimerResource,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone)]
struct TestState {
    ticks: u32,
    last_payload: String,
}

impl AppState for TestState {}

#[fission_macros::fission_action]
struct TimerFired;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct TickPayload {
    label: String,
}

fn on_timer_fired(state: &mut TestState, _: TimerFired, ctx: &mut ReducerContext<TestState>) {
    let payload: TickPayload = ctx.input.timer_tick().expect("timer payload");
    state.ticks += 1;
    state.last_payload = payload.label;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct LoadDetailRequest {
    todo_id: u64,
}

#[derive(Debug)]
struct LoadDetailJob;

impl fission_core::JobSpec for LoadDetailJob {
    type Request = LoadDetailRequest;
    type Ok = String;
    type Err = String;
    const NAME: &'static str = "load-detail";
}

const LOAD_DETAIL: JobRef<LoadDetailJob> = JobRef::new("load-detail");

#[test]
fn job_resources_restart_with_new_generation_when_deps_change() {
    let mut runtime = Runtime::default();
    runtime
        .add_app_state(Box::new(TestState::default()))
        .unwrap();

    let resource = JobResource::new(
        ResourceKey::new("detail"),
        LOAD_DETAIL,
        LoadDetailRequest { todo_id: 1 },
    )
    .deps(1_u64);
    runtime
        .reconcile_resources(vec![fission_core::RuntimeResourceDeclaration {
            key: "detail".into(),
            deps: resource.deps.clone(),
            policy: resource.policy,
            kind: fission_core::RuntimeResourceKind::Job(resource.clone()),
        }])
        .unwrap();

    assert_eq!(runtime.pending_effects.len(), 1);
    let first = runtime.pending_effects[0]
        .resource
        .clone()
        .expect("resource context");

    runtime.pending_effects.clear();
    runtime
        .reconcile_resources(vec![fission_core::RuntimeResourceDeclaration {
            key: "detail".into(),
            deps: resource.deps.clone(),
            policy: resource.policy,
            kind: fission_core::RuntimeResourceKind::Job(resource),
        }])
        .unwrap();
    assert!(runtime.pending_effects.is_empty());

    let changed = JobResource::new(
        ResourceKey::new("detail"),
        LOAD_DETAIL,
        LoadDetailRequest { todo_id: 2 },
    )
    .deps(2_u64);
    runtime
        .reconcile_resources(vec![fission_core::RuntimeResourceDeclaration {
            key: "detail".into(),
            deps: changed.deps.clone(),
            policy: changed.policy,
            kind: fission_core::RuntimeResourceKind::Job(changed),
        }])
        .unwrap();

    assert_eq!(runtime.pending_effects.len(), 1);
    let second = runtime.pending_effects[0]
        .resource
        .clone()
        .expect("resource context");
    assert_ne!(first.generation, second.generation);
}

#[test]
fn timer_resources_dispatch_actions_from_runtime_tick() {
    let mut runtime = Runtime::default();
    runtime
        .add_app_state(Box::new(TestState::default()))
        .unwrap();

    let mut ctx = BuildCtx::new();
    let on_tick = ctx.bind(TimerFired, reduce_with!(on_timer_fired));
    runtime.clear_reducers();
    runtime.absorb_registry(ctx.registry);

    let timer = TimerResource::new(
        ResourceKey::new("refresh"),
        std::time::Duration::from_millis(10),
        TickPayload {
            label: "refresh".into(),
        },
    )
    .immediate()
    .on_tick(on_tick);

    runtime
        .reconcile_resources(vec![fission_core::RuntimeResourceDeclaration {
            key: "refresh".into(),
            deps: timer.deps.clone(),
            policy: timer.policy,
            kind: fission_core::RuntimeResourceKind::Timer(timer),
        }])
        .unwrap();

    runtime.tick(0).unwrap();
    let state = runtime.get_app_state::<TestState>().unwrap();
    assert_eq!(state.ticks, 1);
    assert_eq!(state.last_payload, "refresh");
}
