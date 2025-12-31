use crate::{
    Action, ActionEnvelope, ActionId, AppState, BuildCtx, Effects, Effect, SystemEffect, ReducerContext, ActionInput, EffectPayload,
};
use crate::runtime::Runtime;
use crate::registry::Handler;
use serde::{Deserialize, Serialize};
use std::any::Any;
use anyhow::Result;

#[derive(Default, Debug, Clone)]
struct TestState {
    data: String,
    loading: bool,
}
impl AppState for TestState {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct FetchData;

impl Action for FetchData {
    fn static_id() -> ActionId { ActionId::from_name("FetchData") }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct DataLoaded; 

impl Action for DataLoaded {
    fn static_id() -> ActionId { ActionId::from_name("DataLoaded") }
}

fn on_fetch<'a, 'b>(state: &mut TestState, _: FetchData, ctx: &mut ReducerContext<'a, 'b, TestState>) {
    state.loading = true;
    let on_ok = ctx.effects.bind(DataLoaded, on_loaded as Handler<TestState, DataLoaded>);
    ctx.effects.http_get("https://example.com")
        .on_ok(on_ok)
        .dispatch();
}

fn on_loaded<'a, 'b>(state: &mut TestState, _: DataLoaded, ctx: &mut ReducerContext<'a, 'b, TestState>) {
    state.loading = false;
    if let Some(bytes) = ctx.input.as_bytes() {
        state.data = String::from_utf8(bytes.to_vec()).unwrap();
    }
}

#[test]
fn test_effect_loop() -> Result<()> {
    let mut runtime = Runtime::default();
    runtime.add_app_state(Box::new(TestState::default()))?;
    
    // Register actions
    let mut registry = crate::registry::ActionRegistry::new();
    registry.register(on_fetch as Handler<TestState, FetchData>);
    registry.register(on_loaded as Handler<TestState, DataLoaded>);
    runtime.absorb_registry(registry);
    
    // 1. Dispatch Fetch
    let action = FetchData;
    runtime.dispatch(ActionEnvelope { id: FetchData::static_id(), payload: action.encode() }, crate::NodeId::from_u128(0))?;
    
    // Assert State (Loading)
    let state = runtime.get_app_state::<TestState>().unwrap();
    assert!(state.loading);
    assert_eq!(state.data, "");
    
    // Assert Effect Emitted
    assert_eq!(runtime.pending_effects.len(), 1);
    let env = runtime.pending_effects.pop().unwrap();
    if let Effect::System(SystemEffect::HttpGet { url, .. }) = env.effect {
        assert_eq!(url, "https://example.com");
    } else {
        panic!("Expected HttpGet");
    }
    
    // 2. Simulate Host Response
    let result_bytes = b"Hello World".to_vec();
    let on_ok = env.on_ok.unwrap();
    
    // Dispatch Continuation
    let input = ActionInput::EffectOk { req_id: env.req_id, payload: EffectPayload::InlineBytes(result_bytes) };
    runtime.dispatch_with_input(on_ok, crate::NodeId::from_u128(0), &input)?;
    
    // Assert State (Loaded)
    let state = runtime.get_app_state::<TestState>().unwrap();
    assert!(!state.loading);
    assert_eq!(state.data, "Hello World");
    
    Ok(())
}