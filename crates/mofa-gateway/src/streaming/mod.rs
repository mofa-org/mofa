//! Unified streaming abstractions for the MoFA gateway.
//!
//! # Overview
//!
//! This module centralises all streaming transport logic so that gateway
//! handlers never need to build SSE events or WebSocket frames by hand.
//!
//! ```text
//!                    ┌──────────────────────────────────────────┐
//!                    │           mofa-gateway handlers           │
//!                    └────────────┬──────────────┬──────────────┘
//!                                 │ BoxTokenStream│ BoxTokenStream
//!                    ┌────────────▼──┐    ┌───────▼──────────────┐
//!                    │  sse_builder  │    │         ws           │
//!                    │  SseBuilder   │    │ ws_chat_completions() │
//!                    └────────────┬──┘    └───────┬──────────────┘
//!                                 │               │
//!                    SSE response │               │ WebSocket frames
//!                    (HTTP/1.1    │               │ (JSON chunks
//!                     chunked)    │               │  + "[DONE]")
//!                                 ▼               ▼
//!                              Client          Client
//!
//!                    ┌──────────────────────────────────────────┐
//!                    │               proxy                      │
//!                    │  forward_response()                       │
//!                    │  upstream SSE → client (zero-copy)       │
//!                    │  upstream JSON → client (buffered)       │
//!                    └──────────────────────────────────────────┘
//! ```
//!
//! # Modules
//!
//! | Module | Entry point | Use when |
//! |--------|-------------|---------|
//! | [`sse_builder`] | [`SseBuilder`] | Building an SSE response from a [`BoxTokenStream`] in any handler |
//! | [`ws`] | `ws_chat_completions` | Clients that need bidirectional transport or mid-stream cancellation |
//! | [`proxy`] | `forward_response` | Proxying an upstream LLM server's response transparently |
//!
//! # Data flow (SSE path)
//!
//! ```text
//! InferenceOrchestrator::infer_stream()
//!         │
//!         │  BoxTokenStream  (StreamChunk items)
//!         ▼
//!     SseBuilder::build_response()
//!         │
//!         │  produces three event groups:
//!         │
//!         ├─ 1. role chunk ──────► data: {"choices":[{"delta":{"role":"assistant"}}]}
//!         │
//!         ├─ 2. content chunks ──► data: {"choices":[{"delta":{"content":"Hello"}}]}
//!         │                        data: {"choices":[{"delta":{"content":" world"}}]}
//!         │
//!         ├─ 3. stop chunk ──────► data: {"choices":[{"delta":{},"finish_reason":"stop"}]}
//!         │
//!         └─ 4. [DONE] ──────────► data: [DONE]
//! ```
//!
//! [`BoxTokenStream`]: mofa_kernel::llm::streaming::BoxTokenStream

pub mod proxy;
pub mod sse_builder;
pub mod ws;

pub use sse_builder::SseBuilder;
