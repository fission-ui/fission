use fission_core::{
    Action, ActionId, ActionRegistry, Effect, Effects, GlobalState, JobRef, JobSpec,
    ReducerContext, Runtime, RuntimeEffect, WidgetId,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default)]
struct TestState {
    captured_req_id: Option<u64>,
    captured_req_ids: Vec<u64>,
}
impl GlobalState for TestState {}

#[derive(Debug)]
struct TestJob;

impl JobSpec for TestJob {
    type Request = ();
    type Ok = ();
    type Err = String;

    const NAME: &'static str = "effect-cancellation-test-job";
}

const TEST_JOB: JobRef<TestJob> = JobRef::new("effect-cancellation-test-job");

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StartTestJob;

impl Action for StartTestJob {
    fn static_id() -> ActionId {
        ActionId::from_u128(0x544553545f43414e43454c)
    }
}

fn start_test_job(
    state: &mut TestState,
    _action: StartTestJob,
    context: &mut ReducerContext<'_, '_, '_, TestState>,
) {
    let req_id = context.effects.app(TEST_JOB, ()).req_id();
    state.captured_req_id = Some(req_id);
    state.captured_req_ids.push(req_id);
}

#[test]
fn effect_builder_exposes_the_request_id_used_by_cancel() {
    let mut registry = ActionRegistry::<TestState>::new();
    let mut effects = Effects::new(41, &mut registry);

    let req_id = effects.app(TEST_JOB, ()).req_id();
    effects.cancel(req_id);

    assert_eq!(effects.out[0].req_id, 41);
    assert!(matches!(
        &effects.out[0].effect,
        Effect::Job(job) if job.job_name == TEST_JOB.name
    ));
    assert!(matches!(
        &effects.out[1].effect,
        Effect::Runtime(RuntimeEffect::Cancel { req_id: cancelled }) if *cancelled == req_id
    ));
}

#[test]
fn runtime_queues_job_cancellation_for_the_shell() {
    let mut runtime = Runtime::default();
    assert!(runtime.queue_runtime_effect(RuntimeEffect::Cancel { req_id: 19 }));
    assert_eq!(runtime.take_pending_job_cancellations(), vec![19]);
    assert!(runtime.take_pending_job_cancellations().is_empty());
}

#[test]
fn runtime_preserves_the_request_id_exposed_inside_a_reducer() {
    let mut runtime = Runtime::default();
    runtime
        .add_app_state(Box::new(TestState::default()))
        .unwrap();
    let mut registry = ActionRegistry::<TestState>::new();
    registry.register(start_test_job as fn(&mut TestState, StartTestJob, &mut ReducerContext<_>));
    runtime.absorb_registry(registry);

    runtime
        .dispatch(StartTestJob.into(), WidgetId::from_u128(0))
        .unwrap();

    let exposed_req_id = runtime
        .get_app_state::<TestState>()
        .and_then(|state| state.captured_req_id)
        .expect("reducer captured the job request id");
    assert_eq!(runtime.pending_effects[0].req_id, exposed_req_id);
    assert_eq!(exposed_req_id, 0);
}

#[test]
fn multiple_handlers_and_dispatches_share_one_runtime_request_counter() {
    let mut runtime = Runtime::default();
    runtime
        .add_app_state(Box::new(TestState::default()))
        .unwrap();
    runtime.next_req_id = (1_u64 << 32) - 1;
    let mut registry = ActionRegistry::<TestState>::new();
    let handler: fission_core::registry::Handler<TestState, StartTestJob> = start_test_job;
    registry.register(handler);
    registry.register(handler);
    runtime.absorb_registry(registry);

    for _ in 0..2 {
        runtime
            .dispatch(StartTestJob.into(), WidgetId::from_u128(0))
            .unwrap();
    }
    let expected = ((1_u64 << 32) - 1..(1_u64 << 32) + 3).collect::<Vec<_>>();
    let state = runtime.get_app_state::<TestState>().unwrap();
    assert_eq!(state.captured_req_ids, expected);
    assert_eq!(
        runtime
            .pending_effects
            .iter()
            .map(|effect| effect.req_id)
            .collect::<Vec<_>>(),
        expected
    );
    assert_eq!(runtime.next_req_id, (1_u64 << 32) + 3);
}

#[derive(Debug, Clone, Default)]
struct LocalJobState(Vec<u64>);
impl GlobalState for LocalJobState {}

fn start_local_job(
    state: &mut LocalJobState,
    _: StartTestJob,
    context: &mut ReducerContext<LocalJobState>,
) {
    state.0.push(context.effects.app(TEST_JOB, ()).req_id());
}

#[test]
fn local_state_reducers_reserve_ids_from_the_same_runtime_counter() {
    let mut runtime = Runtime::default();
    runtime
        .add_app_state(Box::new(TestState::default()))
        .unwrap();
    runtime.next_req_id = 19;
    let env = fission_core::Env::default();
    let state = runtime.get_app_state::<TestState>().unwrap();
    let view = fission_core::View::new(state, &runtime.runtime_state, &env, None);
    let mut ctx = fission_core::authoring::BuildCtx::new();
    let (field, action) = fission_core::build::enter(&mut ctx, &view, || {
        let field =
            fission_core::StateField::new("effect-id-test", "local-job", LocalJobState::default());
        let (build, _) = fission_core::build::current::<TestState>();
        let handler: fission_core::registry::Handler<LocalJobState, StartTestJob> = start_local_job;
        let action = build.bind_local(StartTestJob, field.clone(), handler);
        (field, action)
    });
    runtime.absorb_registry(ctx.registry);
    runtime
        .dispatch(action.clone(), WidgetId::from_u128(0))
        .unwrap();
    runtime.dispatch(action, WidgetId::from_u128(0)).unwrap();
    // StateField is a build snapshot; inspect the retained store after dispatch.
    assert!(field.get().0.is_empty());
    let retained = runtime
        .runtime_state
        .local_widget_state
        .get_or_insert_with(field.key().clone(), LocalJobState::default);
    assert_eq!(retained.0, vec![19, 20]);
    assert_eq!(
        runtime
            .pending_effects
            .iter()
            .map(|effect| effect.req_id)
            .collect::<Vec<_>>(),
        vec![19, 20]
    );
    assert_eq!(runtime.next_req_id, 21);
}

#[test]
#[should_panic(expected = "Fission effect request IDs exhausted")]
fn request_id_exhaustion_never_wraps_and_reuses_an_id() {
    let mut registry = ActionRegistry::<TestState>::new();
    let mut effects = Effects::new(u64::MAX, &mut registry);
    effects.app(TEST_JOB, ());
}
