//! Tool execution sandbox — kernel-level trait contracts and policy model.
//!
//! This module defines the backend-agnostic interface for running untrusted
//! tools with capability-based access control and resource limits. Concrete
//! backends (`NullSandbox`, `ProcessSandbox`, `WasmtimeSandbox`) live in
//! `mofa-foundation`; the kernel only owns the contracts.
//!
//! # Layered architecture
//!
//! ```text
//!     ┌───────────────────────────────────────────────────────┐
//!     │                     Agent Loop                         │
//!     │  (mofa-foundation/src/llm/agent_loop.rs)               │
//!     └────────────────────────┬──────────────────────────────┘
//!                              │  ToolInput / ToolResult
//!                              ▼
//!     ┌───────────────────────────────────────────────────────┐
//!     │                SandboxedTool<T: Tool>                  │
//!     │  (mofa-foundation adapter, wraps any Tool impl)        │
//!     └────────────────────────┬──────────────────────────────┘
//!                              │  SandboxRequest
//!                              ▼
//!     ┌───────────────────────────────────────────────────────┐
//!     │            trait ToolSandbox  (this module)            │
//!     │                                                        │
//!     │    .policy()   .tier()   .precheck()   .execute()      │
//!     └────────────────────────┬──────────────────────────────┘
//!                              │
//!          ┌───────────────────┼────────────────────┐
//!          ▼                   ▼                    ▼
//!     ┌──────────┐      ┌──────────────┐     ┌─────────────┐
//!     │ NullSbx  │      │ ProcessSbx   │     │ WasmtimeSbx │
//!     │ (trusted)│      │ (fork+rlimit)│     │ (wasmtime)  │
//!     │ Tier:None│      │ Tier:Process │     │ Tier:VM     │
//!     └──────────┘      └──────────────┘     └─────────────┘
//! ```
//!
//! # Policy model
//!
//! Policies are **default-deny**. The base capability set is empty except
//! for implicit [`SandboxCapability::Compute`]; any other capability
//! (`FsRead`/`FsWrite`, `Net`, `EnvRead`, `Subprocess`, `Clock`,
//! `RandomRead`) must be explicitly listed along with a fine-grained
//! allow-list when applicable (e.g. `fs_allow_list` of [`PathPattern`]s
//! for filesystem access).
//!
//! # Error taxonomy
//!
//! [`SandboxError`] distinguishes three failure classes:
//! - **Policy denial** — tool attempted a forbidden capability or access.
//! - **Resource breach** — tool exceeded CPU/memory/wall/output caps.
//! - **Backend failure** — sandbox infrastructure itself errored.
//!
//! This matters for retry semantics: only backend failures are plausibly
//! retryable; policy denials and resource breaches are deterministic.

pub mod error;
pub mod policy;
pub mod traits;

pub use error::{SandboxError, SandboxResult};
pub use policy::{
    SandboxCapability, NetEndpoint, PathPattern, SandboxResourceLimits, SandboxPolicy, SandboxPolicyBuilder,
};
pub use traits::{
    ObservationDecision, SandboxExecutionStats, SandboxObserver, SandboxRequest, SandboxResponse,
    SandboxTier, ToolSandbox,
};
