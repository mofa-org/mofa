//! LLM Provider Trait
//! LLM Provider Trait
//!
//! 定义 LLM 提供商接口，支持多种 LLM 后端
//! Defines the LLM provider interface, supporting multiple LLM backends

use super::types::*;
use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;

/// 流式响应类型
/// Streaming response type
pub type ChatStream = Pin<Box<dyn Stream<Item = LLMResult<ChatCompletionChunk>> + Send>>;

/// LLM 提供商 trait
/// LLM Provider trait
///
/// 所有 LLM 后端（OpenAI、Anthropic、本地模型等）都需要实现此 trait
/// All LLM backends (OpenAI, Anthropic, local models, etc.) need to implement this trait
///
/// # 示例
/// # Example
///
/// ```rust,ignore
/// use mofa_foundation::llm::{LLMProvider, ChatCompletionRequest, LLMResult};
///
/// struct MyLLMProvider {
///     api_key: String,
///     base_url: String,
/// }
///
/// #[async_trait::async_trait]
/// impl LLMProvider for MyLLMProvider {
///     fn name(&self) -> &str {
///         "my-llm"
///     }
///
///     async fn chat(&self, request: ChatCompletionRequest) -> LLMResult<ChatCompletionResponse> {
///         // 实现 API 调用
///         // Implement API call
///         todo!()
///     }
/// }
/// ```
#[async_trait]
pub trait LLMProvider: Send + Sync {
    /// 获取提供商名称
    /// Get provider name
    fn name(&self) -> &str;

    /// 获取默认模型
    /// Get default model
    fn default_model(&self) -> &str {
        ""
    }

    /// 获取支持的模型列表
    /// Get list of supported models
    fn supported_models(&self) -> Vec<&str> {
        vec![]
    }

    /// 检查是否支持某个模型
    /// Check if a model is supported
    fn supports_model(&self, model: &str) -> bool {
        self.supported_models().contains(&model)
    }

    /// 检查是否支持流式输出
    /// Check if streaming output is supported
    fn supports_streaming(&self) -> bool {
        true
    }

    /// 检查是否支持工具调用
    fn supports_tools(&self) -> bool {
        true
    }

    /// 检查是否支持视觉（图片输入）
    /// Check if vision (image input) is supported
    fn supports_vision(&self) -> bool {
        false
    }

    /// 检查是否支持 embedding
    /// Check if embedding is supported
    fn supports_embedding(&self) -> bool {
        false
    }

    /// 发送 Chat Completion 请求
    /// Send Chat Completion request
    async fn chat(&self, request: ChatCompletionRequest) -> LLMResult<ChatCompletionResponse>;

    /// 发送 Chat Completion 流式请求
    /// Send Chat Completion streaming request
    async fn chat_stream(&self, _request: ChatCompletionRequest) -> LLMResult<ChatStream> {
        // 默认实现：不支持流式
        // Default implementation: streaming not supported
        Err(LLMError::ProviderNotSupported(format!(
            "Provider {} does not support streaming",
            self.name()
        )))
    }

    /// 发送 Embedding 请求
    /// Send Embedding request
    async fn embedding(&self, _request: EmbeddingRequest) -> LLMResult<EmbeddingResponse> {
        Err(LLMError::ProviderNotSupported(format!(
            "Provider {} does not support embedding",
            self.name()
        )))
    }

    /// 健康检查
    /// Health check
    async fn health_check(&self) -> LLMResult<bool> {
        Ok(true)
    }

    /// 获取模型信息
    /// Get model information
    async fn get_model_info(&self, _model: &str) -> LLMResult<ModelInfo> {
        Err(LLMError::ProviderNotSupported(format!(
            "Provider {} does not support model info",
            self.name()
        )))
    }
}

/// 模型信息
/// Model information
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ModelInfo {
    /// 模型 ID
    /// Model ID
    pub id: String,
    /// 模型名称
    /// Model name
    pub name: String,
    /// 模型描述
    /// Model description
    pub description: Option<String>,
    /// 上下文窗口大小
    /// Context window size
    pub context_window: Option<u32>,
    /// 最大输出 token 数
    /// Max output tokens
    pub max_output_tokens: Option<u32>,
    /// 训练数据截止日期
    /// Training data cutoff date
    pub training_cutoff: Option<String>,
    /// 支持的功能
    /// Supported capabilities
    pub capabilities: ModelCapabilities,
}

/// 模型功能
/// Model capabilities
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ModelCapabilities {
    /// 支持流式输出
    /// Support streaming output
    pub streaming: bool,
    /// 支持工具调用
    /// Support tool calling
    pub tools: bool,
    /// 支持视觉（图片输入）
    /// Support vision (image input)
    pub vision: bool,
    /// 支持 JSON 模式
    /// Support JSON mode
    pub json_mode: bool,
    /// 支持 JSON Schema
    /// Support JSON Schema
    pub json_schema: bool,
}

/// LLM 配置
/// LLM Configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LLMConfig {
    /// 提供商名称
    /// Provider name
    pub provider: String,
    /// API Key
    /// API Key
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    /// API 基础 URL
    /// API Base URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    /// 默认模型
    /// Default model
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_model: Option<String>,
    /// 默认温度
    /// Default temperature
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_temperature: Option<f32>,
    /// 默认最大 token 数
    /// Default max tokens
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_max_tokens: Option<u32>,
    /// 请求超时（秒）
    /// Request timeout (seconds)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_secs: Option<u64>,
    /// 重试次数
    /// Retry count
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_retries: Option<u32>,
    /// 额外配置
    /// Extra configuration
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

impl Default for LLMConfig {
    fn default() -> Self {
        Self {
            provider: "openai".to_string(),
            api_key: None,
            base_url: None,
            default_model: None,
            default_temperature: Some(0.7),
            default_max_tokens: Some(4096),
            timeout_secs: Some(60),
            max_retries: Some(3),
            extra: std::collections::HashMap::new(),
        }
    }
}

impl LLMConfig {
    /// 创建 OpenAI 配置
    /// Create OpenAI configuration
    pub fn openai(api_key: impl Into<String>) -> Self {
        Self {
            provider: "openai".to_string(),
            api_key: Some(api_key.into()),
            base_url: Some("https://api.openai.com/v1".to_string()),
            default_model: Some("gpt-4".to_string()),
            ..Default::default()
        }
    }

    /// 创建 Anthropic 配置
    /// Create Anthropic configuration
    pub fn anthropic(api_key: impl Into<String>) -> Self {
        Self {
            provider: "anthropic".to_string(),
            api_key: Some(api_key.into()),
            base_url: Some("https://api.anthropic.com".to_string()),
            default_model: Some("claude-3-sonnet-20240229".to_string()),
            ..Default::default()
        }
    }

    /// 创建本地 Ollama 配置
    /// Create local Ollama configuration
    pub fn ollama(model: impl Into<String>) -> Self {
        Self {
            provider: "ollama".to_string(),
            api_key: None,
            base_url: Some("http://localhost:11434".to_string()),
            default_model: Some(model.into()),
            ..Default::default()
        }
    }

    /// 创建兼容 OpenAI API 的配置
    /// Create OpenAI compatible API configuration
    pub fn openai_compatible(
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        Self {
            provider: "openai-compatible".to_string(),
            api_key: Some(api_key.into()),
            base_url: Some(base_url.into()),
            default_model: Some(model.into()),
            ..Default::default()
        }
    }

    /// 设置模型
    /// Set model
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.default_model = Some(model.into());
        self
    }

    /// 设置温度
    /// Set temperature
    pub fn temperature(mut self, temp: f32) -> Self {
        self.default_temperature = Some(temp);
        self
    }

    /// 设置最大 token 数
    /// Set maximum tokens
    pub fn max_tokens(mut self, tokens: u32) -> Self {
        self.default_max_tokens = Some(tokens);
        self
    }
}

// ============================================================================
// 可扩展的 Provider 注册表
// Extensible Provider Registry
// ============================================================================

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Provider 工厂函数类型
/// Provider factory function type
pub type ProviderFactory = Box<dyn Fn(LLMConfig) -> LLMResult<Box<dyn LLMProvider>> + Send + Sync>;

/// LLM Provider 注册表
/// LLM Provider Registry
///
/// 用于注册和创建 LLM Provider 实例
/// Used to register and create LLM Provider instances
pub struct LLMRegistry {
    factories: RwLock<HashMap<String, ProviderFactory>>,
    providers: RwLock<HashMap<String, Arc<dyn LLMProvider>>>,
}

impl LLMRegistry {
    /// 创建新的注册表
    /// Create a new registry
    pub fn new() -> Self {
        Self {
            factories: RwLock::new(HashMap::new()),
            providers: RwLock::new(HashMap::new()),
        }
    }

    /// 注册 Provider 工厂
    /// Register Provider factory
    pub async fn register_factory<F>(&self, name: &str, factory: F)
    where
        F: Fn(LLMConfig) -> LLMResult<Box<dyn LLMProvider>> + Send + Sync + 'static,
    {
        let mut factories = self.factories.write().await;
        factories.insert(name.to_string(), Box::new(factory));
    }

    /// 创建 Provider 实例
    /// Create Provider instance
    pub async fn create(&self, config: LLMConfig) -> LLMResult<Arc<dyn LLMProvider>> {
        let factories = self.factories.read().await;
        let factory = factories
            .get(&config.provider)
            .ok_or_else(|| LLMError::ProviderNotSupported(config.provider.clone()))?;

        let provider = factory(config)?;
        Ok(Arc::from(provider))
    }

    /// 注册并缓存 Provider 实例
    /// Register and cache Provider instance
    pub async fn register(&self, name: &str, provider: Arc<dyn LLMProvider>) {
        let mut providers = self.providers.write().await;
        providers.insert(name.to_string(), provider);
    }

    /// 获取已注册的 Provider
    /// Get registered Provider
    pub async fn get(&self, name: &str) -> Option<Arc<dyn LLMProvider>> {
        let providers = self.providers.read().await;
        providers.get(name).cloned()
    }

    /// 列出所有已注册的 Provider 名称
    /// List all registered Provider names
    pub async fn list_providers(&self) -> Vec<String> {
        let providers = self.providers.read().await;
        providers.keys().cloned().collect()
    }

    /// 列出所有可用的 Provider 工厂名称
    /// List all available Provider factory names
    pub async fn list_factories(&self) -> Vec<String> {
        let factories = self.factories.read().await;
        factories.keys().cloned().collect()
    }
}

impl Default for LLMRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// 全局 Registry（可选）
// Global Registry (Optional)
// ============================================================================

use std::sync::OnceLock;

static GLOBAL_REGISTRY: OnceLock<LLMRegistry> = OnceLock::new();

/// 获取全局 LLM 注册表
/// Get the global LLM registry
pub fn global_registry() -> &'static LLMRegistry {
    GLOBAL_REGISTRY.get_or_init(LLMRegistry::new)
}
