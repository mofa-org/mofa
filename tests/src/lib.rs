//! MoFA Testing Framework
//!
//! Provides utilities for testing Agents, Tools, and LLM behaviors
//! without requiring live API calls or complex runtime setup.

pub mod bus;
pub mod tools;

pub use bus::MockAgentBus;
