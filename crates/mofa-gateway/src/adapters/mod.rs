//! Gateway external adapters for protocol implementations (A2A, MCP, etc.)

pub mod a2a;

pub use a2a::{A2aAdapter, AgentCard, A2aTask, A2aTaskStatus};
