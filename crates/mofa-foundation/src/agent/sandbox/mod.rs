//! Tool Execution Sandbox Module
//!
//! Provides a secure, isolated execution environment for untrusted tools.
//! This module implements the kernel's `ToolSandbox` trait with configurable
//! security policies, timeouts, capability checks, and audit logging.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │                  SandboxedToolExecutor                   │
//! │  implements ToolSandbox trait from mofa-kernel           │
//! │                                                         │
//! │  ┌─────────────┐  ┌──────────────┐  ┌───────────────┐  │
//! │  │SandboxConfig│  │SandboxPolicy │  │ tokio timeout │  │
//! │  │ (limits +   │  │ (capability  │  │ (execution    │  │
//! │  │ capabilities│  │  evaluation) │  │  deadline)    │  │
//! │  └─────────────┘  └──────────────┘  └───────────────┘  │
//! └─────────────────────────────────────────────────────────┘
//! ```
//!
//! # Usage
//!
//! ```rust,ignore
//! use mofa_foundation::agent::sandbox::{SandboxConfig, SandboxedToolExecutor};
//!
//! let config = SandboxConfig::default()
//!     .with_timeout(5000)
//!     .deny_capability(SandboxCapability::ProcessExec);
//!
//! let sandbox = SandboxedToolExecutor::new(config);
//! let result = sandbox.execute_sandboxed(&tool, input, &ctx).await?;
//! ```

pub mod config;
pub mod executor;
pub mod policy;

pub use config::SandboxConfig;
pub use executor::SandboxedToolExecutor;
pub use policy::{DefaultSandboxPolicy, PolicyDecision, SandboxPolicy};
