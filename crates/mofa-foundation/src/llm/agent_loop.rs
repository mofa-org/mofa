//! Agent loop framework for tool execution and session management
//!
//! This module provides a reusable agent loop that handles:
//! - Message processing with tool execution
//! - Configurable iteration limits
//! - Session management integration
//! - LLM provider abstraction
//! - Media/vision support

use crate::llm::types::{
    ChatMessage, ChatCompletionRequest, Tool, ToolCall, Role, MessageContent,
    ContentPart, ImageUrl,
};
use crate::llm::LLMProvider;
use anyhow::Result;
use serde_json::json;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Configuration for the agent loop
#[derive(Debug, Clone)]
pub struct AgentLoopConfig {
    /// Maximum number of tool iterations
    pub max_tool_iterations: usize,
    /// Default model to use
    pub default_model: String,
    /// Temperature for generation
    pub temperature: Option<f32>,
    /// Maximum tokens to generate
    pub max_tokens: Option<u32>,
}

impl Default for AgentLoopConfig {
    fn default() -> Self {
        Self {
            max_tool_iterations: 10,
            default_model: "gpt-4o-mini".to_string(),
            temperature: Some(0.7),
            max_tokens: None,
        }
    }
}

/// Tool executor trait for executing tool calls (AgentLoop specific)
#[async_trait::async_trait]
pub trait ToolExecutor: Send + Sync {
    /// Execute a tool call
    async fn execute(&self, name: &str, arguments: &str) -> Result<String>;

    /// Get available tool definitions
    fn available_tools(&self) -> Vec<Tool>;
}

/// Agent loop that processes messages with tool support
pub struct AgentLoop {
    /// LLM provider
    provider: Arc<dyn LLMProvider>,
    /// Tool executor
    tools: Arc<dyn ToolExecutor>,
    /// Configuration
    config: AgentLoopConfig,
}

impl AgentLoop {
    /// Create a new agent loop
    pub fn new(
        provider: Arc<dyn LLMProvider>,
        tools: Arc<dyn ToolExecutor>,
        config: AgentLoopConfig,
    ) -> Self {
        Self {
            provider,
            tools,
            config,
        }
    }

    /// Create with default configuration
    pub fn with_defaults(
        provider: Arc<dyn LLMProvider>,
        tools: Arc<dyn ToolExecutor>,
    ) -> Self {
        Self::new(provider, tools, AgentLoopConfig::default())
    }

    /// Process a single message with tool execution loop
    pub async fn process_message(
        &self,
        context: Vec<ChatMessage>,
        content: &str,
        media: Option<Vec<String>>,
    ) -> Result<String> {
        self.process_with_options(context, content, media, None).await
    }

    /// Process with custom model
    pub async fn process_with_model(
        &self,
        context: Vec<ChatMessage>,
        content: &str,
        media: Option<Vec<String>>,
        model: &str,
    ) -> Result<String> {
        self.process_with_options(context, content, media, Some(model)).await
    }

    /// Process with custom context builder and options
    pub async fn process_with_options(
        &self,
        mut context: Vec<ChatMessage>,
        content: &str,
        media: Option<Vec<String>>,
        model: Option<&str>,
    ) -> Result<String> {
        // Build user message with optional media
        let user_msg = if let Some(media_paths) = media {
            if !media_paths.is_empty() {
                Self::build_vision_message(content, &media_paths)?
            } else {
                ChatMessage::user(content)
            }
        } else {
            ChatMessage::user(content)
        };

        context.push(user_msg);

        // Get tool definitions
        let tools = self.tools.available_tools();

        // Run the agent loop
        self.run_agent_loop(context, &tools, model).await
    }

    /// Run the main agent loop with tool execution
    async fn run_agent_loop(
        &self,
        mut messages: Vec<ChatMessage>,
        tools: &[Tool],
        model: Option<&str>,
    ) -> Result<String> {
        let model = model.unwrap_or(&self.config.default_model);

        for iteration in 0..self.config.max_tool_iterations {
            // Build request
            let mut request = ChatCompletionRequest::new(model);
            request.messages = messages.clone();
            request.temperature = self.config.temperature;
            request.max_tokens = self.config.max_tokens;

            if !tools.is_empty() {
                request.tools = Some(tools.to_vec());
            }

            // Call LLM
            let response = self.provider.chat(request).await?;

            // Check for tool calls
            if let Some(tool_calls) = response.tool_calls() {
                if !tool_calls.is_empty() {
                    // Add assistant message with tool calls
                    messages.push(ChatMessage::assistant_with_tool_calls(tool_calls.clone()));

                    // Execute tools
                    for tool_call in tool_calls {
                        tracing::debug!(
                            "Executing tool: {} with args: {:?}",
                            tool_call.function.name,
                            tool_call.function.arguments
                        );

                        let result = self
                            .execute_tool(&tool_call.function.name, &tool_call.function.arguments)
                            .await;

                        messages.push(ChatMessage::tool_result(
                            &tool_call.id,
                            result.unwrap_or_else(|e| format!("Error: {}", e))),
                        );
                    }

                    continue;
                }
            }

            // No tool calls, return the content
            if let Some(content) = response.content() {
                return Ok(content.to_string());
            } else {
                return Ok("No response generated.".to_string());
            }
        }

        // Max iterations exceeded
        tracing::warn!(
            "Agent loop exceeded max iterations ({})",
            self.config.max_tool_iterations
        );
        Ok("I've completed processing but hit the maximum iteration limit.".to_string())
    }

    /// Execute a tool call
    async fn execute_tool(&self, name: &str, arguments: &str) -> Result<String> {
        self.tools.execute(name, arguments).await
    }

    /// Build a vision message with images
    fn build_vision_message(text: &str, image_paths: &[String]) -> Result<ChatMessage> {
        let mut parts = vec![ContentPart::Text {
            text: text.to_string(),
        }];

        for path in image_paths {
            let image_url = Self::encode_image_data_url(Path::new(path))?;
            parts.push(ContentPart::Image { image_url });
        }

        Ok(ChatMessage {
            role: Role::User,
            content: Some(MessageContent::Parts(parts)),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        })
    }

    /// Encode an image file as a data URL
    fn encode_image_data_url(path: &Path) -> Result<ImageUrl> {
        use base64::engine::general_purpose::STANDARD_NO_PAD;
        use base64::Engine;
        use std::fs;

        let bytes = fs::read(path)?;
        let mime_type = infer::get_from_path(path)?
            .ok_or_else(|| anyhow::anyhow!("Unknown MIME type for: {:?}", path))?
            .mime_type()
            .to_string();

        let base64 = STANDARD_NO_PAD.encode(&bytes);
        let url = format!("data:{};base64,{}", mime_type, base64);

        Ok(ImageUrl {
            url,
            detail: None,
        })
    }

    /// Get the configuration
    pub fn config(&self) -> &AgentLoopConfig {
        &self.config
    }
}

/// Simple tool executor for testing
pub struct SimpleToolExecutor {
    tools: HashMap<String, Box<dyn Fn(&str) -> Result<String> + Send + Sync>>,
}

impl SimpleToolExecutor {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub fn register<F>(&mut self, name: impl Into<String>, handler: F) -> &mut Self
    where
        F: Fn(&str) -> Result<String> + Send + Sync + 'static,
    {
        self.tools.insert(name.into(), Box::new(handler));
        self
    }
}

impl Default for SimpleToolExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ToolExecutor for SimpleToolExecutor {
    async fn execute(&self, name: &str, arguments: &str) -> Result<String> {
        if let Some(handler) = self.tools.get(name) {
            handler(arguments)
        } else {
            Err(anyhow::anyhow!("Unknown tool: {}", name))
        }
    }

    fn available_tools(&self) -> Vec<Tool> {
        // Return empty since this is just for testing
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_loop_config_default() {
        let config = AgentLoopConfig::default();
        assert_eq!(config.max_tool_iterations, 10);
        assert_eq!(config.default_model, "gpt-4o-mini");
    }

    #[test]
    fn test_agent_loop_config_custom() {
        let config = AgentLoopConfig {
            max_tool_iterations: 5,
            default_model: "gpt-4".to_string(),
            temperature: Some(0.5),
            max_tokens: Some(1000),
        };
        assert_eq!(config.max_tool_iterations, 5);
        assert_eq!(config.default_model, "gpt-4");
    }
}
