//! Runtime-owned streams for large binary data.
//!
//! Large data should not travel through action payloads or capability results as
//! `Vec<u8>`. Host capabilities register streams here, then reducers pass the
//! returned [`DataStreamId`] to jobs or services that consume the stream
//! asynchronously.

use bytes::{Bytes, BytesMut};
use futures_core::Stream;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

/// Unique identifier for a runtime-owned binary stream.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct DataStreamId(pub u64);

/// Machine-readable class of stream failure.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FissionDataStreamErrorKind {
    /// The stream id is unknown.
    NotFound,
    /// The stream was already opened by a previous consumer.
    AlreadyConsumed,
    /// The host or user cancelled the stream.
    Cancelled,
    /// The host denied access to the data.
    PermissionDenied,
    /// The host cannot provide this stream operation.
    Unsupported,
    /// I/O failed while reading the stream.
    Io,
    /// The data was malformed or could not be decoded by the producer.
    InvalidData,
    /// Any other host-specific failure.
    Other,
}

/// Error item yielded by [`FissionDataStream`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FissionDataStreamError {
    /// Stable error category.
    pub kind: FissionDataStreamErrorKind,
    /// Human-readable diagnostic message.
    pub message: String,
}

impl FissionDataStreamError {
    /// Creates a stream error.
    pub fn new(kind: FissionDataStreamErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    /// Creates a not-found stream error.
    pub fn not_found(id: DataStreamId) -> Self {
        Self::new(
            FissionDataStreamErrorKind::NotFound,
            format!("data stream {} was not found", id.0),
        )
    }

    /// Creates an already-consumed stream error.
    pub fn already_consumed(id: DataStreamId) -> Self {
        Self::new(
            FissionDataStreamErrorKind::AlreadyConsumed,
            format!("data stream {} was already consumed", id.0),
        )
    }
}

impl std::error::Error for FissionDataStreamError {}

impl fmt::Display for FissionDataStreamError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)
    }
}

impl From<std::io::Error> for FissionDataStreamError {
    fn from(error: std::io::Error) -> Self {
        Self::new(FissionDataStreamErrorKind::Io, error.to_string())
    }
}

/// Fission's framework-wide stream trait for large binary payloads.
///
/// It intentionally builds on `futures_core::Stream` because Rust does not
/// currently provide a stable `std::stream::Stream`.
pub trait FissionDataStream:
    Stream<Item = Result<Bytes, FissionDataStreamError>> + Send + 'static
{
}

impl<T> FissionDataStream for T where
    T: Stream<Item = Result<Bytes, FissionDataStreamError>> + Send + 'static
{
}

/// Boxed dynamic Fission data stream.
pub type BoxFissionDataStream = Pin<Box<dyn FissionDataStream>>;

/// Host/runtime registry for one-shot data streams.
#[derive(Clone, Default)]
pub struct DataStreamRegistry {
    inner: Arc<DataStreamRegistryInner>,
}

#[derive(Default)]
struct DataStreamRegistryInner {
    next_id: AtomicU64,
    streams: Mutex<HashMap<DataStreamId, Option<BoxFissionDataStream>>>,
}

impl fmt::Debug for DataStreamRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let len = self
            .inner
            .streams
            .lock()
            .map(|streams| streams.len())
            .unwrap_or_default();
        f.debug_struct("DataStreamRegistry")
            .field("streams", &len)
            .finish()
    }
}

impl DataStreamRegistry {
    /// Creates an empty stream registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a one-shot stream and returns its public id.
    pub fn register(&self, stream: BoxFissionDataStream) -> DataStreamId {
        let id = DataStreamId(self.inner.next_id.fetch_add(1, Ordering::Relaxed) + 1);
        self.inner
            .streams
            .lock()
            .expect("data stream registry lock poisoned")
            .insert(id, Some(stream));
        id
    }

    /// Opens the stream for consumption.
    ///
    /// Streams are single-consumer by default. Opening removes the stream from
    /// the registry slot so subsequent opens fail deterministically.
    pub fn open(&self, id: DataStreamId) -> Result<BoxFissionDataStream, FissionDataStreamError> {
        let mut streams = self.inner.streams.lock().map_err(|_| {
            FissionDataStreamError::new(
                FissionDataStreamErrorKind::Other,
                "data stream registry lock was poisoned",
            )
        })?;
        match streams.get_mut(&id) {
            Some(slot) => slot
                .take()
                .ok_or_else(|| FissionDataStreamError::already_consumed(id)),
            None => Err(FissionDataStreamError::not_found(id)),
        }
    }

    /// Releases a stream without consuming it.
    pub fn release(&self, id: DataStreamId) -> bool {
        self.inner
            .streams
            .lock()
            .map(|mut streams| streams.remove(&id).is_some())
            .unwrap_or(false)
    }
}

struct SingleChunkDataStream {
    chunk: Option<Result<Bytes, FissionDataStreamError>>,
}

impl Stream for SingleChunkDataStream {
    type Item = Result<Bytes, FissionDataStreamError>;

    fn poll_next(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Poll::Ready(self.chunk.take())
    }
}

/// Creates a stream that yields a single byte chunk.
pub fn single_chunk_data_stream(bytes: impl Into<Bytes>) -> BoxFissionDataStream {
    Box::pin(SingleChunkDataStream {
        chunk: Some(Ok(bytes.into())),
    })
}

/// Creates an empty stream.
pub fn empty_data_stream() -> BoxFissionDataStream {
    Box::pin(SingleChunkDataStream { chunk: None })
}

/// Collects a data stream into one contiguous byte buffer.
///
/// Use this only at shell boundaries that must interoperate with platform APIs
/// requiring contiguous memory. Application-facing large-data flows should keep
/// streaming chunks instead.
pub async fn collect_data_stream(
    mut stream: BoxFissionDataStream,
) -> Result<Bytes, FissionDataStreamError> {
    let mut out = BytesMut::new();
    while let Some(chunk) = std::future::poll_fn(|cx| stream.as_mut().poll_next(cx)).await {
        out.extend_from_slice(&chunk?);
    }
    Ok(out.freeze())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::future::Future;
    use std::task::{RawWaker, RawWakerVTable, Waker};

    fn noop_waker() -> Waker {
        unsafe fn clone(data: *const ()) -> RawWaker {
            RawWaker::new(data, &VTABLE)
        }
        unsafe fn wake(_data: *const ()) {}
        unsafe fn wake_by_ref(_data: *const ()) {}
        unsafe fn drop(_data: *const ()) {}

        static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, wake, wake_by_ref, drop);
        unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &VTABLE)) }
    }

    fn block_on_ready<F: Future>(future: F) -> F::Output {
        let waker = noop_waker();
        let mut cx = Context::from_waker(&waker);
        let mut future = Box::pin(future);
        match future.as_mut().poll(&mut cx) {
            Poll::Ready(output) => output,
            Poll::Pending => panic!("test future unexpectedly pending"),
        }
    }

    #[test]
    fn registry_opens_stream_once() {
        let registry = DataStreamRegistry::new();
        let id = registry.register(single_chunk_data_stream(Bytes::from_static(b"abc")));

        let bytes = block_on_ready(collect_data_stream(registry.open(id).unwrap())).unwrap();
        assert_eq!(bytes.as_ref(), b"abc");

        match registry.open(id) {
            Ok(_) => panic!("stream should be consumed"),
            Err(error) => assert_eq!(error.kind, FissionDataStreamErrorKind::AlreadyConsumed),
        }
    }

    #[test]
    fn registry_release_removes_unopened_stream() {
        let registry = DataStreamRegistry::new();
        let id = registry.register(empty_data_stream());

        assert!(registry.release(id));
        match registry.open(id) {
            Ok(_) => panic!("released stream should be removed"),
            Err(error) => assert_eq!(error.kind, FissionDataStreamErrorKind::NotFound),
        }
    }

    #[test]
    fn cloned_registries_share_streams() {
        let registry = DataStreamRegistry::new();
        let clone = registry.clone();
        let id = registry.register(single_chunk_data_stream(Bytes::from_static(b"shared")));

        let bytes = block_on_ready(collect_data_stream(clone.open(id).unwrap())).unwrap();
        assert_eq!(bytes.as_ref(), b"shared");
    }
}
