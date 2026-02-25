//! Time-Travel Debugger Telemetry
//!
//! Defines the core telemetry types and traits for the visual time-travel debugger.
//! The kernel layer defines only trait interfaces and data types — concrete
//! implementations are provided in `mofa-foundation`.
//!
//! # Architecture
//!
//! ```text
//! ┌──────────────────────────────────────────────────────┐
//! │                  Telemetry Pipeline                   │
//! ├──────────────────────────────────────────────────────┤
//! │                                                       │
//! │  WorkflowExecutor ──emit──▶ TelemetryEmitter         │
//! │                                  │                    │
//! │                          ┌───────┴───────┐           │
//! │                          ▼               ▼           │
//! │                    Channel          SessionRecorder   │
//! │                  (real-time)        (persistence)     │
//! │                                                       │
//! └──────────────────────────────────────────────────────┘
//! ```
//!
//! # Event Types
//!
//! - `DebugEvent::WorkflowStart` — emitted when workflow execution begins
//! - `DebugEvent::NodeStart` — emitted when a node begins execution
//! - `DebugEvent::StateChange` — emitted on state mutations within a node
//! - `DebugEvent::NodeEnd` — emitted when a node finishes execution
//! - `DebugEvent::WorkflowEnd` — emitted when workflow execution completes
//! - `DebugEvent::Error` — emitted on errors during execution

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::agent::error::AgentResult;

// ============================================================================
// DebugEvent — Telemetry event types
// ============================================================================

/// Debug telemetry event emitted during workflow execution.
///
/// These events form the backbone of the time-travel debugger, capturing
/// the complete execution trace of a workflow including state mutations.
///
/// # Example
///
/// ```rust,ignore
/// use mofa_kernel::workflow::telemetry::DebugEvent;
///
/// let event = DebugEvent::NodeStart {
///     node_id: "process".to_string(),
///     timestamp_ms: 1700000000000,
///     state_snapshot: serde_json::json!({"messages": ["hello"]}),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub enum DebugEvent {
    /// Workflow execution started
    WorkflowStart {
        /// Workflow graph ID
        workflow_id: String,
        /// Unique execution ID
        execution_id: String,
        /// Timestamp in milliseconds since epoch
        timestamp_ms: u64,
    },

    /// A node started executing
    NodeStart {
        /// Node identifier
        node_id: String,
        /// Timestamp in milliseconds since epoch
        timestamp_ms: u64,
        /// Snapshot of the state when the node started
        state_snapshot: serde_json::Value,
    },

    /// A state key was mutated during node execution
    StateChange {
        /// Node that caused the change
        node_id: String,
        /// Timestamp in milliseconds since epoch
        timestamp_ms: u64,
        /// State key that changed
        key: String,
        /// Previous value (None if key was new)
        old_value: Option<serde_json::Value>,
        /// New value
        new_value: serde_json::Value,
    },

    /// A node finished executing
    NodeEnd {
        /// Node identifier
        node_id: String,
        /// Timestamp in milliseconds since epoch
        timestamp_ms: u64,
        /// Snapshot of the state after node execution
        state_snapshot: serde_json::Value,
        /// Duration of node execution in milliseconds
        duration_ms: u64,
    },

    /// Workflow execution completed
    WorkflowEnd {
        /// Workflow graph ID
        workflow_id: String,
        /// Unique execution ID
        execution_id: String,
        /// Timestamp in milliseconds since epoch
        timestamp_ms: u64,
        /// Final status description
        status: String,
    },

    /// Error during execution
    Error {
        /// Node where the error occurred (if applicable)
        node_id: Option<String>,
        /// Timestamp in milliseconds since epoch
        timestamp_ms: u64,
        /// Error description
        error: String,
    },
}

impl DebugEvent {
    /// Get the timestamp of this event in milliseconds since epoch
    pub fn timestamp_ms(&self) -> u64 {
        match self {
            Self::WorkflowStart { timestamp_ms, .. } => *timestamp_ms,
            Self::NodeStart { timestamp_ms, .. } => *timestamp_ms,
            Self::StateChange { timestamp_ms, .. } => *timestamp_ms,
            Self::NodeEnd { timestamp_ms, .. } => *timestamp_ms,
            Self::WorkflowEnd { timestamp_ms, .. } => *timestamp_ms,
            Self::Error { timestamp_ms, .. } => *timestamp_ms,
        }
    }

    /// Get the node ID associated with this event, if any
    pub fn node_id(&self) -> Option<&str> {
        match self {
            Self::WorkflowStart { .. } => None,
            Self::NodeStart { node_id, .. } => Some(node_id),
            Self::StateChange { node_id, .. } => Some(node_id),
            Self::NodeEnd { node_id, .. } => Some(node_id),
            Self::WorkflowEnd { .. } => None,
            Self::Error { node_id, .. } => node_id.as_deref(),
        }
    }

    /// Get the current timestamp in milliseconds since epoch
    pub fn now_ms() -> u64 {
        crate::utils::now_ms()
    }

    /// Returns a human-readable event type name
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::WorkflowStart { .. } => "workflow_start",
            Self::NodeStart { .. } => "node_start",
            Self::StateChange { .. } => "state_change",
            Self::NodeEnd { .. } => "node_end",
            Self::WorkflowEnd { .. } => "workflow_end",
            Self::Error { .. } => "error",
        }
    }
}

// ============================================================================
// DebugSession — Session metadata
// ============================================================================

/// Metadata for a recorded debugging session.
///
/// A session corresponds to a single workflow execution, capturing
/// all telemetry events from start to finish.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugSession {
    /// Unique session identifier
    pub session_id: String,
    /// Workflow graph ID
    pub workflow_id: String,
    /// Execution ID (from RuntimeContext)
    pub execution_id: String,
    /// Session start timestamp (ms since epoch)
    pub started_at: u64,
    /// Session end timestamp (ms since epoch), None if still running
    pub ended_at: Option<u64>,
    /// Final status ("running", "completed", "failed")
    pub status: String,
    /// Total number of events recorded
    pub event_count: u64,
}

impl DebugSession {
    /// Create a new debug session
    pub fn new(
        session_id: impl Into<String>,
        workflow_id: impl Into<String>,
        execution_id: impl Into<String>,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            workflow_id: workflow_id.into(),
            execution_id: execution_id.into(),
            started_at: DebugEvent::now_ms(),
            ended_at: None,
            status: "running".to_string(),
            event_count: 0,
        }
    }
}

// ============================================================================
// TelemetryEmitter — Trait for emitting debug events
// ============================================================================

/// Trait for emitting telemetry events during workflow execution.
///
/// Implementations can forward events to channels, recorders, loggers, etc.
///
/// # Example
///
/// ```rust,ignore
/// use mofa_kernel::workflow::telemetry::{TelemetryEmitter, DebugEvent};
///
/// struct LoggingEmitter;
///
/// #[async_trait]
/// impl TelemetryEmitter for LoggingEmitter {
///     async fn emit(&self, event: DebugEvent) {
///         println!("[{}] {:?}", event.event_type(), event);
///     }
/// }
/// ```
#[async_trait]
pub trait TelemetryEmitter: Send + Sync {
    /// Emit a debug event
    async fn emit(&self, event: DebugEvent);

    /// Check if telemetry collection is enabled
    ///
    /// When false, the executor may skip expensive operations like
    /// state snapshot serialization.
    fn is_enabled(&self) -> bool {
        true
    }
}

// ============================================================================
// SessionRecorder — Trait for persisting debug sessions
// ============================================================================

/// Trait for persisting and querying debug sessions.
///
/// Implementations store the complete execution trace to enable
/// time-travel debugging: replaying the exact sequence of events
/// and state mutations step-by-step.
///
/// # Example
///
/// ```rust,ignore
/// use mofa_kernel::workflow::telemetry::{SessionRecorder, DebugSession, DebugEvent};
///
/// // Start a session
/// let session = DebugSession::new("session-1", "workflow-1", "exec-1");
/// recorder.start_session(&session).await?;
///
/// // Record events during execution
/// recorder.record_event("session-1", &event).await?;
///
/// // End the session
/// recorder.end_session("session-1").await?;
///
/// // Later: replay the session
/// let events = recorder.get_events("session-1").await?;
/// for event in events {
///     // Step through each event in the time-travel UI
/// }
/// ```
#[async_trait]
pub trait SessionRecorder: Send + Sync {
    /// Start recording a new debug session
    async fn start_session(&self, session: &DebugSession) -> AgentResult<()>;

    /// Record a telemetry event within a session
    async fn record_event(&self, session_id: &str, event: &DebugEvent) -> AgentResult<()>;

    /// End a recording session (sets ended_at and final status)
    async fn end_session(&self, session_id: &str, status: &str) -> AgentResult<()>;

    /// Retrieve session metadata by ID
    async fn get_session(&self, session_id: &str) -> AgentResult<Option<DebugSession>>;

    /// Retrieve all events for a session, ordered by timestamp
    async fn get_events(&self, session_id: &str) -> AgentResult<Vec<DebugEvent>>;

    /// List all recorded sessions
    async fn list_sessions(&self) -> AgentResult<Vec<DebugSession>>;
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_debug_event_serialization() {
        let event = DebugEvent::NodeStart {
            node_id: "process".to_string(),
            timestamp_ms: 1700000000000,
            state_snapshot: json!({"messages": ["hello"]}),
        };

        let serialized = serde_json::to_string(&event).unwrap();
        let deserialized: DebugEvent = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.event_type(), "node_start");
        assert_eq!(deserialized.node_id(), Some("process"));
        assert_eq!(deserialized.timestamp_ms(), 1700000000000);
    }

    #[test]
    fn test_debug_event_all_variants_serialize() {
        let events = vec![
            DebugEvent::WorkflowStart {
                workflow_id: "wf-1".to_string(),
                execution_id: "exec-1".to_string(),
                timestamp_ms: 1000,
            },
            DebugEvent::NodeStart {
                node_id: "n1".to_string(),
                timestamp_ms: 1001,
                state_snapshot: json!({}),
            },
            DebugEvent::StateChange {
                node_id: "n1".to_string(),
                timestamp_ms: 1002,
                key: "count".to_string(),
                old_value: Some(json!(0)),
                new_value: json!(1),
            },
            DebugEvent::NodeEnd {
                node_id: "n1".to_string(),
                timestamp_ms: 1003,
                state_snapshot: json!({"count": 1}),
                duration_ms: 2,
            },
            DebugEvent::WorkflowEnd {
                workflow_id: "wf-1".to_string(),
                execution_id: "exec-1".to_string(),
                timestamp_ms: 1004,
                status: "completed".to_string(),
            },
            DebugEvent::Error {
                node_id: Some("n2".to_string()),
                timestamp_ms: 1005,
                error: "something failed".to_string(),
            },
        ];

        for event in &events {
            let json = serde_json::to_string(event).unwrap();
            let round_trip: DebugEvent = serde_json::from_str(&json).unwrap();
            assert_eq!(event.event_type(), round_trip.event_type());
            assert_eq!(event.timestamp_ms(), round_trip.timestamp_ms());
        }
    }

    #[test]
    fn test_debug_event_node_id() {
        let start = DebugEvent::WorkflowStart {
            workflow_id: "w".to_string(),
            execution_id: "e".to_string(),
            timestamp_ms: 0,
        };
        assert_eq!(start.node_id(), None);

        let node = DebugEvent::NodeStart {
            node_id: "n1".to_string(),
            timestamp_ms: 0,
            state_snapshot: json!(null),
        };
        assert_eq!(node.node_id(), Some("n1"));

        let error_with_node = DebugEvent::Error {
            node_id: Some("n2".to_string()),
            timestamp_ms: 0,
            error: "err".to_string(),
        };
        assert_eq!(error_with_node.node_id(), Some("n2"));

        let error_without = DebugEvent::Error {
            node_id: None,
            timestamp_ms: 0,
            error: "err".to_string(),
        };
        assert_eq!(error_without.node_id(), None);
    }

    #[test]
    fn test_debug_session_creation() {
        let session = DebugSession::new("s-1", "wf-1", "exec-1");
        assert_eq!(session.session_id, "s-1");
        assert_eq!(session.workflow_id, "wf-1");
        assert_eq!(session.execution_id, "exec-1");
        assert_eq!(session.status, "running");
        assert!(session.ended_at.is_none());
        assert_eq!(session.event_count, 0);
        assert!(session.started_at > 0);
    }

    #[test]
    fn test_debug_session_serialization() {
        let session = DebugSession::new("s-1", "wf-1", "exec-1");
        let json = serde_json::to_string(&session).unwrap();
        let round_trip: DebugSession = serde_json::from_str(&json).unwrap();
        assert_eq!(round_trip.session_id, "s-1");
        assert_eq!(round_trip.status, "running");
    }

    #[test]
    fn test_now_ms() {
        let ts = DebugEvent::now_ms();
        // Should be a reasonable timestamp (after 2020)
        assert!(ts > 1_577_836_800_000);
    }
}
