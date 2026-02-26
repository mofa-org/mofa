//! Task orchestration framework for background task spawning
//!
//! This module provides:
//! - Background task spawning via tokio::spawn
//! - Origin-based result routing
//! - Task lifecycle management
//! - Result streaming via channels

use crate::llm::LLMProvider;
use mofa_kernel::agent::types::error::{GlobalError, GlobalResult};
use chrono::{DateTime, Utc};
use tracing::Instrument;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::{RwLock, broadcast};
use tokio::task::JoinHandle;
use uuid::Uuid;

/// Where to route task results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskOrigin {
    /// Routing key (e.g., "channel:chat_id")
    pub routing_key: String,
    /// Optional metadata
    #[serde(flatten)]
    pub metadata: HashMap<String, Value>,
}

impl TaskOrigin {
    /// Create a new task origin
    pub fn new(routing_key: impl Into<String>) -> Self {
        Self {
            routing_key: routing_key.into(),
            metadata: HashMap::new(),
        }
    }

    /// Create from channel and chat_id
    pub fn from_channel(channel: &str, chat_id: &str) -> Self {
        Self::new(format!("{}:{}", channel, chat_id))
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: impl Into<String>, value: Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }
}

/// Background task status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaskStatus {
    /// Task is pending
    Pending,
    /// Task is running
    Running,
    /// Task completed successfully
    Completed(String),
    /// Task failed
    Failed(String),
}

/// Background task with origin tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackgroundTask {
    /// Unique task ID
    pub id: String,
    /// Task prompt/description
    pub prompt: String,
    /// Where to route results
    pub origin: TaskOrigin,
    /// Current status
    pub status: TaskStatus,
    /// When the task started
    pub started_at: DateTime<Utc>,
    /// When the task completed (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<DateTime<Utc>>,
}

impl BackgroundTask {
    /// Create a new background task
    pub fn new(prompt: impl Into<String>, origin: TaskOrigin) -> Self {
        Self {
            id: Uuid::new_v4().to_string()[..8].to_string(),
            prompt: prompt.into(),
            origin,
            status: TaskStatus::Pending,
            started_at: Utc::now(),
            completed_at: None,
        }
    }

    /// Mark as running
    pub fn mark_running(&mut self) {
        self.status = TaskStatus::Running;
    }

    /// Mark as completed
    pub fn mark_completed(&mut self, result: impl Into<String>) {
        self.status = TaskStatus::Completed(result.into());
        self.completed_at = Some(Utc::now());
    }

    /// Mark as failed
    pub fn mark_failed(&mut self, error: impl Into<String>) {
        self.status = TaskStatus::Failed(error.into());
        self.completed_at = Some(Utc::now());
    }

    /// Check if task is finished
    pub fn is_finished(&self) -> bool {
        matches!(
            self.status,
            TaskStatus::Completed(_) | TaskStatus::Failed(_)
        )
    }
}

/// Result from a completed task
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    /// Task ID
    pub task_id: String,
    /// Where to route the result
    pub origin: TaskOrigin,
    /// Result content
    pub content: String,
    /// Whether the task succeeded
    pub success: bool,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

impl TaskResult {
    /// Create a successful result
    pub fn success(
        task_id: impl Into<String>,
        origin: TaskOrigin,
        content: impl Into<String>,
    ) -> Self {
        Self {
            task_id: task_id.into(),
            origin,
            content: content.into(),
            success: true,
            timestamp: Utc::now(),
        }
    }

    /// Create a failed result
    pub fn failure(
        task_id: impl Into<String>,
        origin: TaskOrigin,
        error: impl Into<String>,
    ) -> Self {
        Self {
            task_id: task_id.into(),
            origin,
            content: error.into(),
            success: false,
            timestamp: Utc::now(),
        }
    }
}

/// Configuration for task orchestrator
#[derive(Debug, Clone)]
pub struct TaskOrchestratorConfig {
    /// Maximum concurrent tasks
    pub max_concurrent_tasks: usize,
    /// Default model for tasks
    pub default_model: String,
}

impl Default for TaskOrchestratorConfig {
    fn default() -> Self {
        Self {
            max_concurrent_tasks: 10,
            default_model: "gpt-4o-mini".to_string(),
        }
    }
}

/// Orchestrator for background tasks
pub struct TaskOrchestrator {
    /// LLM provider for tasks
    provider: Arc<dyn LLMProvider>,
    /// Active tasks (Running + recently-completed tasks within the retention window)
    active_tasks: Arc<RwLock<HashMap<String, BackgroundTask>>>,
    /// Result sender
    result_sender: broadcast::Sender<TaskResult>,
    /// Configuration
    config: TaskOrchestratorConfig,
    /// JoinHandles for all inflight tasks; aborted on drop to prevent zombie tasks
    handles: Arc<Mutex<HashMap<String, JoinHandle<()>>>>,
}

impl Drop for TaskOrchestrator {
    fn drop(&mut self) {
        // Abort every inflight task (including those sleeping the cleanup delay) so
        // that no spawned task outlives the orchestrator, preventing zombie tasks,
        // Arc retention of active_tasks, and dangling broadcast::Sender clones.
        if let Ok(mut handles) = self.handles.lock() {
            for (_, handle) in handles.drain() {
                handle.abort();
            }
        }
    }
}

impl TaskOrchestrator {
    /// Create a new task orchestrator
    pub fn new(provider: Arc<dyn LLMProvider>, config: TaskOrchestratorConfig) -> Self {
        let (result_sender, _) = broadcast::channel(100);

        Self {
            provider,
            active_tasks: Arc::new(RwLock::new(HashMap::new())),
            result_sender,
            config,
            handles: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Create with default configuration
    pub fn with_defaults(provider: Arc<dyn LLMProvider>) -> Self {
        Self::new(provider, TaskOrchestratorConfig::default())
    }

    /// Spawn a background task
    pub async fn spawn(&self, prompt: &str, origin: TaskOrigin) -> GlobalResult<String> {
        // Count only tasks that are still actively running, not completed/failed
        // tasks that are waiting in the cleanup retention window. Counting finished
        // tasks caused spurious "max concurrent tasks reached" errors after a burst
        // of quick completions, blocking submissions for up to 5 minutes.
        let running_count = self
            .active_tasks
            .read()
            .await
            .values()
            .filter(|t| t.status == TaskStatus::Running)
            .count();
        if running_count >= self.config.max_concurrent_tasks {
            return Err(GlobalError::Other(format!(
                "Maximum concurrent tasks ({}) reached",
                self.config.max_concurrent_tasks
            )));
        }

        // Create task
        let mut task = BackgroundTask::new(prompt, origin.clone());
        let task_id = task.id.clone();
        task.mark_running();

        // Store task
        self.active_tasks
            .write()
            .await
            .insert(task_id.clone(), task.clone());

        // Spawn background task
        let provider = Arc::clone(&self.provider);
        let active_tasks = Arc::clone(&self.active_tasks);
        let result_sender = self.result_sender.clone();
        let model = self.config.default_model.clone();
        let prompt = prompt.to_string();
        let task_id_clone = task_id.clone();
        let handles = Arc::clone(&self.handles);

let span = tracing::info_span!(
    "task_orchestrator.background",
    task_id = %task_id_clone
);

let handle = tokio::spawn(async move {
    let _enter = span.enter();

    let result = Self::run_task(&provider, &model, &prompt).await;
            

            // Update task status
            {
                let mut tasks = active_tasks.write().await;
                if let Some(task) = tasks.get_mut(&task_id_clone) {
                    match &result {
                        Ok(content) => task.mark_completed(content),
                        Err(e) => task.mark_failed(e.to_string()),
                    }
                }
            }

            // Send result
            let task_result = match &result {
                Ok(content) => TaskResult::success(&task_id_clone, origin, content),
                Err(e) => TaskResult::failure(&task_id_clone, origin, e.to_string()),
            };

            let _ = result_sender.send(task_result);

            // Retain the completed task entry briefly so callers can poll its
            // status after receiving the broadcast result, then clean up.
            tokio::time::sleep(tokio::time::Duration::from_secs(300)).await;
<<<<<<< HEAD
            let mut tasks = active_tasks.write().await;
            tasks.remove(&task_id_clone);
        }.instrument(span));
=======

            active_tasks.write().await.remove(&task_id_clone);

            // Remove our own handle now that we have finished all work.
            if let Ok(mut h) = handles.lock() {
                h.remove(&task_id_clone);
            }
        });
>>>>>>> 53aa163d (fix(task-orchestrator): correct concurrency gate and add JoinHandle abort-on-drop)

        // Store the handle so Drop can abort it if the orchestrator is torn down
        // before this task (including its cleanup sleep) has finished.
        if let Ok(mut h) = self.handles.lock() {
            h.insert(task_id.clone(), handle);
        }

        Ok(task_id)
    }

    /// Run a single task to completion
    async fn run_task(
        provider: &Arc<dyn LLMProvider>,
        model: &str,
        prompt: &str,
    ) -> GlobalResult<String> {
        use crate::llm::types::ChatCompletionRequest;

        let request = ChatCompletionRequest::new(model)
            .system(
                "You are a helpful assistant. Complete the given task thoroughly and concisely.",
            )
            .user(prompt);

        let response = provider.chat(request).await.map_err(|e| GlobalError::Other(e.to_string()))?;

        response
            .content()
            .map(|s| s.to_string())
            .ok_or_else(|| GlobalError::Other("No response content".to_string()))
    }

    /// Subscribe to task results
    pub fn subscribe_results(&self) -> broadcast::Receiver<TaskResult> {
        self.result_sender.subscribe()
    }

    /// Get all active tasks
    pub async fn get_active_tasks(&self) -> Vec<BackgroundTask> {
        self.active_tasks.read().await.values().cloned().collect()
    }

    /// Get a specific task
    pub async fn get_task(&self, task_id: &str) -> Option<BackgroundTask> {
        self.active_tasks.read().await.get(task_id).cloned()
    }

    /// Get the configuration
    pub fn config(&self) -> &TaskOrchestratorConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::types::{
        ChatCompletionResponse, ChatMessage, Choice, FinishReason, LLMResult,
    };
    use async_trait::async_trait;

    struct InstantProvider;

    #[async_trait]
    impl LLMProvider for InstantProvider {
        fn name(&self) -> &str {
            "instant"
        }

        async fn chat(
            &self,
            _req: crate::llm::types::ChatCompletionRequest,
        ) -> LLMResult<ChatCompletionResponse> {
            Ok(ChatCompletionResponse {
                id: "test".to_string(),
                object: "chat.completion".to_string(),
                created: 0,
                model: "instant".to_string(),
                choices: vec![Choice {
                    index: 0,
                    message: ChatMessage::assistant("done"),
                    finish_reason: Some(FinishReason::Stop),
                    logprobs: None,
                }],
                usage: None,
                system_fingerprint: None,
            })
        }
    }

    /// Regression test: after max_concurrent_tasks quick completions, the next
    /// spawn() call must succeed immediately rather than returning the
    /// concurrency-limit error caused by counting finished-but-not-yet-gc'd tasks.
    #[tokio::test]
    async fn test_gate_counts_only_running_tasks() {
        let config = TaskOrchestratorConfig {
            max_concurrent_tasks: 2,
            default_model: "instant".into(),
        };
        let orchestrator = TaskOrchestrator::new(Arc::new(InstantProvider), config);
        let mut rx = orchestrator.subscribe_results();

        // Submit exactly max_concurrent_tasks tasks and wait for both to finish.
        let id1 = orchestrator
            .spawn("task 1", TaskOrigin::new("test"))
            .await
            .expect("spawn 1 should succeed");
        let id2 = orchestrator
            .spawn("task 2", TaskOrigin::new("test"))
            .await
            .expect("spawn 2 should succeed");

        // Drain results so we know both tasks have completed.
        let mut finished = std::collections::HashSet::new();
        while finished.len() < 2 {
            let r = rx.recv().await.expect("result expected");
            finished.insert(r.task_id.clone());
        }

        // Both tasks are now Completed but still in active_tasks (cleanup sleep).
        // The concurrency gate must NOT count them as running.
        let id3 = orchestrator
            .spawn("task 3", TaskOrigin::new("test"))
            .await;
        assert!(
            id3.is_ok(),
            "spawn after completions must succeed; got: {:?}",
            id3
        );

        // Verify the earlier tasks really are in Completed state (not removed yet).
        let t1 = orchestrator.get_task(&id1).await;
        let t2 = orchestrator.get_task(&id2).await;
        assert!(t1.map(|t| t.is_finished()).unwrap_or(true));
        assert!(t2.map(|t| t.is_finished()).unwrap_or(true));
    }
}
