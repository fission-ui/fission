use crate::runtime::Runtime;
use crate::{
    reduce_with, Action, ActionEnvelope, ActionId, CapabilityType, Effect, GlobalState,
    OperationCapability, ReducerContext,
};
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone)]
struct TestState {
    data: String,
    loading: bool,
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

#[test]
fn test_capability_effect_loop() {
    let mut runtime = Runtime::default();
    runtime
        .add_global_state(Box::new(TestState::default()))
        .unwrap();

    let mut registry = crate::registry::ActionRegistry::new();
    registry.register(reduce_with!(on_upload_requested));
    registry.register(reduce_with!(on_upload_finished));
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
    runtime
        .dispatch_with_input(
            on_ok,
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
