use crate::action::ActionEnvelope;
use crate::data_stream::{
    BoxFissionDataStream, DataStreamId, DataStreamRegistry, FissionDataStreamError,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::borrow::Cow;
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::Arc;

pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

pub trait JobSpec {
    type Request: Serialize + DeserializeOwned + Send + 'static;
    type Ok: Serialize + DeserializeOwned + Send + 'static;
    type Err: Serialize + DeserializeOwned + Send + 'static;
    const NAME: &'static str;
}

#[derive(Debug)]
pub struct JobRef<J: JobSpec> {
    pub name: &'static str,
    _marker: PhantomData<fn() -> J>,
}

impl<J: JobSpec> JobRef<J> {
    pub const fn new(name: &'static str) -> Self {
        Self {
            name,
            _marker: PhantomData,
        }
    }
}

impl<J: JobSpec> Clone for JobRef<J> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<J: JobSpec> Copy for JobRef<J> {}

pub trait ServiceSpec {
    type Config: Serialize + DeserializeOwned + Send + 'static;
    type Command: Serialize + DeserializeOwned + Send + 'static;
    type CommandOk: Serialize + DeserializeOwned + Send + 'static;
    type CommandErr: Serialize + DeserializeOwned + Send + 'static;
    type Event: Serialize + DeserializeOwned + Send + 'static;
    type StartErr: Serialize + DeserializeOwned + Send + 'static;
    const NAME: &'static str;
}

#[derive(Debug)]
pub struct ServiceType<S: ServiceSpec> {
    pub name: &'static str,
    _marker: PhantomData<fn() -> S>,
}

impl<S: ServiceSpec> ServiceType<S> {
    pub const fn new(name: &'static str) -> Self {
        Self {
            name,
            _marker: PhantomData,
        }
    }
}

impl<S: ServiceSpec> Clone for ServiceType<S> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<S: ServiceSpec> Copy for ServiceType<S> {}

#[derive(Debug)]
pub struct ServiceSlot<S: ServiceSpec> {
    pub ty: ServiceType<S>,
    pub slot_key: Cow<'static, str>,
}

impl<S: ServiceSpec> ServiceSlot<S> {
    pub fn singleton(ty: ServiceType<S>) -> Self {
        Self {
            ty,
            slot_key: Cow::Borrowed("singleton"),
        }
    }

    pub fn keyed(ty: ServiceType<S>, key: impl Into<String>) -> Self {
        Self {
            ty,
            slot_key: Cow::Owned(key.into()),
        }
    }

    pub fn slot_key(&self) -> &str {
        self.slot_key.as_ref()
    }
}

impl<S: ServiceSpec> Clone for ServiceSlot<S> {
    fn clone(&self) -> Self {
        Self {
            ty: self.ty,
            slot_key: self.slot_key.clone(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct JobRequestPayload {
    pub job_name: String,
    pub payload: Vec<u8>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServiceStartPayload {
    pub service_name: String,
    pub slot_key: String,
    pub config: Vec<u8>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServiceCommandPayload {
    pub service_name: String,
    pub slot_key: String,
    pub payload: Vec<u8>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServiceStopPayload {
    pub service_name: String,
    pub slot_key: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ServiceBindings {
    pub on_started: Option<ActionEnvelope>,
    pub on_start_failed: Option<ActionEnvelope>,
    pub on_event: Option<ActionEnvelope>,
    pub on_stopped: Option<ActionEnvelope>,
    pub on_command_ok: Option<ActionEnvelope>,
    pub on_command_err: Option<ActionEnvelope>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResourceExecutionContext {
    pub key: String,
    pub generation: u64,
}

#[derive(Clone, Debug)]
pub struct JobCtx {
    pub req_id: u64,
    data_streams: DataStreamRegistry,
}

impl JobCtx {
    #[doc(hidden)]
    pub fn new_runtime(req_id: u64, data_streams: DataStreamRegistry) -> Self {
        Self {
            req_id,
            data_streams,
        }
    }

    /// Opens a runtime-owned data stream for this job.
    ///
    /// Streams are one-shot by default; opening consumes the stream handle.
    pub fn open_data_stream(
        &self,
        id: DataStreamId,
    ) -> Result<BoxFissionDataStream, FissionDataStreamError> {
        self.data_streams.open(id)
    }

    /// Releases a stream without consuming it.
    pub fn release_data_stream(&self, id: DataStreamId) -> bool {
        self.data_streams.release(id)
    }
}

type EmitFn = dyn Fn(Vec<u8>) -> BoxFuture<Result<(), String>> + Send + Sync;

struct ServiceCtxInner {
    service_name: String,
    slot_key: String,
    instance_id: u64,
    data_streams: DataStreamRegistry,
    emit: Arc<EmitFn>,
}

pub struct ServiceCtx<S: ServiceSpec> {
    inner: Arc<ServiceCtxInner>,
    _marker: PhantomData<fn() -> S>,
}

impl<S: ServiceSpec> std::fmt::Debug for ServiceCtx<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServiceCtx")
            .field("service_name", &self.inner.service_name)
            .field("slot_key", &self.inner.slot_key)
            .field("instance_id", &self.inner.instance_id)
            .finish()
    }
}

impl<S: ServiceSpec> Clone for ServiceCtx<S> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            _marker: PhantomData,
        }
    }
}

impl<S: ServiceSpec> ServiceCtx<S> {
    #[doc(hidden)]
    pub fn new_runtime(
        service_name: String,
        slot_key: String,
        instance_id: u64,
        data_streams: DataStreamRegistry,
        emit: Arc<EmitFn>,
    ) -> Self {
        Self {
            inner: Arc::new(ServiceCtxInner {
                service_name,
                slot_key,
                instance_id,
                data_streams,
                emit,
            }),
            _marker: PhantomData,
        }
    }

    pub fn service_name(&self) -> &str {
        &self.inner.service_name
    }

    pub fn slot_key(&self) -> &str {
        &self.inner.slot_key
    }

    pub fn instance_id(&self) -> u64 {
        self.inner.instance_id
    }

    pub fn emit(&self, event: S::Event) -> BoxFuture<Result<(), String>> {
        match serde_json::to_vec(&event) {
            Ok(bytes) => (self.inner.emit)(bytes),
            Err(err) => Box::pin(async move { Err(err.to_string()) }),
        }
    }

    /// Opens a runtime-owned data stream for this service.
    ///
    /// Streams are one-shot by default; opening consumes the stream handle.
    pub fn open_data_stream(
        &self,
        id: DataStreamId,
    ) -> Result<BoxFissionDataStream, FissionDataStreamError> {
        self.inner.data_streams.open(id)
    }

    /// Releases a stream without consuming it.
    pub fn release_data_stream(&self, id: DataStreamId) -> bool {
        self.inner.data_streams.release(id)
    }
}

pub trait ServiceRunner<S: ServiceSpec>: Send + 'static {
    fn on_command(
        &mut self,
        command: S::Command,
        ctx: ServiceCtx<S>,
    ) -> BoxFuture<Result<S::CommandOk, S::CommandErr>>;

    fn on_stop(self: Box<Self>, ctx: ServiceCtx<S>) -> BoxFuture<()>;
}
