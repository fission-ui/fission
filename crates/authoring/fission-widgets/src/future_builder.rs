use fission_core::build::{self, BuildCtxHandle, ViewHandle};
use fission_core::{
    ActionEnvelope, GlobalState, JobRef, JobResource, JobSpec, ResourceKey, ResourcePolicy, Widget,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AsyncConnectionState {
    /// No async work is currently connected to the builder.
    None,
    /// Work has been declared and is waiting for a result.
    Waiting,
    /// Work is still producing values or progress.
    Active,
    /// Work completed with either data or an error.
    Done,
}

impl Default for AsyncConnectionState {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AsyncSnapshot<T, E> {
    /// Current async connection state.
    pub connection_state: AsyncConnectionState,
    /// Last successful value, when one is available.
    pub data: Option<T>,
    /// Last error value, when one is available.
    pub error: Option<E>,
}

impl<T, E> AsyncSnapshot<T, E> {
    pub fn none() -> Self {
        Self::nothing()
    }

    pub fn nothing() -> Self {
        Self {
            connection_state: AsyncConnectionState::None,
            data: None,
            error: None,
        }
    }

    pub fn waiting() -> Self {
        Self {
            connection_state: AsyncConnectionState::Waiting,
            data: None,
            error: None,
        }
    }

    pub fn with_data(connection_state: AsyncConnectionState, data: T) -> Self {
        Self {
            connection_state,
            data: Some(data),
            error: None,
        }
    }

    pub fn with_error(connection_state: AsyncConnectionState, error: E) -> Self {
        Self {
            connection_state,
            data: None,
            error: Some(error),
        }
    }

    pub fn in_state(mut self, connection_state: AsyncConnectionState) -> Self {
        self.connection_state = connection_state;
        self
    }

    pub fn has_data(&self) -> bool {
        self.data.is_some()
    }

    pub fn has_error(&self) -> bool {
        self.error.is_some()
    }

    pub fn data(&self) -> Option<&T> {
        self.data.as_ref()
    }

    pub fn error(&self) -> Option<&E> {
        self.error.as_ref()
    }

    pub fn require_data(&self) -> &T {
        self.data
            .as_ref()
            .expect("AsyncSnapshot::require_data called without data")
    }
}

pub type AsyncWidgetBuilder<S, T, E> = Arc<
    dyn Fn(BuildCtxHandle<S>, ViewHandle<S>, &AsyncSnapshot<T, E>) -> Widget
        + Send
        + Sync
        + 'static,
>;

/// Declares one async job resource and builds UI from the current snapshot.
///
/// The snapshot belongs in application state. Successful and failed job
/// callbacks should update that state; this widget then renders the latest
/// snapshot during the next build.
pub struct FutureBuilder<S, J>
where
    S: GlobalState,
    J: JobSpec,
{
    pub key: ResourceKey,
    pub job: JobRef<J>,
    pub request: J::Request,
    pub snapshot: AsyncSnapshot<J::Ok, J::Err>,
    pub on_ok: Option<ActionEnvelope>,
    pub on_err: Option<ActionEnvelope>,
    pub deps: Option<Vec<u8>>,
    pub policy: ResourcePolicy,
    pub builder: AsyncWidgetBuilder<S, J::Ok, J::Err>,
}

impl<S, J> FutureBuilder<S, J>
where
    S: GlobalState,
    J: JobSpec,
{
    pub fn new<F>(
        key: ResourceKey,
        job: JobRef<J>,
        request: J::Request,
        snapshot: AsyncSnapshot<J::Ok, J::Err>,
        builder: F,
    ) -> Self
    where
        F: Fn(BuildCtxHandle<S>, ViewHandle<S>, &AsyncSnapshot<J::Ok, J::Err>) -> Widget
            + Send
            + Sync
            + 'static,
    {
        Self {
            key,
            job,
            request,
            snapshot,
            on_ok: None,
            on_err: None,
            deps: None,
            policy: ResourcePolicy::RestartOnChange,
            builder: Arc::new(builder),
        }
    }

    pub fn on_ok(mut self, action: ActionEnvelope) -> Self {
        self.on_ok = Some(action);
        self
    }

    pub fn on_err(mut self, action: ActionEnvelope) -> Self {
        self.on_err = Some(action);
        self
    }

    pub fn deps<T: Serialize>(mut self, deps: T) -> Self {
        self.deps =
            Some(serde_json::to_vec(&deps).expect("FutureBuilder deps serialization must succeed"));
        self
    }

    pub fn preserve_on_change(mut self) -> Self {
        self.policy = ResourcePolicy::PreserveOnChange;
        self
    }

    pub fn restart_on_change(mut self) -> Self {
        self.policy = ResourcePolicy::RestartOnChange;
        self
    }
}

impl<S, J> From<FutureBuilder<S, J>> for Widget
where
    S: GlobalState,
    J: JobSpec,
    J::Request: Clone,
{
    fn from(component: FutureBuilder<S, J>) -> Self {
        let (ctx, view) = build::current::<S>();
        let this = &component;

        let mut resource = JobResource::new(this.key.clone(), this.job, this.request.clone());
        resource.policy = this.policy;
        resource.deps = this.deps.clone();
        if let Some(action) = &this.on_ok {
            resource = resource.on_ok(action.clone());
        }
        if let Some(action) = &this.on_err {
            resource = resource.on_err(action.clone());
        }
        ctx.with_resources(|resources| resources.job(resource));

        (this.builder)(ctx, view, &this.snapshot)
    }
}
