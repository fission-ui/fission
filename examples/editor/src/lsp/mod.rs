//! Minimal LSP client that speaks JSON-RPC over stdin/stdout to rust-analyzer.
//!
//! This is intentionally simple - a production implementation would use
//! async IO and proper request/response tracking. This synchronous version
//! is enough to get diagnostics and completions flowing.

pub mod client;
pub mod protocol;

pub use client::LspClient;
