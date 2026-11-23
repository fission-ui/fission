use fission_core::{
    Action, ActionId, ActionRegistry, Effect, Effects, GlobalState, JobRef, JobSpec,
    ReducerContext, Runtime, RuntimeEffect, WidgetId,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default)]
struct TestState {
    captured_req_id: Option<u64>,
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
    state.captured_req_id = Some(context.effects.app(TEST_JOB, ()).req_id());
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
    assert!(exposed_req_id >= (1 << 32));
}
