//! UniFFI bindings implementation
//!
//! This module provides clean implementations for the types defined in mofa.udl

use std::sync::Arc;
use tokio::sync::RwLock;

// =============================================================================
// Error Types
// =============================================================================

/// MoFA error type for UniFFI
#[derive(Debug, thiserror::Error)]
pub enum MoFaError {
    #[error("Configuration error: {0}")]
    ConfigError(String),
    #[error("Runtime error: {0}")]
    RuntimeError(String),
    #[error("LLM error: {0}")]
    LLMError(String),
    #[error("IO error: {0}")]
    IoError(String),
    #[error("Invalid argument: {0}")]
    InvalidArgument(String),
}

// =============================================================================
// LLM Types (matching UDL definitions)
// =============================================================================

/// LLM provider type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LLMProviderType {
    OpenAI,
    Ollama,
    Azure,
    Compatible,
}

/// Chat message role
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatRole {
    System,
    User,
    Assistant,
}

/// LLM configuration
#[derive(Debug, Clone)]
pub struct LLMConfig {
    pub provider: LLMProviderType,
    pub model: Option<String>,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub deployment: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub system_prompt: Option<String>,
}

/// Chat message
#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
}

// =============================================================================
// Namespace functions
// =============================================================================

/// Get MoFA version
pub fn get_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

// =============================================================================
// LLM Agent Implementation
// =============================================================================

/// LLM Agent - the main interface for LLM interactions
///
/// This is the primary interface exposed to Python, Kotlin, Swift, and Java.
pub struct LLMAgent {
    agent_id: String,
    name: String,
    inner: Arc<RwLock<mofa_foundation::llm::LLMAgent>>,
    _inner: std::marker::PhantomData<()>,
    runtime: tokio::runtime::Runtime,
    _runtime: std::marker::PhantomData<()>,
}

impl LLMAgent {
    /// Create from configuration file (agent.yml)
    pub fn from_config_file(config_path: String) -> Result<Self, MoFaError> {
        {
            let runtime = tokio::runtime::Runtime::new()
                .map_err(|e| MoFaError::RuntimeError(e.to_string()))?;

            let agent = mofa_foundation::llm::agent_from_config(&config_path)
                .map_err(|e| MoFaError::ConfigError(e.to_string()))?;

            let agent_id = agent.config().agent_id.clone();
            let name = agent.config().name.clone();

            Ok(Self {
                agent_id,
                name,
                inner: Arc::new(RwLock::new(agent)),
                runtime,
            })
        }
        {
            let _ = config_path;
            Err(MoFaError::ConfigError(
                "OpenAI feature not enabled. Rebuild with --features openai".to_string(),
            ))
        }
    }

    /// Create from configuration dictionary
    pub fn from_config(
        config: LLMConfig,
        agent_id: String,
        name: String,
    ) -> Result<Self, MoFaError> {

        {
            use mofa_foundation::llm::{LLMAgentBuilder, OpenAIConfig, OpenAIProvider};

            let runtime = tokio::runtime::Runtime::new()
                .map_err(|e| MoFaError::RuntimeError(e.to_string()))?;

            let mut builder = LLMAgentBuilder::new(&agent_id).with_name(&name);

            // Create provider based on config
            let provider: Arc<dyn mofa_foundation::llm::LLMProvider> = match config.provider {
                LLMProviderType::OpenAI => {
                    let api_key = config
                        .api_key
                        .clone()
                        .or_else(|| std::env::var("OPENAI_API_KEY").ok())
                        .ok_or_else(|| {
                            MoFaError::ConfigError("OpenAI API key not set".to_string())
                        })?;

                    let mut oai_config = OpenAIConfig::new(api_key);
                    if let Some(model) = config.model.clone() {
                        oai_config = oai_config.with_model(model);
                    }
                    if let Some(base_url) = config.base_url.clone() {
                        oai_config = oai_config.with_base_url(base_url);
                    }
                    if let Some(temp) = config.temperature {
                        oai_config = oai_config.with_temperature(temp);
                    }
                    if let Some(tokens) = config.max_tokens {
                        oai_config = oai_config.with_max_tokens(tokens);
                    }
                    Arc::new(OpenAIProvider::with_config(oai_config))
                }
                LLMProviderType::Ollama => {
                    let model = config.model.clone().unwrap_or_else(|| "llama2".to_string());
                    Arc::new(OpenAIProvider::ollama(model))
                }
                LLMProviderType::Azure => {
                    let endpoint = config.base_url.clone().ok_or_else(|| {
                        MoFaError::ConfigError("Azure endpoint not set".to_string())
                    })?;
                    let api_key = config.api_key.clone().ok_or_else(|| {
                        MoFaError::ConfigError("Azure API key not set".to_string())
                    })?;
                    let deployment = config
                        .deployment
                        .clone()
                        .or(config.model.clone())
                        .ok_or_else(|| {
                            MoFaError::ConfigError("Azure deployment not set".to_string())
                        })?;
                    Arc::new(OpenAIProvider::azure(endpoint, api_key, deployment))
                }
                LLMProviderType::Compatible => {
                    let base_url = config
                        .base_url
                        .clone()
                        .ok_or_else(|| MoFaError::ConfigError("base_url not set".to_string()))?;
                    let model = config
                        .model
                        .clone()
                        .unwrap_or_else(|| "default".to_string());
                    Arc::new(OpenAIProvider::local(base_url, model))
                }
            };

            builder = builder.with_provider(provider);

            if let Some(temp) = config.temperature {
                builder = builder.with_temperature(temp);
            }
            if let Some(tokens) = config.max_tokens {
                builder = builder.with_max_tokens(tokens);
            }
            if let Some(prompt) = config.system_prompt {
                builder = builder.with_system_prompt(prompt);
            }

            let inner_agent = builder
                .try_build()
                .map_err(|e| MoFaError::ConfigError(e.to_string()))?;

            Ok(Self {
                agent_id,
                name,
                inner: Arc::new(RwLock::new(inner_agent)),
                runtime,
            })
        }
        #[cfg(not(feature = "openai"))]
        {
            let _ = (config, agent_id, name);
            Err(MoFaError::ConfigError(
                "OpenAI feature not enabled. Rebuild with --features openai".to_string(),
            ))
        }
    }

    /// Get agent ID
    pub fn agent_id(&self) -> String {
        self.agent_id.clone()
    }

    /// Get agent name
    pub fn name(&self) -> String {
        self.name.clone()
    }

    /// Simple Q&A (no context retention)
    pub fn ask(&self, question: String) -> Result<String, MoFaError> {

        {
            self.runtime.block_on(async {
                let agent = self.inner.read().await;
                agent
                    .ask(&question)
                    .await
                    .map_err(|e| MoFaError::LLMError(e.to_string()))
            })
        }
        #[cfg(not(feature = "openai"))]
        {
            let _ = question;
            Err(MoFaError::ConfigError(
                "OpenAI feature not enabled".to_string(),
            ))
        }
    }

    /// Multi-turn chat (with context retention)
    pub fn chat(&self, message: String) -> Result<String, MoFaError> {

        {
            self.runtime.block_on(async {
                let agent = self.inner.read().await;
                agent
                    .chat(&message)
                    .await
                    .map_err(|e| MoFaError::LLMError(e.to_string()))
            })
        }
        #[cfg(not(feature = "openai"))]
        {
            let _ = message;
            Err(MoFaError::ConfigError(
                "OpenAI feature not enabled".to_string(),
            ))
        }
    }

    /// Clear conversation history
    pub fn clear_history(&self) {

        {
            self.runtime.block_on(async {
                let agent = self.inner.read().await;
                agent.clear_history().await;
            });
        }
    }

    /// Get conversation history
    pub fn get_history(&self) -> Vec<ChatMessage> {

        {
            self.runtime.block_on(async {
                let agent = self.inner.read().await;
                agent
                    .history()
                    .await
                    .iter()
                    .map(|msg| {
                        let role = match msg.role {
                            mofa_foundation::llm::Role::System => ChatRole::System,
                            mofa_foundation::llm::Role::User => ChatRole::User,
                            mofa_foundation::llm::Role::Assistant => ChatRole::Assistant,
                            _ => ChatRole::User,
                        };
                        let content = msg
                            .content
                            .as_ref()
                            .map(|c| match c {
                                mofa_foundation::llm::MessageContent::Text(s) => s.clone(),
                                _ => String::new(),
                            })
                            .unwrap_or_default();
                        ChatMessage { role, content }
                    })
                    .collect()
            })
        }
        #[cfg(not(feature = "openai"))]
        {
            Vec::new()
        }
    }
}
