//! Session Recorder Implementations
//!
//! Concrete implementations of `SessionRecorder` for persisting debug sessions.

use async_trait::async_trait;
use mofa_kernel::agent::error::{AgentError, AgentResult};
use mofa_kernel::workflow::telemetry::{DebugEvent, DebugSession, SessionRecorder};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

// ============================================================================
// InMemorySessionRecorder
// ============================================================================

/// In-memory session recorder for testing and single-session debugging.
///
/// Stores all sessions and events in memory. Data is lost when the
/// recorder is dropped.
///
/// # Example
///
/// ```rust,ignore
/// use mofa_foundation::workflow::session_recorder::InMemorySessionRecorder;
/// use mofa_kernel::workflow::telemetry::{SessionRecorder, DebugSession};
///
/// let recorder = InMemorySessionRecorder::new();
/// let session = DebugSession::new("s-1", "wf-1", "exec-1");
/// recorder.start_session(&session).await?;
/// ```
pub struct InMemorySessionRecorder {
    sessions: Arc<RwLock<HashMap<String, DebugSession>>>,
    events: Arc<RwLock<HashMap<String, Vec<DebugEvent>>>>,
}

impl InMemorySessionRecorder {
    /// Create a new in-memory session recorder.
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            events: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for InMemorySessionRecorder {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SessionRecorder for InMemorySessionRecorder {
    async fn start_session(&self, session: &DebugSession) -> AgentResult<()> {
        let mut sessions = self.sessions.write().await;
        sessions.insert(session.session_id.clone(), session.clone());

        let mut events = self.events.write().await;
        events.insert(session.session_id.clone(), Vec::new());

        Ok(())
    }

    async fn record_event(&self, session_id: &str, event: &DebugEvent) -> AgentResult<()> {
        let mut events = self.events.write().await;
        let entry = events.get_mut(session_id).ok_or_else(|| {
            AgentError::InvalidInput(format!("Session not found: {}", session_id))
        })?;
        entry.push(event.clone());

        // Update event count in session metadata
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.get_mut(session_id) {
            session.event_count = entry.len() as u64;
        }

        Ok(())
    }

    async fn end_session(&self, session_id: &str, status: &str) -> AgentResult<()> {
        let mut sessions = self.sessions.write().await;
        let session = sessions.get_mut(session_id).ok_or_else(|| {
            AgentError::InvalidInput(format!("Session not found: {}", session_id))
        })?;

        session.ended_at = Some(DebugEvent::now_ms());
        session.status = status.to_string();

        Ok(())
    }

    async fn get_session(&self, session_id: &str) -> AgentResult<Option<DebugSession>> {
        let sessions = self.sessions.read().await;
        Ok(sessions.get(session_id).cloned())
    }

    async fn get_events(&self, session_id: &str) -> AgentResult<Vec<DebugEvent>> {
        let events = self.events.read().await;
        Ok(events.get(session_id).cloned().unwrap_or_default())
    }

    async fn list_sessions(&self) -> AgentResult<Vec<DebugSession>> {
        let sessions = self.sessions.read().await;
        Ok(sessions.values().cloned().collect())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use mofa_kernel::workflow::telemetry::DebugEvent;
    use serde_json::json;

    #[tokio::test]
    async fn test_in_memory_recorder_start_session() {
        let recorder = InMemorySessionRecorder::new();
        let session = DebugSession::new("s-1", "wf-1", "exec-1");

        recorder.start_session(&session).await.unwrap();

        let retrieved = recorder.get_session("s-1").await.unwrap();
        assert!(retrieved.is_some());
        let s = retrieved.unwrap();
        assert_eq!(s.session_id, "s-1");
        assert_eq!(s.workflow_id, "wf-1");
        assert_eq!(s.status, "running");
    }

    #[tokio::test]
    async fn test_in_memory_recorder_record_events() {
        let recorder = InMemorySessionRecorder::new();
        let session = DebugSession::new("s-1", "wf-1", "exec-1");
        recorder.start_session(&session).await.unwrap();

        let event1 = DebugEvent::NodeStart {
            node_id: "n1".to_string(),
            timestamp_ms: 1000,
            state_snapshot: json!({}),
        };
        let event2 = DebugEvent::NodeEnd {
            node_id: "n1".to_string(),
            timestamp_ms: 1010,
            state_snapshot: json!({"result": "done"}),
            duration_ms: 10,
        };

        recorder.record_event("s-1", &event1).await.unwrap();
        recorder.record_event("s-1", &event2).await.unwrap();

        let events = recorder.get_events("s-1").await.unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type(), "node_start");
        assert_eq!(events[1].event_type(), "node_end");

        // Check event count is updated
        let s = recorder.get_session("s-1").await.unwrap().unwrap();
        assert_eq!(s.event_count, 2);
    }

    #[tokio::test]
    async fn test_in_memory_recorder_end_session() {
        let recorder = InMemorySessionRecorder::new();
        let session = DebugSession::new("s-1", "wf-1", "exec-1");
        recorder.start_session(&session).await.unwrap();

        recorder.end_session("s-1", "completed").await.unwrap();

        let s = recorder.get_session("s-1").await.unwrap().unwrap();
        assert_eq!(s.status, "completed");
        assert!(s.ended_at.is_some());
    }

    #[tokio::test]
    async fn test_in_memory_recorder_list_sessions() {
        let recorder = InMemorySessionRecorder::new();
        recorder
            .start_session(&DebugSession::new("s-1", "wf-1", "e-1"))
            .await
            .unwrap();
        recorder
            .start_session(&DebugSession::new("s-2", "wf-2", "e-2"))
            .await
            .unwrap();

        let sessions = recorder.list_sessions().await.unwrap();
        assert_eq!(sessions.len(), 2);
    }

    #[tokio::test]
    async fn test_in_memory_recorder_missing_session() {
        let recorder = InMemorySessionRecorder::new();

        // record_event for non-existent session should fail
        let event = DebugEvent::NodeStart {
            node_id: "n1".to_string(),
            timestamp_ms: 0,
            state_snapshot: json!(null),
        };
        let result = recorder.record_event("nonexistent", &event).await;
        assert!(result.is_err());

        // end_session for non-existent session should fail
        let result = recorder.end_session("nonexistent", "failed").await;
        assert!(result.is_err());

        // get_session for non-existent session should return None
        let result = recorder.get_session("nonexistent").await.unwrap();
        assert!(result.is_none());

        // get_events for non-existent session should return empty
        let events = recorder.get_events("nonexistent").await.unwrap();
        assert!(events.is_empty());
    }
}
