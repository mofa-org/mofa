//! AgentBuilder example
//!
//! Demonstrates constructing agents via the fluent [`AgentBuilder`] API and
//! loading them from TOML/YAML profiles.
//!
//! Two agents are built — an "analyst" and a "researcher" — registered in an
//! [`AgentRegistry`], and a task is routed to one of them.  A mock LLM is
//! used so the example runs without an API key.
//!
//! To adapt this for a real OpenAI provider, wrap it in a newtype that
//! implements `mofa_sdk::agent::LLMProvider` (the kernel trait).
//!
//! # Usage
//!
//! ```bash
//! cargo run -p agent_builder
//! ```

use std::sync::Arc;

use async_trait::async_trait;
use tracing::info;

use mofa_sdk::agent::{AgentBuilder, AgentProfile, AgentRegistry, LLMProvider};
use mofa_sdk::kernel::{AgentResult, ChatCompletionRequest, ChatCompletionResponse, TokenUsage};

// ---------------------------------------------------------------------------
// Inline TOML profile for the analyst agent
// ---------------------------------------------------------------------------
const ANALYST_TOML: &str = r#"
name = "analyst"
description = "Financial analysis specialist"
system_prompt = "You are a financial analyst. Provide concise, data-driven assessments."
model = "gpt-4o"
max_iterations = 6
temperature = 0.2
max_tokens = 1024
"#;

// ---------------------------------------------------------------------------
// Inline YAML profile for the researcher agent
// ---------------------------------------------------------------------------
const RESEARCHER_YAML: &str = "
name: researcher
description: General-purpose research assistant
system_prompt: |
  You are a thorough research assistant. Break complex questions into
  sub-questions, gather evidence, and synthesise a clear answer.
model: gpt-4o-mini
max_iterations: 10
temperature: 0.5
";

// ---------------------------------------------------------------------------
// Mock LLM provider — returns deterministic responses without network calls.
// In production, replace this with a real provider that implements LLMProvider.
// ---------------------------------------------------------------------------
struct MockLLM {
    response: String,
}

impl MockLLM {
    fn new(response: impl Into<String>) -> Self {
        Self {
            response: response.into(),
        }
    }
}

#[async_trait]
impl LLMProvider for MockLLM {
    fn name(&self) -> &str {
        "mock"
    }

    async fn chat(&self, _request: ChatCompletionRequest) -> AgentResult<ChatCompletionResponse> {
        Ok(ChatCompletionResponse {
            content: Some(self.response.clone()),
            tool_calls: None,
            usage: Some(TokenUsage {
                prompt_tokens: 10,
                completion_tokens: 20,
                total_tokens: 30,
            }),
        })
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().with_env_filter("info").init();

    // ------------------------------------------------------------------
    // 1. Build the analyst agent from a TOML profile
    // ------------------------------------------------------------------
    let analyst_profile = AgentProfile::from_toml(ANALYST_TOML)?;
    info!(
        "Loaded analyst profile: name={:?}, model={:?}, max_iterations={:?}",
        analyst_profile.name, analyst_profile.model, analyst_profile.max_iterations
    );

    let analyst_llm: Arc<dyn LLMProvider> =
        Arc::new(MockLLM::new("NVDA market cap is approximately $3.2T as of Q4 2025."));

    let analyst = analyst_profile
        .to_builder()
        .llm(analyst_llm)
        .build()
        .await?;

    info!("Analyst agent built (id={})", analyst.base().id());

    // ------------------------------------------------------------------
    // 2. Build the researcher agent from a YAML profile
    // ------------------------------------------------------------------
    let researcher_profile = AgentProfile::from_yaml(RESEARCHER_YAML)?;
    info!(
        "Loaded researcher profile: name={:?}, model={:?}",
        researcher_profile.name, researcher_profile.model
    );

    let researcher_llm: Arc<dyn LLMProvider> =
        Arc::new(MockLLM::new("Here is a synthesised research summary..."));

    let researcher = researcher_profile
        .to_builder()
        .llm(researcher_llm)
        .build()
        .await?;

    info!("Researcher agent built (id={})", researcher.base().id());

    // ------------------------------------------------------------------
    // 3. Build a third agent entirely in code using the fluent API
    // ------------------------------------------------------------------
    let coder_llm: Arc<dyn LLMProvider> =
        Arc::new(MockLLM::new("fn add(a: i32, b: i32) -> i32 { a + b }"));

    let coder = AgentBuilder::new()
        .name("coder")
        .description("Expert Rust programmer")
        .system_prompt("You are an expert Rust programmer. Write idiomatic, safe Rust.")
        .llm(coder_llm)
        .model("gpt-4o")
        .max_iterations(8)
        .temperature(0.1)
        .build()
        .await?;

    info!("Coder agent built (id={})", coder.base().id());

    // ------------------------------------------------------------------
    // 4. Register all agents in an AgentRegistry
    // ------------------------------------------------------------------
    let mut registry = AgentRegistry::new();
    registry.register("analyst", analyst);
    registry.register("researcher", researcher);
    registry.register("coder", coder);

    let mut names = registry.list();
    names.sort();
    info!("Registry contains {} agents: {:?}", registry.len(), names);

    // ------------------------------------------------------------------
    // 5. Route a task to a specific agent by name
    // ------------------------------------------------------------------
    if let Some(agent) = registry.get_mut("analyst") {
        let response = agent
            .process_message("session-1", "What is NVIDIA's market cap?")
            .await?;
        info!("Analyst response: {}", response);
    }

    if let Some(agent) = registry.get_mut("coder") {
        let response = agent
            .process_message("session-2", "Write an add function in Rust.")
            .await?;
        info!("Coder response: {}", response);
    }

    info!("Example complete.");
    Ok(())
}
