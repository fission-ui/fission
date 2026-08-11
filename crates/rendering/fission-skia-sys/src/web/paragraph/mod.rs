//! Versioned, pointer-free SkParagraph request/result transport for CanvasKit.
//!
//! Paragraph layout is synchronous because Fission layout needs the resulting
//! geometry before it can compile the frame. The browser owns the resulting
//! CanvasKit Paragraph and returns only a generational handle plus copied
//! scalar geometry. The request may reference font resources already installed
//! in the shared CanvasKit resource registry.

mod output;
mod request;
mod wire;

pub use output::{decode_paragraph_response, encode_paragraph_response, WebParagraphResponse};
pub use request::{
    decode_paragraph_request, encode_paragraph_request, WebParagraphFont, WebParagraphRequest,
};
pub use wire::{ParagraphWireError, MAX_PARAGRAPH_PACKET_BYTES};

#[cfg(test)]
mod tests;
