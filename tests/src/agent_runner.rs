//! Real agent runner harness for integration-style tests.
//!
//! Provides a lightweight wrapper around the MoFA runtime `AgentRunner`
//! with an isolated workspace and deterministic mock LLM.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use mofa_foundation::agent::executor::{AgentExecutor, AgentExecutorConfig};
use mofa_kernel::agent::context::AgentContext;
use mofa_kernel::agent::core::MoFAAgent;
use mofa_kernel::agent::error::{AgentError, AgentResult};
use mofa_kernel::agent::types::{AgentInput, AgentOutput, ChatCompletionRequest};
use mofa_kernel::agent::types::{ChatCompletionResponse, ToolCall};
use mofa_kernel::agent::AgentCapabilities;
use mofa_runtime::runner::{AgentRunner, RunnerState, RunnerStats};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Errors returned by the agent runner harness itself.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum AgentRunnerError {
    #[error("failed to create test workspace: {0}")]
    WorkspaceIo(#[from] std::io::Error),

    #[error("agent runner failure: {0}")]
    Agent(#[from] AgentError),
}

/// Metadata captured for each run.
#[derive(Debug, Clone)]
pub struct AgentRunMetadata {
    pub agent_id: String,
    pub agent_name: String,
    pub execution_id: String,
    pub session_id: Option<String>,
    pub workspace_root: PathBuf,
    pub runner_state: RunnerState,
    pub runner_stats: RunnerStats,
    pub started_at: DateTime<Utc>,
}

/// Result of a single agent run.
#[derive(Debug)]
pub struct AgentRunResult {
    pub output: Option<AgentOutput>,
    pub error: Option<AgentError>,
    pub duration: Duration,
    pub metadata: AgentRunMetadata,
}

impl AgentRunResult {
    pub fn is_success(&self) -> bool {
        self.error.is_none()
    }

    pub fn output_text(&self) -> Option<String> {
        self.output.as_ref().map(AgentOutput::to_text)
    }
}

/// Simple deterministic LLM provider for tests.
#[derive(Debug)]
pub struct MockAgentLLMProvider {
    name: String,
    responses: RwLock<VecDeque<String>>,
    default_response: RwLock<String>,
}

impl MockAgentLLMProvider {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            responses: RwLock::new(VecDeque::new()),
            default_response: RwLock::new("This is a mock response.".to_string()),
        }
    }

    pub async fn add_response(&self, response: impl Into<String>) {
        self.responses.write().await.push_back(response.into());
    }

    pub async fn set_default_response(&self, response: impl Into<String>) {
        *self.default_response.write().await = response.into();
    }

    pub async fn pending_responses(&self) -> usize {
        self.responses.read().await.len()
    }
}

#[async_trait]
impl mofa_kernel::agent::types::LLMProvider for MockAgentLLMProvider {
    fn name(&self) -> &str {
        &self.name
    }

    async fn chat(
        &self,
        _request: ChatCompletionRequest,
    ) -> AgentResult<ChatCompletionResponse> {
        let response = {
            let mut responses = self.responses.write().await;
            if let Some(next) = responses.pop_front() {
                next
            } else {
                self.default_response.read().await.clone()
            }
        };

        Ok(ChatCompletionResponse {
            content: Some(response),
            tool_calls: Some(Vec::<ToolCall>::new()),
            usage: None,
        })
    }
}

struct SessionAwareExecutor {
    executor: AgentExecutor,
}

impl SessionAwareExecutor {
    fn new(executor: AgentExecutor) -> Self {
        Self { executor }
    }
}

#[async_trait]
impl MoFAAgent for SessionAwareExecutor {
    fn id(&self) -> &str {
        self.executor.id()
    }

    fn name(&self) -> &str {
        self.executor.name()
    }

    fn capabilities(&self) -> &AgentCapabilities {
        self.executor.capabilities()
    }

    fn state(&self) -> mofa_kernel::agent::AgentState {
        self.executor.state()
    }

    async fn initialize(&mut self, ctx: &AgentContext) -> AgentResult<()> {
        self.executor.initialize(ctx).await
    }

    async fn execute(
        &mut self,
        input: AgentInput,
        ctx: &AgentContext,
    ) -> AgentResult<AgentOutput> {
        let message = input.as_text().unwrap_or("");
        let session_key = ctx.session_id.as_deref().unwrap_or("default");
        let response = self.executor.process_message(session_key, message).await?;
        Ok(AgentOutput::text(response))
    }

    async fn shutdown(&mut self) -> AgentResult<()> {
        self.executor.shutdown().await
    }
}

struct TempWorkspace {
    root: PathBuf,
}

impl TempWorkspace {
    fn new(prefix: &str) -> Result<Self, AgentRunnerError> {
        let root = std::env::temp_dir().join(format!("{}-{}", prefix, Uuid::now_v7()));
        std::fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    fn path(&self) -> &Path {
        &self.root
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// Test harness for running real agent execution paths.
pub struct AgentTestRunner {
    workspace: TempWorkspace,
    session_id: String,
    execution_id: String,
    llm: Arc<MockAgentLLMProvider>,
    runner: AgentRunner<SessionAwareExecutor>,
}

impl AgentTestRunner {
    pub async fn new() -> Result<Self, AgentRunnerError> {
        Self::with_config(AgentExecutorConfig::default()).await
    }

    pub async fn with_config(config: AgentExecutorConfig) -> Result<Self, AgentRunnerError> {
        let workspace = TempWorkspace::new("mofa-agent-test")?;
        let llm = Arc::new(MockAgentLLMProvider::new("mock-llm"));
        let executor = AgentExecutor::with_config(llm.clone(), workspace.path(), config).await?;
        let agent = SessionAwareExecutor::new(executor);

        let execution_id = Uuid::now_v7().to_string();
        let session_id = Uuid::now_v7().to_string();
        let context = AgentContext::with_session(&execution_id, &session_id);

        let runner = AgentRunner::with_context(agent, context).await?;

        Ok(Self {
            workspace,
            session_id,
            execution_id,
            llm,
            runner,
        })
    }

    pub fn workspace(&self) -> &Path {
        self.workspace.path()
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn execution_id(&self) -> &str {
        &self.execution_id
    }

    pub fn mock_llm(&self) -> Arc<MockAgentLLMProvider> {
        Arc::clone(&self.llm)
    }

    pub async fn run_text(&mut self, input: &str) -> Result<AgentRunResult, AgentRunnerError> {
        self.run_input(AgentInput::text(input)).await
    }

    pub async fn run_input(
        &mut self,
        input: AgentInput,
    ) -> Result<AgentRunResult, AgentRunnerError> {
        let started_at = Utc::now();
        let timer = Instant::now();
        let result = self.runner.execute(input).await;
        let duration = timer.elapsed();

        let (output, error) = match result {
            Ok(output) => (Some(output), None),
            Err(err) => (None, Some(err)),
        };

        let metadata = AgentRunMetadata {
            agent_id: self.runner.agent().id().to_string(),
            agent_name: self.runner.agent().name().to_string(),
            execution_id: self.runner.context().execution_id.clone(),
            session_id: self.runner.context().session_id.clone(),
            workspace_root: self.workspace.path().to_path_buf(),
            runner_state: self.runner.state().await,
            runner_stats: self.runner.stats().await,
            started_at,
        };

        Ok(AgentRunResult {
            output,
            error,
            duration,
            metadata,
        })
    }

    pub async fn shutdown(self) -> Result<(), AgentRunnerError> {
        self.runner.shutdown().await?;
        Ok(())
    }
}
