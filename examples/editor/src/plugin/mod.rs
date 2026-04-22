//! WASM plugin system for the Fission Editor.
//!
//! Plugins are compiled as `.wasm` files and loaded at runtime. Communication
//! between host and plugin happens through a stable ABI using protobuf-encoded
//! messages.
//!
//! # ABI
//!
//! The ABI consists of two functions exported by the host and available to plugins:
//!
//! ```text
//! // Send a message from plugin to host. Returns 0 on success, -1 on error.
//! fn host_send(ptr: *const u8, len: u32) -> i32;
//!
//! // Receive a message from host. Writes into the plugin's buffer.
//! // Returns the number of bytes written, or -1 if no message is pending.
//! fn host_recv(ptr: *mut u8, len: u32) -> i32;
//! ```
//!
//! Plugins export:
//!
//! ```text
//! // Called once when the plugin is loaded.
//! fn plugin_init();
//!
//! // Called each frame/tick to let the plugin process pending work.
//! fn plugin_tick();
//!
//! // Allocate memory in the plugin's linear memory for the host to write into.
//! fn plugin_alloc(len: u32) -> *mut u8;
//! ```
//!
//! # Protocol
//!
//! Messages are encoded using a simple length-prefixed format (not actual
//! protobuf for now, but the structure mirrors what a proto schema would
//! produce):
//!
//! ```text
//! [type: u16] [payload_len: u32] [payload: bytes]
//! ```
//!
//! Message types are defined in the `messages` module.

pub mod messages;
pub mod host;
