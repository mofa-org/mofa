//! UniFFI bindings implementation
//!
//! This module provides clean implementations for the types defined in mofa.udl

use std::sync::{Arc, Mutex as StdMutex};
use tokio::runtime::Runtime;
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
    Anthropic,
    Gemini,
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

/// Check if Dora runtime support is available
pub fn is_dora_available() -> bool {
    cfg!(feature = "dora")
}

/// Create a new LLM Agent Builder
pub fn new_llm_agent_builder() -> Result<std::sync::Arc<LLMAgentBuilder>, MoFaError> {
    Ok(LLMAgentBuilder::create())
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
    runtime: Arc<Runtime>,
    _runtime: std::marker::PhantomData<()>,
}

impl LLMAgent {
    /// Create from configuration file (agent.yml)
    pub fn from_config_file(config_path: String) -> Result<Self, MoFaError> {
        let runtime =
            tokio::runtime::Runtime::new().map_err(|e| MoFaError::RuntimeError(e.to_string()))?;

        let agent = mofa_foundation::llm::agent_from_config(&config_path)
            .map_err(|e| MoFaError::ConfigError(e.to_string()))?;

        let agent_id = agent.config().agent_id.clone();
        let name = agent.config().name.clone();

        Ok(Self {
            agent_id,
            name,
            inner: Arc::new(RwLock::new(agent)),
            _inner: std::marker::PhantomData,
            runtime: Arc::new(runtime),
            _runtime: std::marker::PhantomData,
        })
    }

    /// Create from configuration dictionary
    pub fn from_config(
        config: LLMConfig,
        agent_id: String,
        name: String,
    ) -> Result<Self, MoFaError> {
        use mofa_foundation::llm::{
            AnthropicConfig, AnthropicProvider, GeminiConfig, GeminiProvider, LLMAgentBuilder,
            OpenAIConfig, OpenAIProvider,
        };

        let runtime =
            tokio::runtime::Runtime::new().map_err(|e| MoFaError::RuntimeError(e.to_string()))?;

        let mut builder = LLMAgentBuilder::new().with_id(&agent_id).with_name(&name);

        // Create provider based on config
        let provider: Arc<dyn mofa_foundation::llm::LLMProvider> = match config.provider {
            LLMProviderType::OpenAI => {
                let api_key = config
                    .api_key
                    .clone()
                    .or_else(|| std::env::var("OPENAI_API_KEY").ok())
                    .ok_or_else(|| MoFaError::ConfigError("OpenAI API key not set".to_string()))?;

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
                let endpoint = config
                    .base_url
                    .clone()
                    .ok_or_else(|| MoFaError::ConfigError("Azure endpoint not set".to_string()))?;
                let api_key = config
                    .api_key
                    .clone()
                    .ok_or_else(|| MoFaError::ConfigError("Azure API key not set".to_string()))?;
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
            LLMProviderType::Anthropic => {
                let api_key = config
                    .api_key
                    .clone()
                    .or_else(|| std::env::var("ANTHROPIC_API_KEY").ok())
                    .ok_or_else(|| {
                        MoFaError::ConfigError("Anthropic API key not set".to_string())
                    })?;

                let mut a_cfg = AnthropicConfig::new(api_key);
                if let Some(model) = config.model.clone() {
                    a_cfg = a_cfg.with_model(model);
                }
                if let Some(base_url) = config.base_url.clone() {
                    a_cfg = a_cfg.with_base_url(base_url);
                }
                if let Some(temp) = config.temperature {
                    a_cfg = a_cfg.with_temperature(temp);
                }
                if let Some(tokens) = config.max_tokens {
                    a_cfg = a_cfg.with_max_tokens(tokens);
                }

                Arc::new(AnthropicProvider::with_config(a_cfg))
            }
            LLMProviderType::Gemini => {
                let api_key = config
                    .api_key
                    .clone()
                    .or_else(|| std::env::var("GEMINI_API_KEY").ok())
                    .ok_or_else(|| MoFaError::ConfigError("Gemini API key not set".to_string()))?;

                let mut g_cfg = GeminiConfig::new(api_key);
                if let Some(model) = config.model.clone() {
                    g_cfg = g_cfg.with_model(model);
                }
                if let Some(base_url) = config.base_url.clone() {
                    g_cfg = g_cfg.with_base_url(base_url);
                }
                if let Some(temp) = config.temperature {
                    g_cfg = g_cfg.with_temperature(temp);
                }
                if let Some(tokens) = config.max_tokens {
                    g_cfg = g_cfg.with_max_tokens(tokens);
                }

                Arc::new(GeminiProvider::with_config(g_cfg))
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
            _inner: std::marker::PhantomData,
            runtime: Arc::new(runtime),
            _runtime: std::marker::PhantomData,
        })
    }

    /// Get agent ID
    pub fn agent_id(&self) -> Result<String, MoFaError> {
        Ok(self.agent_id.clone())
    }

    /// Get agent name
    pub fn name(&self) -> Result<String, MoFaError> {
        Ok(self.name.clone())
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
    }
}

// =============================================================================
// LLM Agent Builder Implementation
// ============================================================================

/// Builder state for storing configuration
#[derive(Debug, Clone)]
struct BuilderState {
    agent_id: Option<String>,
    name: Option<String>,
    system_prompt: Option<String>,
    temperature: Option<f32>,
    max_tokens: Option<u32>,
    session_id: Option<String>,
    user_id: Option<String>,
    tenant_id: Option<String>,
    context_window_size: Option<usize>,
    openai_api_key: Option<String>,
    openai_base_url: Option<String>,
    openai_model: Option<String>,
}

impl Default for BuilderState {
    fn default() -> Self {
        Self {
            agent_id: None,
            name: None,
            system_prompt: None,
            temperature: None,
            max_tokens: None,
            session_id: None,
            user_id: None,
            tenant_id: None,
            context_window_size: None,
            openai_api_key: None,
            openai_base_url: None,
            openai_model: None,
        }
    }
}

/// LLM Agent Builder - fluent builder for creating LLMAgent instances
///
/// This is the primary interface for building LLMAgent instances from Python,
/// Kotlin, Swift, and Java.
pub struct LLMAgentBuilder {
    state: Arc<StdMutex<BuilderState>>,
    runtime: Arc<Runtime>,
}

impl LLMAgentBuilder {
    /// Create a new builder
    pub fn create() -> Arc<Self> {
        let runtime = tokio::runtime::Runtime::new()
            .map_err(|e| MoFaError::RuntimeError(e.to_string()))
            .unwrap();

        Arc::new(Self {
            state: Arc::new(StdMutex::new(BuilderState::default())),
            runtime: Arc::new(runtime),
        })
    }

    /// Set agent ID
    pub fn set_id(self: Arc<Self>, id: String) -> Arc<Self> {
        let mut state = self.state.lock().unwrap();
        state.agent_id = Some(id);
        drop(state);
        self
    }

    /// Set agent name
    pub fn set_name(self: Arc<Self>, name: String) -> Arc<Self> {
        let mut state = self.state.lock().unwrap();
        state.name = Some(name);
        drop(state);
        self
    }

    /// Set system prompt
    pub fn set_system_prompt(self: Arc<Self>, prompt: String) -> Arc<Self> {
        let mut state = self.state.lock().unwrap();
        state.system_prompt = Some(prompt);
        drop(state);
        self
    }

    /// Set temperature
    pub fn set_temperature(self: Arc<Self>, temperature: f32) -> Arc<Self> {
        let mut state = self.state.lock().unwrap();
        state.temperature = Some(temperature);
        drop(state);
        self
    }

    /// Set max tokens
    pub fn set_max_tokens(self: Arc<Self>, max_tokens: u32) -> Arc<Self> {
        let mut state = self.state.lock().unwrap();
        state.max_tokens = Some(max_tokens);
        drop(state);
        self
    }

    /// Set initial session ID
    pub fn set_session_id(self: Arc<Self>, session_id: String) -> Arc<Self> {
        let mut state = self.state.lock().unwrap();
        state.session_id = Some(session_id);
        drop(state);
        self
    }

    /// Set user ID
    pub fn set_user_id(self: Arc<Self>, user_id: String) -> Arc<Self> {
        let mut state = self.state.lock().unwrap();
        state.user_id = Some(user_id);
        drop(state);
        self
    }

    /// Set tenant ID
    pub fn set_tenant_id(self: Arc<Self>, tenant_id: String) -> Arc<Self> {
        let mut state = self.state.lock().unwrap();
        state.tenant_id = Some(tenant_id);
        drop(state);
        self
    }

    /// Set context window size (in rounds)
    pub fn set_context_window_size(self: Arc<Self>, size: u32) -> Arc<Self> {
        let mut state = self.state.lock().unwrap();
        state.context_window_size = Some(size as usize);
        drop(state);
        self
    }

    /// Set OpenAI provider
    pub fn set_openai_provider(
        self: Arc<Self>,
        api_key: String,
        base_url: Option<String>,
        model: Option<String>,
    ) -> Result<Arc<Self>, MoFaError> {
        let mut state = self.state.lock().unwrap();
        state.openai_api_key = Some(api_key);
        state.openai_base_url = base_url;
        state.openai_model = model;
        drop(state);
        Ok(self)
    }

    /// Build the LLMAgent synchronously
    pub fn build(self: Arc<Self>) -> Result<Arc<LLMAgent>, MoFaError> {
        use mofa_foundation::llm::{LLMAgentBuilder, OpenAIConfig, OpenAIProvider};
        use std::sync::Arc as StdArc;

        let state = self.state.lock().unwrap();

        // Get or generate agent_id
        let agent_id = state
            .agent_id
            .clone()
            .unwrap_or_else(|| uuid::Uuid::now_v7().to_string());

        let mut builder = LLMAgentBuilder::new().with_id(&agent_id);

        // Set name if provided
        if let Some(ref name) = state.name {
            builder = builder.with_name(name);
        }

        // Set system prompt if provided
        if let Some(ref prompt) = state.system_prompt {
            builder = builder.with_system_prompt(prompt);
        }

        // Set temperature if provided
        if let Some(temp) = state.temperature {
            builder = builder.with_temperature(temp);
        }

        // Set max tokens if provided
        if let Some(tokens) = state.max_tokens {
            builder = builder.with_max_tokens(tokens);
        }

        // Set session ID if provided
        if let Some(ref session_id) = state.session_id {
            builder = builder.with_session_id(session_id);
        }

        // Set user ID if provided
        if let Some(ref user_id) = state.user_id {
            builder = builder.with_user(user_id);
        }

        // Set tenant ID if provided
        if let Some(ref tenant_id) = state.tenant_id {
            builder = builder.with_tenant(tenant_id);
        }

        // Set context window size if provided
        if let Some(size) = state.context_window_size {
            builder = builder.with_sliding_window(size);
        }

        // Set OpenAI provider if API key is provided
        if let Some(ref api_key) = state.openai_api_key {
            let mut config = OpenAIConfig::new(api_key.clone());
            if let Some(ref base_url) = state.openai_base_url {
                config = config.with_base_url(base_url);
            }
            if let Some(ref model) = state.openai_model {
                config = config.with_model(model);
            }
            let provider = StdArc::new(OpenAIProvider::with_config(config));
            builder = builder.with_provider(provider);
        }

        drop(state);

        let inner_agent = builder
            .try_build()
            .map_err(|e| MoFaError::ConfigError(e.to_string()))?;

        let agent_id = inner_agent.config().agent_id.clone();
        let name = inner_agent.config().name.clone();

        Ok(Arc::new(LLMAgent {
            agent_id,
            name,
            inner: Arc::new(RwLock::new(inner_agent)),
            _inner: std::marker::PhantomData,
            runtime: self.runtime.clone(),
            _runtime: std::marker::PhantomData,
        }))
    }
}
