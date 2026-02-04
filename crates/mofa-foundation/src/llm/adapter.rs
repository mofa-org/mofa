//! LLM Provider Adapters
//!
//! This module provides adapter types to bridge different LLM interfaces
//! with mofa's standard LLMProvider trait.

use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;

use crate::agent::{
    ChatCompletionRequest, ChatCompletionResponse, ChatMessage, LLMProvider,
    ToolCall, TokenUsage,
};

use mofa_kernel::agent::error::{AgentError, AgentResult};

// ============================================================================
// Legacy LLM Provider Interface
// ============================================================================

/// Legacy LLM provider trait for simple chat interfaces
///
/// This trait provides a simpler interface for LLM providers that don't want
/// to deal with mofa's full ChatCompletionRequest type. LLMProviderAdapter
/// automatically implements the full LLMProvider trait.
///
/// # Example
///
/// ```rust,ignore
/// use mofa_foundation::llm::adapter::{LegacyLLMProvider, LLMResponse, LegacyToolCall};
/// use serde_json::json;
///
/// struct MySimpleLLM {
///     api_key: String,
/// }
///
/// #[async_trait::async_trait]
/// impl LegacyLLMProvider for MySimpleLLM {
///     async fn chat(
///         &self,
///         messages: &[Value],
///         tools: Option<&[Value]>,
///         model: Option<&str>,
///     ) -> Result<LLMResponse, String> {
///         // Simple implementation
///         Ok(LLMResponse {
///             content: Some("Hello!".to_string()),
///             tool_calls: vec![],
///             usage: HashMap::new(),
///             finish_reason: "stop".to_string(),
///         })
///     }
///
///     fn get_default_model(&self) -> String {
///         "my-model".to_string()
///     }
/// }
/// ```
#[async_trait]
pub trait LegacyLLMProvider: Send + Sync {
    /// Send a chat request
    ///
    /// # Parameters
    /// - `messages`: Array of message objects in JSON format
    /// - `tools`: Optional array of tool definitions
    /// - `model`: Optional model name override
    ///
    /// # Returns
    /// A response with content, tool calls, and usage information
    async fn chat(
        &self,
        messages: &[Value],
        tools: Option<&[Value]>,
        model: Option<&str>,
    ) -> Result<LLMResponse, String>;

    /// Get the default model name for this provider
    fn get_default_model(&self) -> String;
}

/// Response from a legacy LLM provider
#[derive(Debug, Clone)]
pub struct LLMResponse {
    /// Text content of the response
    pub content: Option<String>,
    /// Tool calls requested by the LLM
    pub tool_calls: Vec<LegacyToolCall>,
    /// Token usage information
    pub usage: HashMap<String, u32>,
    /// Reason for finishing (stop, length, tool_calls, etc.)
    pub finish_reason: String,
}

/// A tool call from a legacy LLM provider
#[derive(Debug, Clone)]
pub struct LegacyToolCall {
    /// Unique identifier for this tool call
    pub id: String,
    /// Name of the tool/function to call
    pub name: String,
    /// Arguments for the tool call as key-value pairs
    pub arguments: HashMap<String, Value>,
}

// ============================================================================
// LLM Provider Adapter
// ============================================================================

/// Adapter that converts a LegacyLLMProvider to mofa's LLMProvider trait
///
/// This adapter handles the conversion between mofa's ChatCompletionRequest
/// and the simpler LegacyLLMProvider interface.
pub struct LLMProviderAdapter<P: LegacyLLMProvider> {
    inner: P,
    cached_name: &'static str,
}

impl<P: LegacyLLMProvider> LLMProviderAdapter<P> {
    /// Create a new adapter from a legacy provider
    ///
    /// # Parameters
    /// - `inner`: The legacy LLM provider to wrap
    /// - `name`: A name for this provider (will be leaked as static str)
    pub fn new(inner: P, name: String) -> Self {
        Self {
            inner,
            cached_name: Box::leak(name.into_boxed_str()),
        }
    }

    /// Get a reference to the inner provider
    pub fn inner(&self) -> &P {
        &self.inner
    }
}

#[async_trait]
impl<P: LegacyLLMProvider + Send + Sync> LLMProvider for LLMProviderAdapter<P> {
    fn name(&self) -> &str {
        self.cached_name
    }

    async fn chat(&self, request: ChatCompletionRequest) -> AgentResult<ChatCompletionResponse> {
        // Convert ChatMessage to Value format
        let messages: Vec<Value> = request
            .messages
            .iter()
            .map(|m| {
                let mut msg = serde_json::json!({
                    "role": m.role,
                });
                if let Some(content) = &m.content {
                    msg["content"] = serde_json::json!(content);
                }
                msg
            })
            .collect();

        // Convert tool definitions to Value format
        let tools: Option<Vec<Value>> = request.tools.map(|defs| {
            defs.iter()
                .map(|t| {
                    serde_json::json!({
                        "type": "function",
                        "function": {
                            "name": t.name,
                            "description": t.description,
                            "parameters": t.parameters
                        }
                    })
                })
                .collect()
        });

        // Get model name (use request model or default)
        let default_model = self.inner.get_default_model();
        let model = request.model.as_deref().unwrap_or(&default_model);

        // Call the inner provider
        let response = self
            .inner
            .chat(&messages, tools.as_deref(), Some(model))
            .await
            .map_err(|e| AgentError::ExecutionFailed(format!("LLM call failed: {}", e)))?;

        // Convert tool calls to mofa format
        let tool_calls = if response.tool_calls.is_empty() {
            None
        } else {
            Some(
                response
                    .tool_calls
                    .into_iter()
                    .map(|tc| ToolCall {
                        id: tc.id,
                        name: tc.name,
                        arguments: serde_json::to_value(tc.arguments).unwrap_or_default(),
                    })
                    .collect(),
            )
        };

        // Convert usage information
        let usage = if response.usage.is_empty() {
            None
        } else {
            Some(TokenUsage {
                prompt_tokens: response.usage.get("prompt_tokens").copied().unwrap_or(0),
                completion_tokens: response.usage.get("completion_tokens").copied().unwrap_or(0),
                total_tokens: response.usage.get("total_tokens").copied().unwrap_or(0),
            })
        };

        Ok(ChatCompletionResponse {
            content: response.content,
            tool_calls,
            usage,
        })
    }
}

/// Convenience function to convert a LegacyLLMProvider to Arc<dyn LLMProvider>
///
/// # Example
///
/// ```rust,ignore
/// use mofa_foundation::llm::adapter::{LegacyLLMProvider, as_llm_provider};
/// use std::sync::Arc;
///
/// let my_provider = MySimpleLLM::new("api-key");
/// let llm_provider: Arc<dyn LLMProvider> = as_llm_provider(my_provider, "my-provider".to_string());
/// ```
pub fn as_llm_provider<P: LegacyLLMProvider + Send + Sync + 'static>(
    provider: P,
    name: String,
) -> std::sync::Arc<dyn LLMProvider> {
    std::sync::Arc::new(LLMProviderAdapter::new(provider, name))
}
