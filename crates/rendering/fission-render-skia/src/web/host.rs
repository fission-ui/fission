use std::fmt;

use fission_render::surface::{MemoryPressure, PhysicalSize};
use fission_skia_sys::web::ResourceHandle;

/// Integer pixel rectangle requested from the owning CanvasKit surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CanvasKitPixelRegion {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl CanvasKitPixelRegion {
    pub const fn size(self) -> PhysicalSize {
        PhysicalSize::new(self.width, self.height)
    }
}

/// Owned RGBA8888 pixels copied out of CanvasKit's Wasm heap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanvasKitReadback {
    pub size: PhysicalSize,
    pub row_bytes: usize,
    pub pixels: Vec<u8>,
}

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

    /// Whether this host can synchronously copy pixels from Ganesh or raster
    /// CanvasKit surfaces. Graphite hosts must keep this false until they own
    /// an asynchronous completion path.
    fn supports_readback(&self) -> bool {
        false
    }

    /// Copies one validated physical-pixel region. `None` means that the host
    /// does not implement readback and must not advertise it.
    fn read_pixels_rgba8888(
        &mut self,
        _region: CanvasKitPixelRegion,
    ) -> Result<Option<CanvasKitReadback>, Self::Error> {
        Ok(None)
    }

    /// Applies cache pressure without retiring authoritative resource bytes.
    /// `false` means the host does not implement the operation.
    fn trim_memory(&mut self, _pressure: MemoryPressure) -> Result<bool, Self::Error> {
        Ok(false)
    }

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

/// Synchronous paragraph operations exposed by the same browser executor as
/// [`CanvasKitHost`]. Keeping these calls on the owning executor guarantees
/// that layout handles and frame paint resolve against one CanvasKit session.
pub trait CanvasKitParagraphHost: CanvasKitHost + Send + 'static {
    fn layout_paragraph(&mut self, request: Vec<u8>) -> Result<Vec<u8>, Self::Error>;
    fn destroy_paragraph(&mut self, handle: ResourceHandle) -> Result<(), Self::Error>;
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
