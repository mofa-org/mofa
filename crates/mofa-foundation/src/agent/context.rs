//! Agent context module - Foundation layer extensions
//!
//! This module provides extensions to the kernel's CoreAgentContext.
//! Following microkernel architecture principles:
//!
//! - **Kernel Layer**: CoreAgentContext with basic primitives (K/V store, interrupt, event bus)
//! - **Foundation Layer**: Extended functionality through composition
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    CoreAgentContext                         │
//! │                   (mofa-kernel)                             │
//! │  - execution_id, session_id                                 │
//! │  - generic K/V store                                        │
//! │  - interrupt signal, event bus                              │
//! └───────────────────────────┬─────────────────────────────────┘
//!                            │
//!        ┌───────────────────┼───────────────────┐
//!        │                   │                   │
//!        ▼                   ▼                   ▼
//! ┌─────────────┐   ┌─────────────┐   ┌─────────────┐
//! │RichAgentCtx │   │PromptContext│   │ Custom Ext  │
//! │ (metrics)   │   │ (building)  │   │ (via trait) │
//! └─────────────┘   └─────────────┘   └─────────────┘
//! ```
//!
//! # Modules
//!
//! - `rich`: RichAgentContext - adds metrics and output tracking
//! - `ext`: ContextExt trait - generic extension mechanism
//! - `prompt`: PromptContext - specialized prompt building
//!
//! # Usage
//!
//! ```rust,ignore
//! use mofa_foundation::agent::context::RichAgentContext;
//! use mofa_kernel::agent::context::CoreAgentContext;
//!
//! let core = CoreAgentContext::new("exec-123");
//! let rich = RichAgentContext::new(core);
//!
//! // Core functionality (delegated)
//! rich.set("key", "value").await;
//!
//! // Extended functionality
//! rich.record_output("llm", json!("response")).await;
//! ```

pub mod ext;
pub mod prompt;
pub mod rich;

// Re-export rich context types (primary foundation extension)
pub use rich::{ComponentOutput, ExecutionMetrics, RichAgentContext};

// Re-export extension traits
pub use ext::ContextExt;

// Re-export prompt context (specialized builder)
pub use prompt::{AgentIdentity, PromptContext, PromptContextBuilder};
