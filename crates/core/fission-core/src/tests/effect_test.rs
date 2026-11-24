use crate::runtime::Runtime;
use crate::{
    reduce_with, Action, ActionEnvelope, ActionId, CapabilityType, Effect, GlobalState, JobSpec,
    OperationCapability, ReducerContext,
};
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone)]
struct TestState {
    data: String,
    loading: bool,
    completions: usize,
}
impl GlobalState for TestState {}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct UploadFileRequest {
    path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct UploadFileOk {
    bytes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct UploadFileErr(String);

#[derive(Debug)]
struct UploadFile;

impl OperationCapability for UploadFile {
    type Request = UploadFileRequest;
    type Ok = UploadFileOk;
    type Err = UploadFileErr;
}

const UPLOAD_FILE: CapabilityType<UploadFile> = CapabilityType::new("upload-file");

fn on_upload_requested<'a, 'b, 'c>(
    state: &mut TestState,
    _: UploadRequested,
    ctx: &mut ReducerContext<'a, 'b, 'c, TestState>,
) {
    state.loading = true;
    let on_ok = ctx
        .effects
        .bind(UploadFinished, reduce_with!(on_upload_finished));
    ctx.effects
        .capability(
            UPLOAD_FILE,
            UploadFileRequest {
                path: "/tmp/payload.bin".into(),
            },
        )
        .on_ok(on_ok);
}

fn on_upload_finished<'a, 'b, 'c>(
    state: &mut TestState,
    _: UploadFinished,
    ctx: &mut ReducerContext<'a, 'b, 'c, TestState>,
) {
    state.loading = false;
    state.completions += 1;
    if let Some(result) = ctx.input.capability_ok(UPLOAD_FILE) {
        state.data = format!("uploaded {} bytes", result.bytes);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct UploadRequested;

impl Action for UploadRequested {
    fn static_id() -> ActionId {
        ActionId::from_name("UploadRequested")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct UploadFinished;

impl Action for UploadFinished {
    fn static_id() -> ActionId {
        ActionId::from_name("UploadFinished")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct RegisterUnusedCallback;

impl Action for RegisterUnusedCallback {
    fn static_id() -> ActionId {
        ActionId::from_name("RegisterUnusedCallback")
    }
}

fn register_unused_callback<'a, 'b, 'c>(
    _: &mut TestState,
    _: RegisterUnusedCallback,
    ctx: &mut ReducerContext<'a, 'b, 'c, TestState>,
) {
    let _unused = ctx
        .effects
        .bind(UploadFinished, reduce_with!(on_upload_finished));
}

#[test]
fn test_capability_effect_loop() {
    let mut runtime = Runtime::default();
    runtime
        .add_global_state(Box::new(TestState::default()))
        .unwrap();

    let mut registry = crate::registry::ActionRegistry::new();
    registry.register(reduce_with!(on_upload_requested));
    runtime.absorb_registry(registry);

    runtime
        .dispatch(
            ActionEnvelope {
                id: UploadRequested::static_id(),
                payload: UploadRequested.encode(),
            },
            crate::WidgetId::from_u128(0),
        )
        .unwrap();

    let env = runtime.pending_effects.pop().unwrap();
    let on_ok = env.on_ok.clone().expect("capability continuation");
    runtime.clear_reducers();
    runtime
        .dispatch_with_input(
            on_ok.clone(),
            crate::WidgetId::from_u128(0),
            &crate::ActionInput::CapabilityOk {
                capability: "upload-file".into(),
                req_id: env.req_id,
                payload: serde_json::to_vec(&UploadFileOk { bytes: 11 }).unwrap(),
            },
        )
        .unwrap();

    let state = runtime.get_global_state::<TestState>().unwrap();
    assert!(!state.loading);
    assert_eq!(state.data, "uploaded 11 bytes");
    assert_eq!(state.completions, 1);

    runtime
        .dispatch_with_input(
            on_ok,
            crate::WidgetId::from_u128(0),
            &crate::ActionInput::CapabilityOk {
                capability: "upload-file".into(),
                req_id: env.req_id,
                payload: serde_json::to_vec(&UploadFileOk { bytes: 99 }).unwrap(),
            },
        )
        .unwrap();
    let state = runtime.get_global_state::<TestState>().unwrap();
    assert_eq!(state.data, "uploaded 11 bytes");
    assert_eq!(state.completions, 1);
}

#[test]
fn discard_pending_effects_removes_envelopes_and_one_shot_callbacks() {
    let mut runtime = Runtime::default();
    runtime
        .add_global_state(Box::new(TestState::default()))
        .unwrap();

    let mut registry = crate::registry::ActionRegistry::new();
    registry.register(reduce_with!(on_upload_requested));
    runtime.absorb_registry(registry);
    runtime
        .dispatch(
            ActionEnvelope {
                id: UploadRequested::static_id(),
                payload: UploadRequested.encode(),
            },
            crate::WidgetId::from_u128(0),
        )
        .unwrap();

    let completion = runtime.pending_effects[0]
        .on_ok
        .clone()
        .expect("capability continuation");
    assert_eq!(runtime.discard_pending_effects(), 2);
    assert!(runtime.pending_effects.is_empty());

    runtime
        .dispatch_with_input(
            completion,
            crate::WidgetId::from_u128(0),
            &crate::ActionInput::CapabilityOk {
                capability: "upload-file".into(),
                req_id: 0,
                payload: serde_json::to_vec(&UploadFileOk { bytes: 11 }).unwrap(),
            },
        )
        .unwrap();
    let state = runtime.get_global_state::<TestState>().unwrap();
    assert!(state.loading);
    assert_eq!(state.completions, 0);
}

#[test]
fn discard_pending_effects_removes_unattached_callbacks() {
    let mut runtime = Runtime::default();
    runtime
        .add_global_state(Box::new(TestState::default()))
        .unwrap();
    let mut registry = crate::registry::ActionRegistry::new();
    registry.register(reduce_with!(register_unused_callback));
    runtime.absorb_registry(registry);

    runtime
        .dispatch(
            ActionEnvelope {
                id: RegisterUnusedCallback::static_id(),
                payload: RegisterUnusedCallback.encode(),
            },
            crate::WidgetId::from_u128(0),
        )
        .unwrap();

    assert!(runtime.pending_effects.is_empty());
    assert_eq!(runtime.discard_pending_effects(), 1);
    assert_eq!(runtime.discard_pending_effects(), 0);
}

#[test]
fn test_operation_capability_effect() {
    let mut runtime = Runtime::default();
    runtime
        .add_global_state(Box::new(TestState::default()))
        .unwrap();

    let mut registry = crate::registry::ActionRegistry::new();
    registry.register(reduce_with!(on_upload_requested));
    runtime.absorb_registry(registry);

    runtime
        .dispatch(
            ActionEnvelope {
                id: UploadRequested::static_id(),
                payload: UploadRequested.encode(),
            },
            crate::WidgetId::from_u128(0),
        )
        .unwrap();

    assert_eq!(runtime.pending_effects.len(), 1);
    let env = runtime.pending_effects.pop().unwrap();
    match env.effect {
        Effect::Capability(crate::capability::CapabilityInvocationPayload::Operation(op)) => {
            assert_eq!(op.capability_name, "upload-file");
            let request: UploadFileRequest = serde_json::from_slice(&op.request).unwrap();
            assert_eq!(request.path, "/tmp/payload.bin");
        }
        _ => panic!("expected typed capability effect"),
    }
}

#[derive(Debug, Default)]
struct CallbackLifetimeState {
    probes: Vec<std::sync::Weak<()>>,
    completions: usize,
}

impl GlobalState for CallbackLifetimeState {}

struct CapturedCallback(std::sync::Arc<()>);

impl crate::registry::IntoHandler<CallbackLifetimeState, UploadFinished> for CapturedCallback {
    fn call<'a, 'b, 'c>(
        &self,
        state: &mut CallbackLifetimeState,
        _: UploadFinished,
        _: &mut ReducerContext<'a, 'b, 'c, CallbackLifetimeState>,
    ) {
        let _retained_component = &self.0;
        state.completions += 1;
    }
}

struct CallbackJob;

impl crate::JobSpec for CallbackJob {
    type Request = ();
    type Ok = ();
    type Err = String;
    const NAME: &'static str = "callback-lifetime-job";
}

fn start_callback_job(
    state: &mut CallbackLifetimeState,
    _: UploadRequested,
    ctx: &mut ReducerContext<CallbackLifetimeState>,
) {
    let retained_component = std::sync::Arc::new(());
    state
        .probes
        .push(std::sync::Arc::downgrade(&retained_component));
    let on_ok = ctx
        .effects
        .bind(UploadFinished, CapturedCallback(retained_component.clone()));
    let on_err = ctx
        .effects
        .bind(UploadFinished, CapturedCallback(retained_component));
    ctx.effects
        .app(crate::JobRef::<CallbackJob>::new(CallbackJob::NAME), ())
        .on_ok(on_ok)
        .on_err(on_err);
}

fn callback_runtime() -> Runtime {
    let mut runtime = Runtime::default();
    runtime
        .add_app_state(Box::new(CallbackLifetimeState::default()))
        .unwrap();
    let mut registry = crate::ActionRegistry::new();
    registry.register(
        start_callback_job as crate::registry::Handler<CallbackLifetimeState, UploadRequested>,
    );
    runtime.absorb_persistent_registry(registry);
    runtime
}

fn request_callback_job(runtime: &mut Runtime) -> crate::EffectEnvelope {
    runtime
        .dispatch(UploadRequested.into(), crate::WidgetId::from_u128(0))
        .unwrap();
    runtime.pending_effects.pop().unwrap()
}

#[test]
fn repeated_job_cancellation_releases_both_captured_callbacks_without_running_them() {
    let mut runtime = callback_runtime();
    for _ in 0..100 {
        let effect = request_callback_job(&mut runtime);
        let probe = runtime
            .get_app_state::<CallbackLifetimeState>()
            .unwrap()
            .probes
            .last()
            .unwrap()
            .clone();
        assert!(probe.upgrade().is_some());
        runtime.queue_runtime_effect(crate::RuntimeEffect::Cancel {
            req_id: effect.req_id,
        });
        assert!(probe.upgrade().is_none());
    }
    assert_eq!(
        runtime
            .get_app_state::<CallbackLifetimeState>()
            .unwrap()
            .completions,
        0
    );
    assert_eq!(runtime.discard_pending_effects(), 0);
}

#[test]
fn job_terminal_outcome_releases_the_unused_callback_and_preserves_another_request() {
    for failed in [false, true] {
        let mut runtime = callback_runtime();
        let cancelled = request_callback_job(&mut runtime);
        let completed = request_callback_job(&mut runtime);
        runtime.queue_runtime_effect(crate::RuntimeEffect::Cancel {
            req_id: cancelled.req_id,
        });
        let state = runtime.get_app_state::<CallbackLifetimeState>().unwrap();
        assert!(state.probes[0].upgrade().is_none());
        assert!(state.probes[1].upgrade().is_some());
        let (selected, unused, input) = if failed {
            (
                completed.on_err.unwrap(),
                completed.on_ok.unwrap(),
                crate::ActionInput::JobErr {
                    job_name: CallbackJob::NAME.into(),
                    req_id: completed.req_id,
                    payload: None,
                    message: Some("failed".into()),
                },
            )
        } else {
            (
                completed.on_ok.unwrap(),
                completed.on_err.unwrap(),
                crate::ActionInput::JobOk {
                    job_name: CallbackJob::NAME.into(),
                    req_id: completed.req_id,
                    payload: serde_json::to_vec(&()).unwrap(),
                },
            )
        };
        runtime
            .dispatch_with_input(selected, crate::WidgetId::from_u128(0), &input)
            .unwrap();
        let state = runtime.get_app_state::<CallbackLifetimeState>().unwrap();
        assert!(state.probes[1].upgrade().is_none());
        assert_eq!(state.completions, 1);
        runtime
            .dispatch_with_input(unused, crate::WidgetId::from_u128(0), &input)
            .unwrap();
        assert_eq!(
            runtime
                .get_app_state::<CallbackLifetimeState>()
                .unwrap()
                .completions,
            1
        );
        assert_eq!(runtime.discard_pending_effects(), 0);
    }
}

fn start_shared_callback_jobs(
    state: &mut CallbackLifetimeState,
    _: UploadRequested,
    ctx: &mut ReducerContext<CallbackLifetimeState>,
) {
    let retained_component = std::sync::Arc::new(());
    state
        .probes
        .push(std::sync::Arc::downgrade(&retained_component));
    let shared = ctx
        .effects
        .bind(UploadFinished, CapturedCallback(retained_component));
    for _ in 0..2 {
        ctx.effects
            .app(crate::JobRef::<CallbackJob>::new(CallbackJob::NAME), ())
            .on_err(shared.clone());
    }
}

#[test]
fn cancelling_one_request_does_not_remove_an_ephemeral_callback_shared_with_another() {
    let mut runtime = Runtime::default();
    runtime
        .add_app_state(Box::new(CallbackLifetimeState::default()))
        .unwrap();
    let mut registry = crate::ActionRegistry::new();
    registry.register(
        start_shared_callback_jobs
            as crate::registry::Handler<CallbackLifetimeState, UploadRequested>,
    );
    runtime.absorb_persistent_registry(registry);
    runtime
        .dispatch(UploadRequested.into(), crate::WidgetId::from_u128(0))
        .unwrap();
    let second = runtime.pending_effects.pop().unwrap();
    let first = runtime.pending_effects.pop().unwrap();
    runtime.queue_runtime_effect(crate::RuntimeEffect::Cancel {
        req_id: first.req_id,
    });
    assert!(runtime
        .get_app_state::<CallbackLifetimeState>()
        .unwrap()
        .probes[0]
        .upgrade()
        .is_some());
    runtime
        .dispatch_with_input(
            second.on_err.unwrap(),
            crate::WidgetId::from_u128(0),
            &crate::ActionInput::JobErr {
                job_name: CallbackJob::NAME.into(),
                req_id: second.req_id,
                payload: None,
                message: None,
            },
        )
        .unwrap();
    let state = runtime.get_app_state::<CallbackLifetimeState>().unwrap();
    assert!(state.probes[0].upgrade().is_none());
    assert_eq!(state.completions, 1);
}

#[test]
fn cancelling_a_job_keeps_reusable_registered_action_reducers() {
    let mut runtime = callback_runtime();
    let mut registry = crate::ActionRegistry::new();
    let handler: fn(&mut CallbackLifetimeState, UploadFinished) = |state, _| state.completions += 1;
    registry.register(handler);
    runtime.absorb_persistent_registry(registry);
    let effect = request_callback_job(&mut runtime);
    runtime.queue_runtime_effect(crate::RuntimeEffect::Cancel {
        req_id: effect.req_id,
    });
    runtime
        .dispatch(UploadFinished.into(), crate::WidgetId::from_u128(0))
        .unwrap();
    assert_eq!(
        runtime
            .get_app_state::<CallbackLifetimeState>()
            .unwrap()
            .completions,
        1
    );
}
