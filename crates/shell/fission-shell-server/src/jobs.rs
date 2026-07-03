use anyhow::{anyhow, Result as AnyResult};
use fission_core::{
    collect_data_stream, BoxFissionDataStream, Bytes, DataStreamId, DataStreamRegistry,
    FissionDataStreamError, JobRef, JobSpec,
};
use serde::Serialize;
use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

#[derive(Clone, Debug)]
pub struct ServerJobCtx {
    pub req_id: u64,
    pub resource_key: String,
    data_streams: DataStreamRegistry,
}

impl PartialEq for ServerJobCtx {
    fn eq(&self, other: &Self) -> bool {
        self.req_id == other.req_id && self.resource_key == other.resource_key
    }
}

impl Eq for ServerJobCtx {}

impl ServerJobCtx {
    #[doc(hidden)]
    pub fn new_runtime(
        req_id: u64,
        resource_key: impl Into<String>,
        data_streams: DataStreamRegistry,
    ) -> Self {
        Self {
            req_id,
            resource_key: resource_key.into(),
            data_streams,
        }
    }

    /// Opens a runtime-owned data stream for this server job.
    ///
    /// Streams are one-shot by default; opening consumes the stream handle.
    pub fn open_data_stream(
        &self,
        id: DataStreamId,
    ) -> Result<BoxFissionDataStream, FissionDataStreamError> {
        self.data_streams.open(id)
    }

    /// Collects a data stream in a synchronous server job.
    ///
    /// Server jobs are currently synchronous. This helper is intended for host
    /// streams that can make progress from normal `Stream` wakeups without a
    /// platform-specific async reactor.
    pub fn collect_data_stream(&self, id: DataStreamId) -> Result<Bytes, FissionDataStreamError> {
        let stream = self.open_data_stream(id)?;
        block_on_stream(collect_data_stream(stream))
    }

    /// Releases a stream without consuming it.
    pub fn release_data_stream(&self, id: DataStreamId) -> bool {
        self.data_streams.release(id)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerJobError {
    pub payload: Option<Vec<u8>>,
    pub message: Option<String>,
}

impl ServerJobError {
    pub fn message(message: impl Into<String>) -> Self {
        Self {
            payload: None,
            message: Some(message.into()),
        }
    }

    pub fn typed<E: Serialize>(error: &E) -> Self {
        Self {
            payload: serde_json::to_vec(error).ok(),
            message: None,
        }
    }
}

type JobHandler =
    dyn Fn(Vec<u8>, ServerJobCtx) -> std::result::Result<Vec<u8>, ServerJobError> + Send + Sync;

#[derive(Clone, Default)]
pub struct ServerJobRegistry {
    handlers: BTreeMap<String, Arc<JobHandler>>,
    data_streams: DataStreamRegistry,
}

impl ServerJobRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn data_streams(&self) -> DataStreamRegistry {
        self.data_streams.clone()
    }

    pub fn register_data_stream(&self, stream: BoxFissionDataStream) -> DataStreamId {
        self.data_streams.register(stream)
    }

    pub fn open_data_stream(
        &self,
        id: DataStreamId,
    ) -> Result<BoxFissionDataStream, FissionDataStreamError> {
        self.data_streams.open(id)
    }

    pub fn release_data_stream(&self, id: DataStreamId) -> bool {
        self.data_streams.release(id)
    }

    pub fn register_job<J, F>(mut self, job: JobRef<J>, handler: F) -> Self
    where
        J: JobSpec,
        F: Fn(J::Request, ServerJobCtx) -> std::result::Result<J::Ok, J::Err>
            + Send
            + Sync
            + 'static,
        J::Err: Serialize,
    {
        self.handlers.insert(
            job.name.to_string(),
            Arc::new(move |payload, ctx| {
                let request = serde_json::from_slice::<J::Request>(&payload)
                    .map_err(|error| ServerJobError::message(error.to_string()))?;
                match handler(request, ctx) {
                    Ok(value) => serde_json::to_vec(&value)
                        .map_err(|error| ServerJobError::message(error.to_string())),
                    Err(error) => Err(ServerJobError::typed(&error)),
                }
            }),
        );
        self
    }

    pub fn has_job(&self, name: &str) -> bool {
        self.handlers.contains_key(name)
    }

    pub fn run(
        &self,
        name: &str,
        payload: Vec<u8>,
        ctx: ServerJobCtx,
    ) -> std::result::Result<Vec<u8>, ServerJobError> {
        let Some(handler) = self.handlers.get(name) else {
            return Err(ServerJobError::message(format!(
                "server job `{name}` is not registered"
            )));
        };
        handler(payload, ctx)
    }

    pub fn require_job(&self, name: &str) -> AnyResult<()> {
        if self.has_job(name) {
            Ok(())
        } else {
            Err(anyhow!("server job `{name}` is not registered"))
        }
    }
}

fn block_on_stream<F: Future>(future: F) -> F::Output {
    let Parker { waker, state } = Parker::new();
    let mut cx = Context::from_waker(&waker);
    let mut future = Box::pin(future);
    loop {
        match Pin::as_mut(&mut future).poll(&mut cx) {
            Poll::Ready(value) => return value,
            Poll::Pending => state.park(),
        }
    }
}

struct Parker {
    waker: Waker,
    state: Arc<ParkerState>,
}

struct ParkerState {
    signalled: std::sync::Mutex<bool>,
    cv: std::sync::Condvar,
}

impl Parker {
    fn new() -> Self {
        let state = Arc::new(ParkerState {
            signalled: std::sync::Mutex::new(false),
            cv: std::sync::Condvar::new(),
        });
        let waker =
            unsafe { Waker::from_raw(raw_waker(Arc::into_raw(state.clone()) as *const ())) };
        Self { waker, state }
    }
}

impl ParkerState {
    fn park(&self) {
        let mut signalled = self.signalled.lock().expect("server stream waker poisoned");
        while !*signalled {
            signalled = self
                .cv
                .wait(signalled)
                .expect("server stream waker poisoned");
        }
        *signalled = false;
    }

    fn wake(&self) {
        let mut signalled = self.signalled.lock().expect("server stream waker poisoned");
        *signalled = true;
        self.cv.notify_one();
    }
}

unsafe fn raw_waker(data: *const ()) -> RawWaker {
    RawWaker::new(data, &VTABLE)
}

unsafe fn clone_waker(data: *const ()) -> RawWaker {
    let arc = Arc::<ParkerState>::from_raw(data as *const ParkerState);
    let cloned = arc.clone();
    let _ = Arc::into_raw(arc);
    raw_waker(Arc::into_raw(cloned) as *const ())
}

unsafe fn wake(data: *const ()) {
    let arc = Arc::<ParkerState>::from_raw(data as *const ParkerState);
    arc.wake();
}

unsafe fn wake_by_ref(data: *const ()) {
    let arc = Arc::<ParkerState>::from_raw(data as *const ParkerState);
    arc.wake();
    let _ = Arc::into_raw(arc);
}

unsafe fn drop_waker(data: *const ()) {
    let _ = Arc::<ParkerState>::from_raw(data as *const ParkerState);
}

static VTABLE: RawWakerVTable = RawWakerVTable::new(clone_waker, wake, wake_by_ref, drop_waker);

#[cfg(test)]
mod tests {
    use super::*;
    use fission_core::{single_chunk_data_stream, FissionDataStreamErrorKind};
    use serde::Deserialize;

    #[derive(Debug)]
    struct StreamJob;

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    struct StreamRequest {
        stream: DataStreamId,
    }

    impl JobSpec for StreamJob {
        type Request = StreamRequest;
        type Ok = String;
        type Err = String;
        const NAME: &'static str = "server.stream";
    }

    const STREAM_JOB: JobRef<StreamJob> = JobRef::new(StreamJob::NAME);

    #[test]
    fn server_jobs_can_collect_runtime_streams() {
        let registry = ServerJobRegistry::new().register_job(
            STREAM_JOB,
            |request: StreamRequest, ctx: ServerJobCtx| {
                let bytes = ctx
                    .collect_data_stream(request.stream)
                    .map_err(|error| error.to_string())?;
                String::from_utf8(bytes.to_vec()).map_err(|error| error.to_string())
            },
        );
        let stream = registry.register_data_stream(single_chunk_data_stream("hello server"));
        let payload = serde_json::to_vec(&StreamRequest { stream }).unwrap();
        let result = registry
            .run(
                STREAM_JOB.name,
                payload,
                ServerJobCtx::new_runtime(7, "test", registry.data_streams()),
            )
            .unwrap();

        let value: String = serde_json::from_slice(&result).unwrap();
        assert_eq!(value, "hello server");
    }

    #[test]
    fn server_job_stream_handles_are_one_shot() {
        let registry = ServerJobRegistry::new();
        let stream = registry.register_data_stream(single_chunk_data_stream("once"));
        let ctx = ServerJobCtx::new_runtime(7, "test", registry.data_streams());

        assert_eq!(ctx.collect_data_stream(stream).unwrap().as_ref(), b"once");
        match ctx.open_data_stream(stream) {
            Ok(_) => panic!("stream should be consumed"),
            Err(error) => assert_eq!(error.kind, FissionDataStreamErrorKind::AlreadyConsumed),
        }
    }
}
