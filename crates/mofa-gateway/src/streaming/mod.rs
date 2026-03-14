//! Unified streaming abstractions for the MoFA gateway.
//!
//! This module provides:
//!
//! - [`SseBuilder`]: Converts any [`BoxTokenStream`] into an OpenAI-compatible
//!   Server-Sent Events HTTP response. Usable from any gateway handler.
//!
//! - [`proxy`]: SSE-aware HTTP proxy passthrough. Streams `text/event-stream`
//!   responses without buffering; buffers non-streaming responses and sets an
//!   accurate `Content-Length`.
//!
//! - [`ws`]: WebSocket streaming endpoint (`GET /ws/v1/chat/completions`) for
//!   clients that require bidirectional communication or mid-stream cancellation.

pub mod proxy;
pub mod sse_builder;
pub mod ws;

pub use sse_builder::SseBuilder;
