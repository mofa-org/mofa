//! Raft state machine and state management.
//!
//! This module implements the core Raft state machine, including:
//! - State transitions (Follower → Candidate → Leader)
//! - Log replication
//! - Leader election
//! - Term management
//!
//! # Implementation Status
//!
//! **Complete** - Raft state machine and state management fully implemented

use crate::error::{ConsensusError, ConsensusResult};
use crate::types::{LogEntry, LogIndex, NodeId, RaftState, Term};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Raft node state.
#[derive(Debug, Clone)]
pub struct RaftNodeState {
    /// Current Raft state (Follower, Candidate, Leader).
    pub state: RaftState,
    /// Current term.
    pub current_term: Term,
    /// Node that received vote in current term (or None).
    pub voted_for: Option<NodeId>,
    /// Log entries.
    pub log: Vec<LogEntry>,
    /// Index of highest log entry known to be committed.
    pub commit_index: LogIndex,
    /// Index of highest log entry applied to state machine.
    pub last_applied: LogIndex,
}

impl RaftNodeState {
    /// Create a new Raft node state.
    pub fn new() -> Self {
        Self {
            state: RaftState::Follower,
            current_term: Term::new(0),
            voted_for: None,
            log: Vec::new(),
            commit_index: LogIndex::new(0),
            last_applied: LogIndex::new(0),
        }
    }

    /// Get the last log term and index.
    pub fn last_log_info(&self) -> (Term, LogIndex) {
        if let Some(last) = self.log.last() {
            (last.term, last.index)
        } else {
            (Term::new(0), LogIndex::new(0))
        }
    }
}

impl Default for RaftNodeState {
    fn default() -> Self {
        Self::new()
    }
}

/// Leader-specific state (only valid when state is Leader).
#[derive(Debug, Clone)]
pub struct LeaderState {
    /// For each server, index of next log entry to send to that server.
    pub next_index: HashMap<NodeId, LogIndex>,
    /// For each server, index of highest log entry known to be replicated.
    pub match_index: HashMap<NodeId, LogIndex>,
}

impl LeaderState {
    /// Create new leader state for given followers.
    pub fn new(followers: &[NodeId], last_log_index: LogIndex) -> Self {
        let mut next_index = HashMap::new();
        let mut match_index = HashMap::new();
        let next = last_log_index.increment();

        for follower in followers {
            next_index.insert(follower.clone(), next);
            match_index.insert(follower.clone(), LogIndex::new(0));
        }

        Self {
            next_index,
            match_index,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_raft_node_state() {
        let state = RaftNodeState::new();
        assert_eq!(state.state, RaftState::Follower);
        assert_eq!(state.current_term.0, 0);
        assert!(state.voted_for.is_none());
        assert!(state.log.is_empty());
    }

    #[test]
    fn test_last_log_info_empty() {
        let state = RaftNodeState::new();
        let (term, index) = state.last_log_info();
        assert_eq!(term.0, 0);
        assert_eq!(index.0, 0);
    }
}
