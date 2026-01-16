//! Agent 配置文件解析
//!
//! 支持从 agent.yml 文件读取配置
//!
//! # 示例配置 (agent.yml)
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
    /// 从文件加载配置
    pub fn from_file(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path.as_ref())?;
        Self::from_str(&content)
    }

    /// 从字符串解析配置
    pub fn from_str(content: &str) -> anyhow::Result<Self> {
        let mut config: Self = serde_yaml::from_str(content)?;
        config.resolve_env_vars();
        Ok(config)
    }

    /// 解析环境变量
    fn resolve_env_vars(&mut self) {
        if let Some(ref mut llm) = self.llm {
            if let Some(ref mut api_key) = llm.api_key {
                *api_key = resolve_env_var(api_key);
            }
            if let Some(ref mut base_url) = llm.base_url {
                *base_url = resolve_env_var(base_url);
            }
        }
    }
}

/// 解析环境变量语法 ${VAR_NAME} 或 $VAR_NAME
fn resolve_env_var(value: &str) -> String {
    let value = value.trim();

    // ${VAR_NAME} 格式
    if value.starts_with("${") && value.ends_with('}') {
        let var_name = &value[2..value.len() - 1];
        return std::env::var(var_name).unwrap_or_default();
    }

    // $VAR_NAME 格式
    if value.starts_with('$') && !value.contains('{') {
        let var_name = &value[1..];
        return std::env::var(var_name).unwrap_or_default();
    }

    value.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_config() {
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
}
