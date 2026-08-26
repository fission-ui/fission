//! Public application-side SDK for Fission Developer generation workers.

#![forbid(unsafe_code)]

mod app;
mod protocol;
mod remote_surface;

pub use app::{run, DevtoolsApp};
pub use protocol::{
    decode_line, encode_line, AppAction, AppFrame, DispatchResult, StateSnapshot, WorkerCommand,
    WorkerError, WorkerHandshake, WorkerOutput, WorkerRequest, WorkerResponse,
    APP_WORKER_PROTOCOL_VERSION,
};
pub use remote_surface::RemoteAppSurface;

#[macro_export]
macro_rules! devtools_main {
    ($factory:path) => {
        fn main() -> ::core::result::Result<(), Box<dyn ::std::error::Error>> {
            $crate::run($factory()).map_err(Into::into)
        }
    };
}

#[cfg(test)]
mod tests {
    use fission_core::ui::Text;
    use fission_core::{GlobalState, Widget};
    use serde::{Deserialize, Serialize};

    use super::*;

    #[derive(Debug, Default, Serialize, Deserialize)]
    struct State {
        message: String,
    }

    impl GlobalState for State {}

    #[derive(Clone)]
    struct Root;

    impl From<Root> for Widget {
        fn from(_: Root) -> Self {
            let (_, view) = fission_core::build::current::<State>();
            Text::new(view.state().message.clone()).into()
        }
    }

    fn app(message: &str) -> DevtoolsApp<State, Root> {
        DevtoolsApp::new(
            "fission.test.app",
            "Test app",
            "state/v1",
            State {
                message: message.into(),
            },
            Root,
        )
    }

    #[test]
    fn build_emits_real_fission_ir_and_snapshot_restores_state() {
        let mut first = app::AppWorker::new(app("first"), 1).unwrap();
        let WorkerOutput::Frame(frame) = first.handle(WorkerCommand::Build).unwrap() else {
            panic!("build must return a frame");
        };
        assert!(!frame.ir.nodes.is_empty());
        let WorkerOutput::Snapshot(snapshot) = first.handle(WorkerCommand::Snapshot).unwrap()
        else {
            panic!("snapshot must be returned");
        };

        let mut second = app::AppWorker::new(app("replacement"), 2).unwrap();
        second.handle(WorkerCommand::Restore { snapshot }).unwrap();
        let WorkerOutput::Frame(frame) = second.handle(WorkerCommand::Build).unwrap() else {
            panic!("build must return a frame");
        };
        assert_eq!(frame.generation, 2);
        assert!(frame.ir.nodes.values().any(|node| matches!(
            &node.op,
            fission_ir::Op::Paint(fission_ir::PaintOp::DrawText { text, .. }) if text == "first"
        )));

        let response = WorkerResponse {
            request_id: 7,
            result: Ok(WorkerOutput::Frame(frame)),
        };
        let encoded = encode_line(&response).expect("worker response should encode");
        let decoded: WorkerResponse = decode_line(&encoded).unwrap_or_else(|error| {
            let column = error.column();
            let start = column.saturating_sub(120);
            let end = (column + 120).min(encoded.len());
            panic!(
                "worker response should preserve 128-bit identities: {error}; nearby JSON: {}",
                &encoded[start..end]
            )
        });
        assert_eq!(decoded, response);
    }
}
