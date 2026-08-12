use std::fmt;

/// Synchronous, ownership-safe boundary between Rust-Wasm and the CanvasKit
/// executor owned by the browser shell.
///
/// Both buffers are owned deliberately. The host must not retain a view into
/// Rust-Wasm memory after `exchange` returns, and Rust never borrows memory
/// owned by JavaScript. One request produces exactly one Ack or Error packet.
pub trait CanvasKitHost {
    type Error: fmt::Display;

    fn exchange(&mut self, request: Vec<u8>) -> Result<Vec<u8>, Self::Error>;
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
