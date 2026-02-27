//! LLM Plugin Adapter
//! LLM Plugin Adapter
//!
//! 将 LLM Provider 封装为 MoFA 插件
//! Encapsulate LLM Provider as a MoFA plugin

use super::provider::{LLMConfig, LLMProvider};
use super::types::*;
use mofa_kernel::plugin::{
    AgentPlugin, PluginContext, PluginMetadata, PluginPriority, PluginResult, PluginState,
    PluginType,
};
use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// LLM 插件
/// LLM Plugin
///
/// 将 LLM Provider 封装为 MoFA 框架的插件
/// Encapsulates the LLM Provider as a plugin for the MoFA framework
///
/// # 示例
/// # Example
///
/// ```rust,ignore
/// use mofa_foundation::llm::{LLMPlugin, LLMConfig};
///
/// // 创建 LLM 插件
/// // Create LLM plugin
/// let plugin = LLMPlugin::new("my-llm", provider);
///
/// // 作为 AgentPlugin 使用
/// // Use as an AgentPlugin
/// agent.add_plugin(Box::new(plugin));
/// ```
pub struct LLMPlugin {
    metadata: PluginMetadata,
    state: PluginState,
    provider: Arc<dyn LLMProvider>,
    config: LLMConfig,
    stats: RwLock<LLMStats>,
}

/// LLM 统计信息
/// LLM statistics information
#[derive(Debug, Default)]
struct LLMStats {
    total_requests: u64,
    total_tokens: u64,
    total_prompt_tokens: u64,
    total_completion_tokens: u64,
    failed_requests: u64,
    avg_latency_ms: f64,
}

impl LLMPlugin {
    /// 创建新的 LLM 插件
    /// Create a new LLM plugin
    pub fn new(id: &str, provider: Arc<dyn LLMProvider>) -> Self {
        let metadata = PluginMetadata::new(id, provider.name(), PluginType::LLM)
            .with_description(&format!("LLM provider: {}", provider.name()))
            .with_priority(PluginPriority::High)
            .with_capability("chat")
            .with_capability("text-generation");

        Self {
            metadata,
            state: PluginState::Unloaded,
            provider,
            config: LLMConfig::default(),
            stats: RwLock::new(LLMStats::default()),
        }
    }

    /// 使用配置创建插件
    /// Create plugin with configuration
    pub fn with_config(id: &str, provider: Arc<dyn LLMProvider>, config: LLMConfig) -> Self {
        let mut plugin = Self::new(id, provider);
        plugin.config = config;
        plugin
    }

    /// 获取 Provider
    /// Get the Provider
    pub fn provider(&self) -> &Arc<dyn LLMProvider> {
        &self.provider
    }

    /// 获取配置
    /// Get configuration
    pub fn llm_config(&self) -> &LLMConfig {
        &self.config
    }

    /// 发送 Chat 请求
    /// Send Chat request
    pub async fn chat(&self, request: ChatCompletionRequest) -> LLMResult<ChatCompletionResponse> {
        let start = std::time::Instant::now();

        let result = self.provider.chat(request).await;

        // 更新统计
        // Update statistics
        let mut stats = self.stats.write().await;
        stats.total_requests += 1;

        match &result {
            Ok(response) => {
                if let Some(usage) = &response.usage {
                    stats.total_tokens += usage.total_tokens as u64;
                    stats.total_prompt_tokens += usage.prompt_tokens as u64;
                    stats.total_completion_tokens += usage.completion_tokens as u64;
                }
                let latency = start.elapsed().as_millis() as f64;
                stats.avg_latency_ms = (stats.avg_latency_ms * (stats.total_requests - 1) as f64
                    + latency)
                    / stats.total_requests as f64;
            }
            Err(_) => {
                stats.failed_requests += 1;
            }
        }

        result
    }

    /// 简单问答
    /// Simple Q&A
    pub async fn ask(&self, question: &str) -> LLMResult<String> {
        let model = self
            .config
            .default_model
            .clone()
            .unwrap_or_else(|| self.provider.default_model().to_string());

        let request = ChatCompletionRequest::new(model)
            .user(question)
            .temperature(self.config.default_temperature.unwrap_or(0.7))
            .max_tokens(self.config.default_max_tokens.unwrap_or(4096));

        let response = self.chat(request).await?;

        response
            .content()
            .map(|s| s.to_string())
            .ok_or_else(|| LLMError::Other("No content in response".to_string()))
    }
}

#[async_trait::async_trait]
impl AgentPlugin for LLMPlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    fn state(&self) -> PluginState {
        self.state.clone()
    }

    async fn load(&mut self, ctx: &PluginContext) -> PluginResult<()> {
        self.state = PluginState::Loading;

        // 从上下文配置中读取 LLM 配置
        // Read LLM configuration from plugin context
        if let Some(api_key) = ctx.config.get_string("api_key") {
            self.config.api_key = Some(api_key);
        }
        if let Some(base_url) = ctx.config.get_string("base_url") {
            self.config.base_url = Some(base_url);
        }
        if let Some(model) = ctx.config.get_string("model") {
            self.config.default_model = Some(model);
        }

        self.state = PluginState::Loaded;
        Ok(())
    }

    async fn init_plugin(&mut self) -> PluginResult<()> {
        // 健康检查
        // Health check
        self.provider.health_check().await.map_err(|e| {
            self.state = PluginState::Error(e.to_string());
            mofa_kernel::plugin::PluginError::InitFailed(format!("LLM health check failed: {}", e))
        })?;

        Ok(())
    }

    async fn start(&mut self) -> PluginResult<()> {
        self.state = PluginState::Running;
        Ok(())
    }

    async fn stop(&mut self) -> PluginResult<()> {
        self.state = PluginState::Paused;
        Ok(())
    }

    async fn unload(&mut self) -> PluginResult<()> {
        self.state = PluginState::Unloaded;
        Ok(())
    }

    async fn execute(&mut self, input: String) -> PluginResult<String> {
        // 简单模式：直接将输入作为用户消息
        // Simple mode: treat input directly as a user message
        self.ask(&input)
            .await
            .map_err(|e| mofa_kernel::plugin::PluginError::ExecutionFailed(format!("LLM execution failed: {}", e)))
    }

    fn stats(&self) -> HashMap<String, serde_json::Value> {
        // 注意：这里使用 try_read 避免阻塞
        // Note: use try_read here to avoid blocking
        let stats = match self.stats.try_read() {
            Ok(s) => s,
            Err(_) => return HashMap::new(),
        };

        let mut result = HashMap::new();
        result.insert(
            "total_requests".to_string(),
            serde_json::json!(stats.total_requests),
        );
        result.insert(
            "total_tokens".to_string(),
            serde_json::json!(stats.total_tokens),
        );
        result.insert(
            "total_prompt_tokens".to_string(),
            serde_json::json!(stats.total_prompt_tokens),
        );
        result.insert(
            "total_completion_tokens".to_string(),
            serde_json::json!(stats.total_completion_tokens),
        );
        result.insert(
            "failed_requests".to_string(),
            serde_json::json!(stats.failed_requests),
        );
        result.insert(
            "avg_latency_ms".to_string(),
            serde_json::json!(stats.avg_latency_ms),
        );
        result
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }
}

// ============================================================================
// LLM 能力扩展
// LLM Capability Extension
// ============================================================================

/// LLM 能力 trait
/// LLM Capability trait
///
/// 为 Agent 提供 LLM 交互能力
/// Provides LLM interaction capabilities for the Agent
#[async_trait::async_trait]
pub trait LLMCapability: Send + Sync {
    /// 获取 LLM 提供商
    /// Get the LLM provider
    fn llm_provider(&self) -> Option<&Arc<dyn LLMProvider>>;

    /// 简单问答
    /// Simple Q&A
    async fn llm_ask(&self, question: &str) -> LLMResult<String> {
        let provider = self
            .llm_provider()
            .ok_or_else(|| LLMError::ConfigError("LLM provider not configured".to_string()))?;

        let request = ChatCompletionRequest::new(provider.default_model()).user(question);

        let response = provider.chat(request).await?;

        response
            .content()
            .map(|s| s.to_string())
            .ok_or_else(|| LLMError::Other("No content in response".to_string()))
    }

    /// 带系统提示的问答
    /// Q&A with system prompt
    async fn llm_ask_with_system(&self, system: &str, question: &str) -> LLMResult<String> {
        let provider = self
            .llm_provider()
            .ok_or_else(|| LLMError::ConfigError("LLM provider not configured".to_string()))?;

        let request = ChatCompletionRequest::new(provider.default_model())
            .system(system)
            .user(question);

        let response = provider.chat(request).await?;

        response
            .content()
            .map(|s| s.to_string())
            .ok_or_else(|| LLMError::Other("No content in response".to_string()))
    }

    /// 发送完整的 Chat 请求
    /// Send complete Chat request
    async fn llm_chat(&self, request: ChatCompletionRequest) -> LLMResult<ChatCompletionResponse> {
        let provider = self
            .llm_provider()
            .ok_or_else(|| LLMError::ConfigError("LLM provider not configured".to_string()))?;

        provider.chat(request).await
    }
}

// ============================================================================
// Mock Provider（用于测试）
// Mock Provider (for testing)
// ============================================================================

/// Mock LLM Provider（用于测试）
/// Mock LLM Provider (for testing)
pub struct MockLLMProvider {
    name: String,
    responses: RwLock<Vec<String>>,
    default_response: String,
}

impl MockLLMProvider {
    /// 创建 Mock Provider
    /// Create Mock Provider
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            responses: RwLock::new(Vec::new()),
            default_response: "This is a mock response.".to_string(),
        }
    }

    /// 设置默认响应
    /// Set default response
    pub fn with_default_response(mut self, response: impl Into<String>) -> Self {
        self.default_response = response.into();
        self
    }

    /// 添加预设响应（按顺序返回）
    /// Add preset responses (returned in order)
    pub async fn add_response(&self, response: impl Into<String>) {
        let mut responses = self.responses.write().await;
        responses.push(response.into());
    }
}

#[async_trait::async_trait]
impl LLMProvider for MockLLMProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn default_model(&self) -> &str {
        "mock-model"
    }

    fn supported_models(&self) -> Vec<&str> {
        vec!["mock-model", "mock-model-large"]
    }

    fn supports_streaming(&self) -> bool {
        false
    }

    fn supports_tools(&self) -> bool {
        true
    }

    async fn chat(&self, _request: ChatCompletionRequest) -> LLMResult<ChatCompletionResponse> {
        let content = {
            let mut responses = self.responses.write().await;
            if responses.is_empty() {
                self.default_response.clone()
            } else {
                responses.remove(0)
            }
        };

        Ok(ChatCompletionResponse {
            id: format!("mock-{}", uuid::Uuid::now_v7()),
            object: "chat.completion".to_string(),
            created: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            model: "mock-model".to_string(),
            choices: vec![Choice {
                index: 0,
                message: ChatMessage::assistant(content),
                finish_reason: Some(FinishReason::Stop),
                logprobs: None,
            }],
            usage: Some(Usage {
                prompt_tokens: 10,
                completion_tokens: 20,
                total_tokens: 30,
            }),
            system_fingerprint: None,
        })
    }
}

// Would you like me to add similar English comments to other files in your MoFA project?
