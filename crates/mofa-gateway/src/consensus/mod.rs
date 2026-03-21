//! Raft consensus algorithm implementation.
//!
//! This module provides a production-grade Raft consensus engine for the
//! control plane. Raft is used to ensure consistency across the distributed
//! cluster.
//!
//! # Raft Overview
//!
//! Raft is a consensus algorithm designed to be understandable. It's equivalent
//! to Paxos in fault-tolerance and performance, but structured to be easier to
//! understand and implement.
//!
//! Key concepts:
//! - **Leader**: Accepts client requests, replicates log to followers
//! - **Follower**: Receives log entries from leader, votes in elections
//! - **Candidate**: Participates in leader election
//! - **Term**: Monotonically increasing number, incremented on election
//! - **Log**: Sequence of log entries, replicated across all nodes
//!
//! # Implementation Status
//!
//! **Complete** - Raft consensus engine fully implemented and tested

pub mod state;
pub mod transport;
pub mod storage;
pub mod engine;

#[cfg(test)]
mod engine_tests;

// Make transport_impl always available (it's only used for testing)
// This allows test files to import it
pub mod transport_impl;

pub use state::*;
pub use transport::*;
pub use storage::*;
pub use engine::*;
pub use transport_impl::*;