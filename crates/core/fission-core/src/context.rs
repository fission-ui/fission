use crate::action::{Action, ActionEnvelope};
use crate::effect::{Effect, EffectEnvelope, EffectPayload, ActionInput, SystemEffect};
use crate::registry::ActionRegistry;
use serde::Serialize;
use std::marker::PhantomData;

// Context passed to action handlers (reducers).
// 'a: lifetime of the reference to Effects
// 'b: lifetime of the Registry borrow inside Effects
pub struct ReducerContext<'a, 'b, S: crate::AppState> {
    pub effects: &'a mut Effects<'b, S>,
    pub input: &'a ActionInput,
}

// Effects builder/queue.
pub struct Effects<'a, S: crate::AppState> {
    pub(crate) next_req_id: u64,
    pub(crate) out: Vec<EffectEnvelope>,
    pub(crate) registry: Option<&'a mut ActionRegistry<S>>, 
    _phantom: PhantomData<S>,
}

impl<'a, S: crate::AppState> Effects<'a, S> {
    pub fn new(next_req_id: u64, registry: &'a mut ActionRegistry<S>) -> Self {
        Self {
            next_req_id,
            out: Vec::new(),
            registry: Some(registry),
            _phantom: PhantomData,
        }
    }
    
    pub fn new_headless(next_req_id: u64) -> Self {
        Self {
            next_req_id,
            out: Vec::new(),
            registry: None,
            _phantom: PhantomData,
        }
    }

    pub fn http_get(&mut self, url: impl Into<String>) -> EffectBuilder<'_, 'a, S> {
        self.system_effect(SystemEffect::HttpGet { 
            url: url.into(), 
            headers: vec![] 
        })
    }
    
    pub fn file_read(&mut self, path: impl Into<String>) -> EffectBuilder<'_, 'a, S> {
        self.system_effect(SystemEffect::FileRead { 
            path: path.into() 
        })
    }
    
    pub fn cancel(&mut self, req_id: u64) {
        // Cancellation is typically fire-and-forget
        self.system_effect(SystemEffect::Cancel { req_id }).dispatch();
    }
    
    pub fn release_resource(&mut self, resource_id: u64) {
        self.system_effect(SystemEffect::ReleaseResource { resource_id }).dispatch();
    }

    pub fn system_effect(&mut self, effect: SystemEffect) -> EffectBuilder<'_, 'a, S> {
        let req_id = self.next_req_id;
        self.next_req_id += 1;
        
        EffectBuilder {
            fx: self,
            envelope: EffectEnvelope {
                req_id,
                effect: Effect::System(effect),
                on_ok: None,
                on_err: None,
            }
        }
    }
    
    pub fn app_effect<E: Serialize>(&mut self, effect: E) -> EffectBuilder<'_, 'a, S> {
        let req_id = self.next_req_id;
        self.next_req_id += 1;
        
        let payload = serde_json::to_vec(&effect).expect("Failed to serialize app effect");
        
        EffectBuilder {
            fx: self,
            envelope: EffectEnvelope {
                req_id,
                effect: Effect::App(payload),
                on_ok: None,
                on_err: None,
            }
        }
    }
    
    // Bind helper
    
    pub fn bind<A: Action, H>(&mut self, action: A, handler: H) -> ActionEnvelope 
    where H: crate::registry::IntoHandler<S, A> + Send + Sync + 'static 
    {
        if let Some(registry) = &mut self.registry {
            registry.register(handler);
        }
        
        ActionEnvelope {
            id: A::static_id(),
            payload: action.encode(),
        }
    }
}

pub struct EffectBuilder<'a, 'b, S: crate::AppState> {
    fx: &'a mut Effects<'b, S>,
    envelope: EffectEnvelope,
}

impl<'a, 'b, S: crate::AppState> EffectBuilder<'a, 'b, S> {
    pub fn on_ok(mut self, action: ActionEnvelope) -> Self {
        self.envelope.on_ok = Some(action);
        self
    }
    
    pub fn on_err(mut self, action: ActionEnvelope) -> Self {
        self.envelope.on_err = Some(action);
        self
    }
    
    pub fn dispatch(self) {
        self.fx.out.push(self.envelope);
    }
}
