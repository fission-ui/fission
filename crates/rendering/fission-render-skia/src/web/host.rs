use std::fmt;

/// Synchronous, ownership-safe boundary between Rust-Wasm and the CanvasKit
/// executor owned by the browser shell.
///
/// Both buffers are owned deliberately. The host must not retain a view into
/// Rust-Wasm memory after `exchange` returns, and Rust never borrows memory
/// owned by JavaScript. One request produces exactly one Ack or Error packet.
/// Browser lifecycle events use the same owned response-packet shape and are
/// drained separately so they cannot silently advance the executor's response
/// sequence behind the Rust driver.
pub trait CanvasKitHost {
    type Error: fmt::Display;

    fn exchange(&mut self, request: Vec<u8>) -> Result<Vec<u8>, Self::Error>;

    /// Returns the next packet emitted by the executor's lifecycle event sink.
    ///
    /// A host that installs `createCanvasKitExecutor`'s `eventSink` must retain
    /// each owned packet until this method returns it. Hosts without an event
    /// sink may use the default; synchronous commands still report a lost
    /// context as their ordinary Error response.
    fn poll_lifecycle_event(&mut self) -> Result<Option<Vec<u8>>, Self::Error> {
        Ok(None)
    }
}

impl<F, E> CanvasKitHost for F
where
    F: FnMut(Vec<u8>) -> Result<Vec<u8>, E>,
    E: fmt::Display,
{
    type Error = E;

    fn exchange(&mut self, request: Vec<u8>) -> Result<Vec<u8>, Self::Error> {
        self(request)
    }
}
