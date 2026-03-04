//! MoFA Testing Framework
//!
//! Provides mock implementations and assertion utilities for testing
//! MoFA Agents, Tools, and LLM orchestration without live API calls.
//!
//! # Modules
//!
//! - [`backend`] — `MockLLMBackend`: a deterministic `ModelOrchestrator` double
//! - [`bus`]     — `MockAgentBus`: message-bus spy with capture history
//! - [`tools`]   — `MockTool` + `assert_tool_called!` macro

pub mod backend;
pub mod bus;
pub mod tools;

pub use backend::MockLLMBackend;
pub use bus::MockAgentBus;
pub use tools::MockTool;
