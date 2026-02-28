//! Agent 配置文件解析
//! Agent configuration file parsing
//!
//! 支持多种配置格式: YAML, TOML, JSON, INI, RON, JSON5
//! Supports multiple config formats: YAML, TOML, JSON, INI, RON, JSON5
//!
//! # 示例配置 (agent.yml, agent.toml, agent.json, etc.)
//! # Example configuration (agent.yml, agent.toml, agent.json, etc.)
//!
//! ```yaml
//! agent:
//!   id: "my-agent-001"
//!   name: "My LLM Agent"
//!
//! llm:
//!   provider: openai          # openai, ollama, azure
//!   model: gpt-4o
//!   api_key: ${OPENAI_API_KEY}  # 支持环境变量
//!                               # Supports environment variables
//!   base_url: null            # 可选，用于自定义 endpoint
//!                             # Optional, used for custom endpoints
//!   temperature: 0.7
//!   max_tokens: 4096
//!   system_prompt: "You are a helpful assistant."
//!
//! tools:
//!   - name: web_search
//!     enabled: true
//!   - name: calculator
//!     enabled: true
//!
//! runtime:
//!   max_concurrent_tasks: 10
//!   default_timeout_secs: 30
//! ```

use mofa_kernel::agent::types::error::{GlobalError, GlobalResult};
use mofa_kernel::config::{from_str, load_config};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Agent 配置文件根结构
/// Agent configuration file root structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentYamlConfig {
    /// Agent 基本信息
    /// Agent basic information
    pub agent: AgentInfo,
    /// LLM 配置
    /// LLM configuration
    #[serde(default)]
    pub llm: Option<LLMYamlConfig>,
    /// 工具配置
    /// Tools configuration
    #[serde(default)]
    pub tools: Option<Vec<ToolConfig>>,
    /// 运行时配置
    /// Runtime configuration
    #[serde(default)]
    pub runtime: Option<RuntimeConfig>,
    /// 输入端口
    /// Input ports
    #[serde(default)]
    pub inputs: Option<Vec<String>>,
    /// 输出端口
    /// Output ports
    #[serde(default)]
    pub outputs: Option<Vec<String>>,
}

/// Agent 基本信息
/// Agent basic information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    /// Agent ID
    /// Agent ID
    pub id: String,
    /// Agent 名称
    /// Agent name
    pub name: String,
    /// 描述
    /// Description
    #[serde(default)]
    pub description: Option<String>,
    /// 能力列表
    /// Capabilities list
    #[serde(default)]
    pub capabilities: Vec<String>,
}

/// LLM 配置
/// LLM configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMYamlConfig {
    /// Provider 类型: openai, ollama, azure, compatible, anthropic, gemini
    /// Provider type: openai, ollama, azure, compatible, anthropic, gemini
    #[serde(default = "default_provider")]
    pub provider: String,
    /// 模型名称
    /// Model name
    #[serde(default)]
    pub model: Option<String>,
    /// API Key (支持 ${ENV_VAR} 语法)
    /// API Key (supports ${ENV_VAR} syntax)
    #[serde(default)]
    pub api_key: Option<String>,
    /// API Base URL
    /// API Base URL
    #[serde(default)]
    pub base_url: Option<String>,
    /// Azure deployment name
    /// Azure deployment name
    #[serde(default)]
    pub deployment: Option<String>,
    /// 温度参数
    /// Temperature parameter
    #[serde(default)]
    pub temperature: Option<f32>,
    /// 最大 token 数 (output generation limit)
    /// Maximum token count (output generation limit)
    #[serde(default)]
    pub max_tokens: Option<u32>,
    /// 上下文窗口大小 (input token budget)
    /// Context window size (input token budget)
    ///
    /// When set, the framework will automatically trim conversation history
    /// to fit within this token budget. This is the *input* limit, distinct
    /// from `max_tokens` which controls *output* generation length.
    #[serde(default)]
    pub context_window_tokens: Option<u32>,
    /// 系统提示词
    /// System prompt
    #[serde(default)]
    pub system_prompt: Option<String>,
}

fn default_provider() -> String {
    "openai".to_string()
}

impl Default for LLMYamlConfig {
    fn default() -> Self {
        Self {
            provider: "openai".to_string(),
            model: None,
            api_key: None,
            base_url: None,
            deployment: None,
            temperature: Some(0.7),
            max_tokens: Some(4096),
            context_window_tokens: None,
            system_prompt: None,
        }
    }
}

/// 工具配置
/// Tool configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolConfig {
    /// 工具名称
    /// Tool name
    pub name: String,
    /// 是否启用
    /// Whether enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// 工具特定配置
    /// Tool specific configuration
    #[serde(default)]
    pub config: HashMap<String, serde_json::Value>,
}

fn default_true() -> bool {
    true
}

/// 运行时配置
/// Runtime configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    /// 最大并发任务数
    /// Maximum concurrent tasks
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent_tasks: usize,
    /// 默认超时（秒）
    /// Default timeout (seconds)
    #[serde(default = "default_timeout")]
    pub default_timeout_secs: u64,
}

fn default_max_concurrent() -> usize {
    10
}

fn default_timeout() -> u64 {
    30
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            max_concurrent_tasks: 10,
            default_timeout_secs: 30,
        }
    }
}

impl AgentYamlConfig {
    /// 从文件加载配置 (自动检测格式)
    /// Load config from file (auto-detect format)
    pub fn from_file(path: impl AsRef<Path>) -> GlobalResult<Self> {
        let path_str = path.as_ref().to_string_lossy().to_string();
        load_config(&path_str).map_err(|e| GlobalError::Other(format!("Failed to load config: {}", e)))
    }

    /// 从字符串解析配置 (指定格式)
    /// Parse config from string (specified format)
    pub fn from_str_with_format(content: &str, format: &str) -> GlobalResult<Self> {
        use config::FileFormat;

        let file_format = match format.to_lowercase().as_str() {
            "yaml" | "yml" => FileFormat::Yaml,
            "toml" => FileFormat::Toml,
            "json" => FileFormat::Json,
            "ini" => FileFormat::Ini,
            "ron" => FileFormat::Ron,
            "json5" => FileFormat::Json5,
            _ => return Err(GlobalError::Other(format!("Unsupported config format: {}", format))),
        };

        from_str(content, file_format).map_err(|e| GlobalError::Other(format!("Failed to parse config: {}", e)))
    }

    /// 从字符串解析配置 (自动检测为 YAML)
    /// Parse config from string (defaults to YAML)
    pub fn parse(content: &str) -> GlobalResult<Self> {
        Self::from_str_with_format(content, "yaml")
    }
}
