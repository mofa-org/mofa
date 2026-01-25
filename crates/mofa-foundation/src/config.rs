//! Agent 配置文件解析
//!
//! 支持多种配置格式: YAML, TOML, JSON, INI, RON, JSON5
//!
//! # 示例配置 (agent.yml, agent.toml, agent.json, etc.)
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
//!   base_url: null            # 可选，用于自定义 endpoint
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

use mofa_kernel::config::{from_str, load_config, substitute_env_vars};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Agent 配置文件根结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentYamlConfig {
    /// Agent 基本信息
    pub agent: AgentInfo,
    /// LLM 配置
    #[serde(default)]
    pub llm: Option<LLMYamlConfig>,
    /// 工具配置
    #[serde(default)]
    pub tools: Option<Vec<ToolConfig>>,
    /// 运行时配置
    #[serde(default)]
    pub runtime: Option<RuntimeConfig>,
    /// 输入端口
    #[serde(default)]
    pub inputs: Option<Vec<String>>,
    /// 输出端口
    #[serde(default)]
    pub outputs: Option<Vec<String>>,
}

/// Agent 基本信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    /// Agent ID
    pub id: String,
    /// Agent 名称
    pub name: String,
    /// 描述
    #[serde(default)]
    pub description: Option<String>,
    /// 能力列表
    #[serde(default)]
    pub capabilities: Vec<String>,
}

/// LLM 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMYamlConfig {
    /// Provider 类型: openai, ollama, azure, compatible
    #[serde(default = "default_provider")]
    pub provider: String,
    /// 模型名称
    #[serde(default)]
    pub model: Option<String>,
    /// API Key (支持 ${ENV_VAR} 语法)
    #[serde(default)]
    pub api_key: Option<String>,
    /// API Base URL
    #[serde(default)]
    pub base_url: Option<String>,
    /// Azure deployment name
    #[serde(default)]
    pub deployment: Option<String>,
    /// 温度参数
    #[serde(default)]
    pub temperature: Option<f32>,
    /// 最大 token 数
    #[serde(default)]
    pub max_tokens: Option<u32>,
    /// 系统提示词
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
            system_prompt: None,
        }
    }
}

/// 工具配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolConfig {
    /// 工具名称
    pub name: String,
    /// 是否启用
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// 工具特定配置
    #[serde(default)]
    pub config: HashMap<String, serde_json::Value>,
}

fn default_true() -> bool {
    true
}

/// 运行时配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    /// 最大并发任务数
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent_tasks: usize,
    /// 默认超时（秒）
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
    pub fn from_file(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let path_str = path.as_ref().to_string_lossy().to_string();
        load_config(&path_str).map_err(|e| anyhow::anyhow!("Failed to load config: {}", e))
    }

    /// 从字符串解析配置 (指定格式)
    pub fn from_str_with_format(content: &str, format: &str) -> anyhow::Result<Self> {
        use config::FileFormat;

        let file_format = match format.to_lowercase().as_str() {
            "yaml" | "yml" => FileFormat::Yaml,
            "toml" => FileFormat::Toml,
            "json" => FileFormat::Json,
            "ini" => FileFormat::Ini,
            "ron" => FileFormat::Ron,
            "json5" => FileFormat::Json5,
            _ => return Err(anyhow::anyhow!("Unsupported config format: {}", format)),
        };

        from_str(content, file_format).map_err(|e| anyhow::anyhow!("Failed to parse config: {}", e))
    }

    /// 从字符串解析配置 (自动检测为 YAML，保持向后兼容)
    pub fn from_str(content: &str) -> anyhow::Result<Self> {
        Self::from_str_with_format(content, "yaml")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_yaml_config() {
        let yaml = r#"
agent:
  id: "test-agent"
  name: "Test Agent"
  capabilities:
    - llm
    - tools

llm:
  provider: openai
  model: gpt-4o
  temperature: 0.8
  system_prompt: "Be helpful."

runtime:
  max_concurrent_tasks: 5
  default_timeout_secs: 60
"#;

        let config = AgentYamlConfig::from_str(yaml).unwrap();

        assert_eq!(config.agent.id, "test-agent");
        assert_eq!(config.agent.name, "Test Agent");
        assert_eq!(config.agent.capabilities.len(), 2);

        let llm = config.llm.unwrap();
        assert_eq!(llm.provider, "openai");
        assert_eq!(llm.model, Some("gpt-4o".to_string()));
        assert_eq!(llm.temperature, Some(0.8));

        let runtime = config.runtime.unwrap();
        assert_eq!(runtime.max_concurrent_tasks, 5);
        assert_eq!(runtime.default_timeout_secs, 60);
    }

    #[test]
    fn test_parse_json_config() {
        let json = r#"{
    "agent": {
        "id": "test-agent",
        "name": "Test Agent",
        "capabilities": ["llm", "tools"]
    },
    "llm": {
        "provider": "openai",
        "model": "gpt-4o",
        "temperature": 0.8
    }
}"#;

        let config = AgentYamlConfig::from_str_with_format(json, "json").unwrap();

        assert_eq!(config.agent.id, "test-agent");
        assert_eq!(config.agent.name, "Test Agent");
        assert_eq!(config.agent.capabilities.len(), 2);

        let llm = config.llm.unwrap();
        assert_eq!(llm.provider, "openai");
        assert_eq!(llm.model, Some("gpt-4o".to_string()));
    }

    #[test]
    fn test_parse_toml_config() {
        let toml = r#"
[agent]
id = "test-agent"
name = "Test Agent"
capabilities = ["llm", "tools"]

[llm]
provider = "openai"
model = "gpt-4o"
temperature = 0.8
"#;

        let config = AgentYamlConfig::from_str_with_format(toml, "toml").unwrap();

        assert_eq!(config.agent.id, "test-agent");
        assert_eq!(config.agent.name, "Test Agent");
        assert_eq!(config.agent.capabilities.len(), 2);

        let llm = config.llm.unwrap();
        assert_eq!(llm.provider, "openai");
        assert_eq!(llm.model, Some("gpt-4o".to_string()));
    }

    #[test]
    fn test_substitute_env_vars() {
        std::env::set_var("TEST_API_KEY", "sk-test-key-12345");

        let result = substitute_env_vars("api_key: ${TEST_API_KEY}");
        assert_eq!(result, "api_key: sk-test-key-12345");

        let result = substitute_env_vars("url: $TEST_API_KEY");
        assert_eq!(result, "url: sk-test-key-12345");

        std::env::remove_var("TEST_API_KEY");
    }

    #[test]
    fn test_env_vars_in_config() {
        std::env::set_var("OPENAI_API_KEY", "sk-test-key-12345");
        std::env::set_var("OPENAI_BASE_URL", "https://api.test.com");

        let yaml = r#"
agent:
  id: "test-agent"
  name: "Test Agent"

llm:
  provider: openai
  api_key: ${OPENAI_API_KEY}
  base_url: ${OPENAI_BASE_URL}
"#;

        let config = AgentYamlConfig::from_str(yaml).unwrap();
        let llm = config.llm.unwrap();

        assert_eq!(llm.api_key, Some("sk-test-key-12345".to_string()));
        assert_eq!(llm.base_url, Some("https://api.test.com".to_string()));

        std::env::remove_var("OPENAI_API_KEY");
        std::env::remove_var("OPENAI_BASE_URL");
    }
}
