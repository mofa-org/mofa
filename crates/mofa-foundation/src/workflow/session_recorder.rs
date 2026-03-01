//! Session Recorder Implementations
//!
//! Concrete implementations of `SessionRecorder` for persisting debug sessions.

use async_trait::async_trait;
use mofa_kernel::agent::error::{AgentError, AgentResult};
use mofa_kernel::workflow::telemetry::{DebugEvent, DebugSession, SessionRecorder};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs::{self, OpenOptions};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::RwLock;
use tracing::warn;

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

// ============================================================================
// FileSessionRecorder
// ============================================================================

/// Configuration for [`FileSessionRecorder`].
#[derive(Debug, Clone)]
pub struct FileSessionRecorderConfig {
    /// Directory where session files are stored.
    pub session_dir: PathBuf,
    /// Maximum number of session files to retain (0 = unlimited).
    pub max_sessions: usize,
    /// Maximum size in bytes for each session file (0 = unlimited).
    pub max_file_size: u64,
}

impl Default for FileSessionRecorderConfig {
    fn default() -> Self {
        Self {
            session_dir: PathBuf::from(".mofa/sessions"),
            max_sessions: 100,
            max_file_size: 50 * 1024 * 1024,
        }
    }
}

/// JSONL-backed recorder for debug sessions.
///
/// Format:
/// - Header line: `{"schema_version":1,"session":{...}}`
/// - Event line(s): `{"event":{...DebugEvent...}}`
/// - Footer line: `{"session_end":{"ended_at":...,"status":"...","event_count":...}}`
pub struct FileSessionRecorder {
    config: FileSessionRecorderConfig,
    active_sessions: Arc<RwLock<HashMap<String, DebugSession>>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SessionHeaderLine {
    schema_version: u32,
    session: DebugSession,
}

#[derive(Debug, Serialize, Deserialize)]
struct SessionEventLine {
    event: DebugEvent,
}

#[derive(Debug, Serialize, Deserialize)]
struct SessionEndLine {
    session_end: SessionEndMetadata,
}

#[derive(Debug, Serialize, Deserialize)]
struct SessionEndMetadata {
    ended_at: u64,
    status: String,
    event_count: u64,
}

impl FileSessionRecorder {
    /// Create a file recorder from explicit config.
    pub fn with_config(config: FileSessionRecorderConfig) -> Self {
        Self {
            config,
            active_sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create a file recorder with default config.
    pub fn new() -> Self {
        Self::with_config(FileSessionRecorderConfig::default())
    }

    fn session_path(&self, session_id: &str) -> PathBuf {
        self.config
            .session_dir
            .join(format!("{}.jsonl", session_id))
    }

    fn session_tmp_path(&self, session_id: &str) -> PathBuf {
        self.config
            .session_dir
            .join(format!("{}.jsonl.tmp", session_id))
    }

    async fn append_json_line<T: Serialize>(path: &Path, payload: &T) -> AgentResult<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .await?;
        let mut line = serde_json::to_string(payload)?;
        line.push('\n');
        file.write_all(line.as_bytes()).await?;
        file.flush().await?;
        Ok(())
    }

    async fn load_session_from_file(path: &Path) -> AgentResult<Option<DebugSession>> {
        let file = match fs::File::open(path).await {
            Ok(file) => file,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(err.into()),
        };
        let mut lines = BufReader::new(file).lines();
        let Some(first_line) = lines.next_line().await? else {
            return Ok(None);
        };

        let header: SessionHeaderLine = serde_json::from_str(&first_line)?;
        let mut session = header.session;

        while let Some(line) = lines.next_line().await? {
            if let Ok(footer) = serde_json::from_str::<SessionEndLine>(&line) {
                session.ended_at = Some(footer.session_end.ended_at);
                session.status = footer.session_end.status;
                session.event_count = footer.session_end.event_count;
            }
        }

        Ok(Some(session))
    }

    async fn prune_sessions_if_needed(&self) -> AgentResult<()> {
        if self.config.max_sessions == 0 {
            return Ok(());
        }

        let mut sessions = self.list_sessions().await?;
        if sessions.len() <= self.config.max_sessions {
            return Ok(());
        }

        let active_ids: std::collections::HashSet<String> =
            self.active_sessions.read().await.keys().cloned().collect();

        let mut completed = sessions
            .drain(..)
            .filter(|s| !active_ids.contains(&s.session_id) && s.status != "running")
            .collect::<Vec<_>>();
        completed.sort_by_key(|s| s.started_at);

        let excess = self
            .list_sessions()
            .await?
            .len()
            .saturating_sub(self.config.max_sessions);
        for session in completed.into_iter().take(excess) {
            let path = self.session_path(&session.session_id);
            if let Err(err) = fs::remove_file(&path).await {
                warn!("failed to prune session file '{}': {}", path.display(), err);
            }
        }

        Ok(())
    }
}

impl Default for FileSessionRecorder {
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

#[async_trait]
impl SessionRecorder for FileSessionRecorder {
    async fn start_session(&self, session: &DebugSession) -> AgentResult<()> {
        fs::create_dir_all(&self.config.session_dir).await?;

        let tmp_path = self.session_tmp_path(&session.session_id);
        let final_path = self.session_path(&session.session_id);

        let header = SessionHeaderLine {
            schema_version: 1,
            session: session.clone(),
        };
        let mut header_line = serde_json::to_string(&header)?;
        header_line.push('\n');

        fs::write(&tmp_path, header_line.as_bytes()).await?;
        fs::rename(&tmp_path, &final_path).await?;

        self.active_sessions
            .write()
            .await
            .insert(session.session_id.clone(), session.clone());

        self.prune_sessions_if_needed().await?;
        Ok(())
    }

    async fn record_event(&self, session_id: &str, event: &DebugEvent) -> AgentResult<()> {
        let path = self.session_path(session_id);

        {
            let sessions = self.active_sessions.read().await;
            if !sessions.contains_key(session_id) {
                return Err(AgentError::InvalidInput(format!(
                    "Session not found: {}",
                    session_id
                )));
            }
        }

        if self.config.max_file_size > 0 {
            let file_size = match fs::metadata(&path).await {
                Ok(metadata) => metadata.len(),
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => 0,
                Err(err) => return Err(err.into()),
            };
            if file_size >= self.config.max_file_size {
                warn!(
                    "session '{}' reached max_file_size ({} bytes), skipping event",
                    session_id, self.config.max_file_size
                );
                return Ok(());
            }
        }

        Self::append_json_line(
            &path,
            &SessionEventLine {
                event: event.clone(),
            },
        )
        .await?;

        let mut sessions = self.active_sessions.write().await;
        if let Some(session) = sessions.get_mut(session_id) {
            session.event_count += 1;
        }

        Ok(())
    }

    async fn end_session(&self, session_id: &str, status: &str) -> AgentResult<()> {
        let mut sessions = self.active_sessions.write().await;
        let mut session = sessions.remove(session_id).ok_or_else(|| {
            AgentError::InvalidInput(format!("Session not found: {}", session_id))
        })?;

        let ended_at = DebugEvent::now_ms();
        session.ended_at = Some(ended_at);
        session.status = status.to_string();

        let footer = SessionEndLine {
            session_end: SessionEndMetadata {
                ended_at,
                status: status.to_string(),
                event_count: session.event_count,
            },
        };
        Self::append_json_line(&self.session_path(session_id), &footer).await?;
        Ok(())
    }

    async fn get_session(&self, session_id: &str) -> AgentResult<Option<DebugSession>> {
        if let Some(session) = self.active_sessions.read().await.get(session_id).cloned() {
            return Ok(Some(session));
        }
        Self::load_session_from_file(&self.session_path(session_id)).await
    }

    async fn get_events(&self, session_id: &str) -> AgentResult<Vec<DebugEvent>> {
        let path = self.session_path(session_id);
        let file = match fs::File::open(&path).await {
            Ok(file) => file,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(err) => return Err(err.into()),
        };

        let mut lines = BufReader::new(file).lines();
        let _ = lines.next_line().await?;

        let mut events = Vec::new();
        while let Some(line) = lines.next_line().await? {
            if let Ok(event_line) = serde_json::from_str::<SessionEventLine>(&line) {
                events.push(event_line.event);
                continue;
            }
            if serde_json::from_str::<SessionEndLine>(&line).is_ok() {
                continue;
            }
            warn!(
                "skipping unparseable session event line in '{}'",
                path.display()
            );
        }

        Ok(events)
    }

    async fn list_sessions(&self) -> AgentResult<Vec<DebugSession>> {
        let mut sessions = HashMap::<String, DebugSession>::new();

        match fs::read_dir(&self.config.session_dir).await {
            Ok(mut read_dir) => {
                while let Some(entry) = read_dir.next_entry().await? {
                    let path = entry.path();
                    if path.extension().and_then(|s| s.to_str()) != Some("jsonl") {
                        continue;
                    }
                    if let Some(session) = Self::load_session_from_file(&path).await? {
                        sessions.insert(session.session_id.clone(), session);
                    }
                }
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => return Err(err.into()),
        }

        for (id, session) in self.active_sessions.read().await.iter() {
            sessions.insert(id.clone(), session.clone());
        }

        let mut result = sessions.into_values().collect::<Vec<_>>();
        result.sort_by(|a, b| b.started_at.cmp(&a.started_at));
        Ok(result)
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
    use tempfile::TempDir;

    fn file_recorder_with_tempdir(max_sessions: usize) -> (FileSessionRecorder, TempDir) {
        let temp_dir = tempfile::tempdir().unwrap();
        let recorder = FileSessionRecorder::with_config(FileSessionRecorderConfig {
            session_dir: temp_dir.path().to_path_buf(),
            max_sessions,
            max_file_size: 0,
        });
        (recorder, temp_dir)
    }

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

    #[tokio::test]
    async fn test_file_recorder_round_trip() {
        let (recorder, _temp_dir) = file_recorder_with_tempdir(0);
        let session = DebugSession::new("s-1", "wf-1", "exec-1");

        recorder.start_session(&session).await.unwrap();
        recorder
            .record_event(
                "s-1",
                &DebugEvent::NodeStart {
                    node_id: "n1".to_string(),
                    timestamp_ms: 100,
                    state_snapshot: json!({"a": 1}),
                },
            )
            .await
            .unwrap();
        recorder
            .record_event(
                "s-1",
                &DebugEvent::NodeEnd {
                    node_id: "n1".to_string(),
                    timestamp_ms: 110,
                    state_snapshot: json!({"a": 2}),
                    duration_ms: 10,
                },
            )
            .await
            .unwrap();
        recorder.end_session("s-1", "completed").await.unwrap();

        let events = recorder.get_events("s-1").await.unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type(), "node_start");
        assert_eq!(events[1].event_type(), "node_end");

        let session = recorder.get_session("s-1").await.unwrap().unwrap();
        assert_eq!(session.status, "completed");
        assert!(session.ended_at.is_some());
        assert_eq!(session.event_count, 2);
    }

    #[tokio::test]
    async fn test_file_recorder_list_sessions() {
        let (recorder, _temp_dir) = file_recorder_with_tempdir(0);

        let mut s1 = DebugSession::new("s-1", "wf", "e-1");
        s1.started_at = 1;
        let mut s2 = DebugSession::new("s-2", "wf", "e-2");
        s2.started_at = 2;
        let mut s3 = DebugSession::new("s-3", "wf", "e-3");
        s3.started_at = 3;

        recorder.start_session(&s1).await.unwrap();
        recorder.end_session("s-1", "completed").await.unwrap();
        recorder.start_session(&s2).await.unwrap();
        recorder.end_session("s-2", "completed").await.unwrap();
        recorder.start_session(&s3).await.unwrap();
        recorder.end_session("s-3", "completed").await.unwrap();

        let sessions = recorder.list_sessions().await.unwrap();
        assert_eq!(sessions.len(), 3);
        assert_eq!(sessions[0].session_id, "s-3");
        assert_eq!(sessions[1].session_id, "s-2");
        assert_eq!(sessions[2].session_id, "s-1");
    }

    #[tokio::test]
    async fn test_file_recorder_missing_session() {
        let (recorder, _temp_dir) = file_recorder_with_tempdir(0);
        let result = recorder
            .record_event(
                "missing",
                &DebugEvent::Error {
                    node_id: None,
                    timestamp_ms: 0,
                    error: "x".to_string(),
                },
            )
            .await;
        assert!(result.is_err());

        let result = recorder.end_session("missing", "failed").await;
        assert!(result.is_err());

        let session = recorder.get_session("missing").await.unwrap();
        assert!(session.is_none());

        let events = recorder.get_events("missing").await.unwrap();
        assert!(events.is_empty());
    }

    #[tokio::test]
    async fn test_file_recorder_crash_recovery() {
        let (recorder, temp_dir) = file_recorder_with_tempdir(0);
        let session = DebugSession::new("s-crash", "wf", "e");
        recorder.start_session(&session).await.unwrap();

        recorder
            .record_event(
                "s-crash",
                &DebugEvent::NodeStart {
                    node_id: "a".to_string(),
                    timestamp_ms: 1,
                    state_snapshot: json!({}),
                },
            )
            .await
            .unwrap();
        recorder
            .record_event(
                "s-crash",
                &DebugEvent::NodeEnd {
                    node_id: "a".to_string(),
                    timestamp_ms: 2,
                    state_snapshot: json!({}),
                    duration_ms: 1,
                },
            )
            .await
            .unwrap();

        let mut file = OpenOptions::new()
            .append(true)
            .open(temp_dir.path().join("s-crash.jsonl"))
            .await
            .unwrap();
        file.write_all(b"{bad-json\n").await.unwrap();
        file.flush().await.unwrap();

        let events = recorder.get_events("s-crash").await.unwrap();
        assert_eq!(events.len(), 2);
    }

    #[tokio::test]
    async fn test_file_recorder_session_pruning() {
        let (recorder, _temp_dir) = file_recorder_with_tempdir(2);

        let mut s1 = DebugSession::new("s-1", "wf", "e-1");
        s1.started_at = 1;
        recorder.start_session(&s1).await.unwrap();
        recorder.end_session("s-1", "completed").await.unwrap();

        let mut s2 = DebugSession::new("s-2", "wf", "e-2");
        s2.started_at = 2;
        recorder.start_session(&s2).await.unwrap();
        recorder.end_session("s-2", "completed").await.unwrap();

        let mut s3 = DebugSession::new("s-3", "wf", "e-3");
        s3.started_at = 3;
        recorder.start_session(&s3).await.unwrap();
        recorder.end_session("s-3", "completed").await.unwrap();

        let sessions = recorder.list_sessions().await.unwrap();
        assert_eq!(sessions.len(), 2);
        assert!(sessions.iter().any(|s| s.session_id == "s-2"));
        assert!(sessions.iter().any(|s| s.session_id == "s-3"));
    }

    #[tokio::test]
    async fn test_file_recorder_concurrent_sessions() {
        let (recorder, _temp_dir) = file_recorder_with_tempdir(0);
        let recorder = Arc::new(recorder);

        let mut tasks = Vec::new();
        for idx in 0..3usize {
            let recorder = recorder.clone();
            tasks.push(tokio::spawn(async move {
                let session_id = format!("s-{idx}");
                let session = DebugSession::new(session_id.clone(), "wf", format!("exec-{idx}"));
                recorder.start_session(&session).await.unwrap();

                for event_idx in 0..5usize {
                    recorder
                        .record_event(
                            &session_id,
                            &DebugEvent::NodeStart {
                                node_id: format!("n-{idx}-{event_idx}"),
                                timestamp_ms: event_idx as u64,
                                state_snapshot: json!({"idx": idx, "event": event_idx}),
                            },
                        )
                        .await
                        .unwrap();
                }

                recorder
                    .end_session(&session_id, "completed")
                    .await
                    .unwrap();
            }));
        }

        for task in tasks {
            task.await.unwrap();
        }

        for idx in 0..3usize {
            let session_id = format!("s-{idx}");
            let events = recorder.get_events(&session_id).await.unwrap();
            assert_eq!(events.len(), 5);
            assert!(events.iter().all(|event| {
                event
                    .node_id()
                    .unwrap_or_default()
                    .starts_with(&format!("n-{idx}-"))
            }));
        }
    }
}
