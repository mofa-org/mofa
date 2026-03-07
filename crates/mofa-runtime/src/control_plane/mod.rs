//! HTTP control plane and gateway scaffolding.
//!
//! This module is gated behind the `gateway` feature and provides the
//! foundational pieces for the upcoming HTTP control plane and API gateway.
//! Phase 1 intentionally exposes no routes; it only wires shared state,
//! server configuration, and type definitions so later phases can add
//! handlers and middleware incrementally.

pub mod api;
pub mod gateway;
pub mod middleware;
pub mod server;
pub mod state;
pub mod types;

pub use server::{ControlPlaneConfig, ControlPlaneServer, ControlPlaneServerError};
pub use state::ControlPlaneState;
pub use types::{ApiError, ApiResponse, CreateAgentRequest, InvokeRequest};
