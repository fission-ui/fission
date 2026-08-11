//! Target-independent wire contract for the CanvasKit host bridge.
//!
//! The browser owns CanvasKit objects. Rust exchanges bounded binary packets
//! containing only values and generational identifiers, so neither side keeps
//! pointers into the other module's WebAssembly memory.

mod protocol;
mod types;

pub use protocol::{
    decode, encode, DecodeLimits, ProtocolError, ProtocolSession, DEFAULT_DECODE_LIMITS,
};
pub use types::{
    Ack, AlphaMode, BackendPreference, ColorSpace, DamageRect, Destroy, DestroyReason, Envelope,
    ErrorCode, ErrorPacket, Frame, Init, Message, Packet, PacketKind, Resize, ResourceBatch,
    ResourceHandle, ResourceKind, ResourceOperation, ResourceUpdate, SessionId, SurfaceSize,
    HEADER_LEN, MAGIC, PROTOCOL_VERSION,
};

#[cfg(test)]
mod tests;
