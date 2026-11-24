#![cfg(not(target_arch = "wasm32"))]

use fission_core::{
    ActionEnvelope, ActionId, BoxFuture, ServiceCtx, ServiceRunner, ServiceSpec, ServiceType,
};
use fission_shell::async_host::{AsyncMessage, AsyncRegistry, ServiceControlMessage};
use serde::{Deserialize, Serialize, Serializer};
use std::sync::{mpsc, Arc};
use std::time::Duration;

#[derive(Deserialize)]
struct InvalidResult;

impl Serialize for InvalidResult {
    fn serialize<S: Serializer>(&self, _serializer: S) -> Result<S::Ok, S::Error> {
        Err(serde::ser::Error::custom(
            "deliberate result serialization failure",
        ))
    }
}

struct TestService;

impl ServiceSpec for TestService {
    type Config = ();
    type Command = ();
    type CommandOk = InvalidResult;
    type CommandErr = String;
    type Event = ();
    type StartErr = String;
    const NAME: &'static str = "serialization-regression";
}

struct Runner;

impl ServiceRunner<TestService> for Runner {
    fn on_command(
        &mut self,
        _command: (),
        _ctx: ServiceCtx<TestService>,
    ) -> BoxFuture<Result<InvalidResult, String>> {
        Box::pin(async { Ok(InvalidResult) })
    }

    fn on_stop(self: Box<Self>, _ctx: ServiceCtx<TestService>) -> BoxFuture<()> {
        Box::pin(async {})
    }
}

#[test]
fn an_unserializable_service_result_is_an_error_not_an_empty_success() {
    let service = ServiceType::<TestService>::new(TestService::NAME);
    let mut registry = AsyncRegistry::new();
    registry.register_service(service, |(), _ctx| async {
        Ok::<_, String>(Box::new(Runner) as Box<dyn ServiceRunner<TestService>>)
    });
    let (tx, rx) = mpsc::channel();
    let handle = registry
        .spawn_service(
            service.name,
            "test".into(),
            7,
            b"null".to_vec(),
            None,
            &tx,
            Arc::new(|| {}),
        )
        .expect("registered service");
    assert!(matches!(
        rx.recv_timeout(Duration::from_secs(5)).unwrap(),
        AsyncMessage::ServiceStarted { .. }
    ));
    let failure_action = ActionEnvelope {
        id: ActionId::from_u128(12),
        payload: Vec::new(),
    };
    handle
        .control_tx
        .send(ServiceControlMessage::Command {
            req_id: 19,
            payload: b"null".to_vec(),
            on_ok: None,
            on_err: Some(failure_action.clone()),
        })
        .unwrap();
    match rx.recv_timeout(Duration::from_secs(5)).unwrap() {
        AsyncMessage::ServiceCommandErr {
            instance_id,
            req_id,
            payload,
            on_err,
            message,
            ..
        } => {
            assert_eq!(instance_id, 7);
            assert_eq!(req_id, 19);
            assert!(payload.is_none());
            assert_eq!(on_err.unwrap().id, failure_action.id);
            assert!(message
                .unwrap()
                .contains("deliberate result serialization failure"));
        }
        other => panic!("expected serialization failure, got {other:?}"),
    }
    handle.control_tx.send(ServiceControlMessage::Stop).unwrap();
    assert!(matches!(
        rx.recv_timeout(Duration::from_secs(5)).unwrap(),
        AsyncMessage::ServiceStopped { .. }
    ));
}
