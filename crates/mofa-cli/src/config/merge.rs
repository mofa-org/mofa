//! Configuration merging utilities

use super::AgentConfig;

/// Configuration merge strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigMergeStrategy {
    /// File values take precedence over environment
    FilePrecedence,
    /// Environment values take precedence over file
    EnvPrecedence,
    /// CLI values take highest precedence
    CliPrecedence,
}

/// Merge multiple configuration sources
pub fn merge_configs(
    mut base: AgentConfig,
    overrides: AgentConfig,
    _strategy: ConfigMergeStrategy,
) -> AgentConfig {
    // Merge agent identity
    if !overrides.agent.id.is_empty() {
        base.agent.id = overrides.agent.id;
    }
    if !overrides.agent.name.is_empty() {
        base.agent.name = overrides.agent.name;
    }
    if overrides.agent.description.is_some() {
        base.agent.description = overrides.agent.description;
    }
    if overrides.agent.capabilities.is_some() {
        base.agent.capabilities = overrides.agent.capabilities;
    }

    // Merge LLM config
    if let Some(override_llm) = overrides.llm {
        let base_llm = base.llm.get_or_insert_with(|| super::LlmConfig {
            provider: "openai".to_string(),
            model: String::new(),
            api_key: None,
            base_url: None,
            temperature: None,
            max_tokens: None,
            system_prompt: None,
        });

        if !override_llm.provider.is_empty() {
            base_llm.provider = override_llm.provider;
        }
        if !override_llm.model.is_empty() {
            base_llm.model = override_llm.model;
        }
        if override_llm.api_key.is_some() {
            base_llm.api_key = override_llm.api_key;
        }
        if override_llm.base_url.is_some() {
            base_llm.base_url = override_llm.base_url;
        }
        if override_llm.temperature.is_some() {
            base_llm.temperature = override_llm.temperature;
        }
        if override_llm.max_tokens.is_some() {
            base_llm.max_tokens = override_llm.max_tokens;
        }
        if override_llm.system_prompt.is_some() {
            base_llm.system_prompt = override_llm.system_prompt;
        }
    }

    // Merge runtime config
    if let Some(override_runtime) = overrides.runtime {
        let base_runtime = base.runtime.get_or_insert(super::RuntimeConfig {
            max_concurrent_tasks: None,
            default_timeout_secs: None,
            dora_enabled: None,
            persistence: None,
        });

        if override_runtime.max_concurrent_tasks.is_some() {
            base_runtime.max_concurrent_tasks = override_runtime.max_concurrent_tasks;
        }
        if override_runtime.default_timeout_secs.is_some() {
            base_runtime.default_timeout_secs = override_runtime.default_timeout_secs;
        }
        if override_runtime.dora_enabled.is_some() {
            base_runtime.dora_enabled = override_runtime.dora_enabled;
        }
        if override_runtime.persistence.is_some() {
            base_runtime.persistence = override_runtime.persistence;
        }
    }

    // Merge node config
    for (key, value) in overrides.node_config {
        base.node_config.insert(key, value);
    }

    base
}

/// Create config from environment variables
pub fn config_from_env(prefix: &str) -> AgentConfig {
    let mut config = AgentConfig::default();
    let prefix = prefix.trim_end_matches('_');

    // Parse environment variables
    for (key, value) in std::env::vars() {
        if key.as_str().starts_with(prefix) {
            let rest = &key[prefix.len()..];
            let rest = rest.trim_start_matches('_');

            let parts: Vec<&str> = rest.split('_').collect();

            match parts.as_slice() {
                ["AGENT", "ID"] => config.agent.id = value,
                ["AGENT", "NAME"] => config.agent.name = value,
                ["AGENT", "DESCRIPTION"] => config.agent.description = Some(value),
                ["LLM", "PROVIDER"] => {
                    config.llm.get_or_insert_with(default_llm_config).provider = value;
                }
                ["LLM", "MODEL"] => {
                    config.llm.get_or_insert_with(default_llm_config).model = value;
                }
                ["LLM", "API", "KEY"] => {
                    config.llm.get_or_insert_with(default_llm_config).api_key = Some(value);
                }
                ["LLM", "BASE", "URL"] => {
                    config.llm.get_or_insert_with(default_llm_config).base_url = Some(value);
                }
                ["LLM", "TEMPERATURE"] => {
                    if let Ok(temp) = value.parse::<f32>() {
                        config.llm.get_or_insert_with(default_llm_config).temperature = Some(temp);
                    }
                }
                ["LLM", "MAX", "TOKENS"] => {
                    if let Ok(tokens) = value.parse::<u32>() {
                        config.llm.get_or_insert_with(default_llm_config).max_tokens = Some(tokens);
                    }
                }
                ["LLM", "SYSTEM", "PROMPT"] => {
                    config.llm.get_or_insert_with(default_llm_config).system_prompt = Some(value);
                }
                _ => {
                    // Store in node_config
                    config.node_config.insert(key.as_str().to_lowercase(), serde_json::json!(value));
                }
            }
        }
    }

    config
}

fn default_llm_config() -> super::LlmConfig {
    super::LlmConfig {
        provider: String::new(),
        model: String::new(),
        api_key: None,
        base_url: None,
        temperature: None,
        max_tokens: None,
        system_prompt: None,
    }
}
