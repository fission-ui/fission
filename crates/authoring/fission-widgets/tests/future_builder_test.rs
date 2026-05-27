use fission_core::{
    Action, AppState, BuildCtx, Effect, JobRef, JobSpec, Node, ResourceKey, RuntimeResourceKind,
    View, Widget,
};
use fission_widgets::{AsyncConnectionState, AsyncSnapshot, FutureBuilder, Text, TextContent};
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone)]
struct State;
impl AppState for State {}

#[derive(Debug)]
struct LoadMessage;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct LoadMessageRequest {
    id: u32,
}

impl JobSpec for LoadMessage {
    type Request = LoadMessageRequest;
    type Ok = String;
    type Err = String;

    const NAME: &'static str = "load-message";
}

const LOAD_MESSAGE: JobRef<LoadMessage> = JobRef::new("load-message");

#[fission_macros::fission_action]
struct MessageLoaded;

#[test]
fn async_snapshot_helpers_report_state() {
    let empty = AsyncSnapshot::<String, String>::none();
    assert_eq!(empty.connection_state, AsyncConnectionState::None);
    assert!(!empty.has_data());
    assert!(!empty.has_error());

    let loaded =
        AsyncSnapshot::<String, String>::with_data(AsyncConnectionState::Done, "ready".to_string());
    assert_eq!(loaded.data(), Some(&"ready".to_string()));
    assert_eq!(loaded.require_data(), "ready");
}

#[test]
fn future_builder_declares_job_and_builds_from_snapshot() {
    let env = fission_core::Env::default();
    let runtime = fission_core::RuntimeState::default();
    let state = State;
    let view = View::new(&state, &runtime, &env, None);
    let mut ctx = BuildCtx::<State>::new();

    let loaded = MessageLoaded;
    let loaded_action = fission_core::ActionEnvelope {
        id: MessageLoaded::static_id(),
        payload: loaded.encode(),
    };

    let node = FutureBuilder::new(
        ResourceKey::new("message"),
        LOAD_MESSAGE,
        LoadMessageRequest { id: 42 },
        AsyncSnapshot::<String, String>::waiting(),
        |_ctx, _view, snapshot| Text::new(format!("{:?}", snapshot.connection_state)).into_node(),
    )
    .deps(("message", 42_u32))
    .preserve_on_change()
    .on_ok(loaded_action.clone())
    .build(&mut ctx, &view);

    let Node::Text(text) = node else {
        panic!("FutureBuilder should render the node returned by its builder");
    };
    assert_eq!(text.content, TextContent::Literal("Waiting".to_string()));

    let resources = ctx.take_resources();
    assert_eq!(resources.len(), 1);
    assert_eq!(resources[0].key, "message");
    assert_eq!(
        resources[0].policy,
        fission_core::ResourcePolicy::PreserveOnChange
    );
    assert!(resources[0].deps.is_some());

    let RuntimeResourceKind::Job(job) = &resources[0].kind else {
        panic!("FutureBuilder should declare a job resource");
    };

    let Effect::Job(request) = &job.effect.effect else {
        panic!("FutureBuilder should emit a job effect");
    };
    assert_eq!(request.job_name, "load-message");
    assert_eq!(job.effect.on_ok.as_ref(), Some(&loaded_action));
}
