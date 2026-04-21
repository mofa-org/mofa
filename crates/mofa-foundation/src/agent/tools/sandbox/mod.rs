//! Tool execution sandbox — concrete backends.
//!
//! Implements the [`mofa_kernel::agent::components::sandbox::ToolSandbox`]
//! contract with three backends:
//!
//! - [`NullSandbox`] — passthrough for trusted tools ([`SandboxTier::None`]).
//! - [`InProcessSandbox`] — same-process with policy precheck, wall-clock
//!   timeout, and output-size cap ([`SandboxTier::None`]).
//! - [`ChildProcessSandbox`] — OS child process per invocation, JSON over
//!   stdin/stdout, env scrubbing, timeout ([`SandboxTier::Process`]).
//!
//! A [`SandboxedTool`] adapter wraps any `ToolSandbox` as a regular `Tool`
//! so sandboxing drops transparently into an existing `ToolRegistry`.
//!
//! ```text
//!      Tool (trusted)
//!           │
//!           ▼
//!      NullSandbox      ─────►  SandboxedTool  ─────►  ToolRegistry
//!
//!      Tool (semi-trusted)
//!           │
//!           ▼
//!      InProcessSandbox ─────►  SandboxedTool  ─────►  ToolRegistry
//!
//!      External binary
//!           │
//!           ▼
//!      ChildProcessSandbox ──►  SandboxedTool  ─────►  ToolRegistry
//! ```
//!
//! See `docs/tool-sandbox.md` for the complete architecture.
//!
//! [`SandboxTier::None`]: mofa_kernel::agent::components::sandbox::SandboxTier::None
//! [`SandboxTier::Process`]: mofa_kernel::agent::components::sandbox::SandboxTier::Process

pub mod child_process;
pub mod in_process;
pub mod null;
pub mod sandboxed_tool;

pub use child_process::{ChildProcessCommand, ChildProcessSandbox};
pub use in_process::InProcessSandbox;
pub use null::NullSandbox;
pub use sandboxed_tool::SandboxedTool;
