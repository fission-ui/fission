//! Target-independent wire contract for the CanvasKit host bridge.
//!
//! The browser owns CanvasKit objects. Rust exchanges bounded binary packets
//! containing only values and generational identifiers, so neither side keeps
//! pointers into the other module's WebAssembly memory.

mod commands;
mod paragraph;
mod protocol;
mod types;

pub use commands::{
    decode_commands, encode_commands, CommandStreamError, WebCommand, WebImageAlignment,
    WebImageFit, COMMAND_HEADER_LEN, COMMAND_MAGIC, COMMAND_VERSION, MAX_COMMANDS,
    MAX_COMMAND_STREAM_BYTES, MAX_DASH_INTERVALS, MAX_GRADIENT_STOPS, MAX_PATH_COMMANDS,
};
pub use paragraph::{
    decode_paragraph_request, decode_paragraph_response, encode_paragraph_request,
    encode_paragraph_response, ParagraphWireError, WebParagraphFont, WebParagraphRequest,
    WebParagraphResponse, MAX_PARAGRAPH_PACKET_BYTES,
};
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
mod command_tests;
#[cfg(test)]
mod tests;
