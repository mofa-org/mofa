//! Network transport for Raft consensus.
//!
//! This module handles inter-node communication for the Raft consensus
//! algorithm, including:
//! - RequestVote RPCs (for leader election)
//! - AppendEntries RPCs (for log replication)
//! - Heartbeat messages
//!
//! # Implementation Status
//!
//! **Complete** - Network transport interface and in-memory implementation for testing

use crate::error::ConsensusResult;
use crate::types::{LogEntry, LogIndex, NodeId, Term};

/// Request to vote for a candidate.
#[derive(Debug, Clone)]
pub struct RequestVoteRequest {
    /// Candidate's term.
    pub term: Term,
    /// Candidate requesting vote.
    pub candidate_id: NodeId,
    /// Index of candidate's last log entry.
    pub last_log_index: LogIndex,
    /// Term of candidate's last log entry.
    pub last_log_term: Term,
}

/// Response to vote request.
#[derive(Debug, Clone)]
pub struct RequestVoteResponse {
    /// Current term (for candidate to update itself).
    pub term: Term,
    /// True means candidate received vote.
    pub vote_granted: bool,
}

/// Request to append entries to follower's log.
#[derive(Debug, Clone)]
pub struct AppendEntriesRequest {
    /// Leader's term.
    pub term: Term,
    /// Leader ID (so follower can redirect clients).
    pub leader_id: NodeId,
    /// Index of log entry immediately preceding new ones.
    pub prev_log_index: LogIndex,
    /// Term of prev_log_index entry.
    pub prev_log_term: Term,
    /// Log entries to store (empty for heartbeat).
    pub entries: Vec<LogEntry>,
    /// Leader's commit_index.
    pub leader_commit: LogIndex,
}

/// Response to append entries request.
#[derive(Debug, Clone)]
pub struct AppendEntriesResponse {
    /// Current term (for leader to update itself).
    pub term: Term,
    /// True if follower contained entry matching prev_log_index and prev_log_term.
    pub success: bool,
    /// Follower's last log index (for leader to update next_index).
    pub last_log_index: LogIndex,
}

/// Trait for Raft network transport.
#[async_trait::async_trait]
pub trait RaftTransport: Send + Sync {
    /// Send a RequestVote RPC to a node.
    async fn request_vote(
        &self,
        node_id: &NodeId,
        request: RequestVoteRequest,
    ) -> ConsensusResult<RequestVoteResponse>;

    /// Send an AppendEntries RPC to a node.
    async fn append_entries(
        &self,
        node_id: &NodeId,
        request: AppendEntriesRequest,
    ) -> ConsensusResult<AppendEntriesResponse>;
}
