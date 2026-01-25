//! Integration tests for multi-format configuration support
//!
//! Tests all supported configuration formats with environment variable substitution.

#[cfg(test)]
mod integration_tests {
    use crate::config::*;
    use serde::Deserialize;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    /// Test configuration structure
    #[derive(Debug, Deserialize, PartialEq)]
    struct TestAgentConfig {
        agent: AgentInfo,
        llm: Option<LlmConfig>,
        runtime: Option<RuntimeConfig>,
    }

    #[derive(Debug, Deserialize, PartialEq)]
    struct AgentInfo {
        id: String,
        name: String,
        description: Option<String>,
    }

    #[derive(Debug, Deserialize, PartialEq)]
    struct LlmConfig {
        provider: String,
        model: String,
        api_key: Option<String>,
        temperature: Option<f32>,
    }

    #[derive(Debug, Deserialize, PartialEq)]
    struct RuntimeConfig {
        max_concurrent_tasks: Option<usize>,
        default_timeout_secs: Option<u64>,
    }

    fn create_test_file(dir: &TempDir, filename: &str, content: &str) -> PathBuf {
        let path = dir.path().join(filename);
        fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn test_all_formats_load_basic_config() {
        let temp_dir = TempDir::new().unwrap();

        // YAML
        let yaml = r#"
agent:
  id: test-001
  name: Test Agent
llm:
  provider: openai
  model: gpt-4
  temperature: 0.7
"#;
        let yaml_path = create_test_file(&temp_dir, "agent.yml", yaml);
        let yaml_config: TestAgentConfig = load_config(yaml_path.to_str().unwrap()).unwrap();
        assert_eq!(yaml_config.agent.id, "test-001");
        assert_eq!(yaml_config.agent.name, "Test Agent");

        // TOML
        let toml = r#"
[agent]
id = "test-001"
name = "Test Agent"

[llm]
provider = "openai"
model = "gpt-4"
temperature = 0.7
"#;
        let toml_path = create_test_file(&temp_dir, "agent.toml", toml);
        let toml_config: TestAgentConfig = load_config(toml_path.to_str().unwrap()).unwrap();
        assert_eq!(toml_config.agent.id, "test-001");
        assert_eq!(toml_config.agent.name, "Test Agent");

        // JSON
        let json = r#"{
    "agent": {
        "id": "test-001",
        "name": "Test Agent"
    },
    "llm": {
        "provider": "openai",
        "model": "gpt-4",
        "temperature": 0.7
    }
}"#;
        let json_path = create_test_file(&temp_dir, "agent.json", json);
        let json_config: TestAgentConfig = load_config(json_path.to_str().unwrap()).unwrap();
        assert_eq!(json_config.agent.id, "test-001");
        assert_eq!(json_config.agent.name, "Test Agent");

        // INI (limited support - flat structure only)
        let ini = r#"
[id]
value = "test-001"

[name]
value = "Test Agent"

[model]
value = "gpt-4"
"#;
        let ini_path = create_test_file(&temp_dir, "agent.ini", ini);

        #[derive(Deserialize)]
        struct SimpleConfig {
            id: String,
            name: String,
            model: String,
        }
        let ini_config: SimpleConfig = load_config(ini_path.to_str().unwrap()).unwrap();
        assert_eq!(ini_config.id, "test-001");
        assert_eq!(ini_config.name, "Test Agent");
        assert_eq!(ini_config.model, "gpt-4");

        // RON
        let ron = r#"
(
    agent: (
        id: "test-001",
        name: "Test Agent",
    ),
    llm: Some((
        provider: "openai",
        model: "gpt-4",
    )),
)
"#;
        let ron_path = create_test_file(&temp_dir, "agent.ron", ron);
        let ron_config: TestAgentConfig = load_config(ron_path.to_str().unwrap()).unwrap();
        assert_eq!(ron_config.agent.id, "test-001");
        assert_eq!(ron_config.agent.name, "Test Agent");

        // JSON5
        let json5 = r#"{
    // JSON5 allows comments
    agent: {
        id: "test-001",
        name: "Test Agent",
    },
    llm: {
        provider: "openai",
        model: "gpt-4",
    },
}"#;
        let json5_path = create_test_file(&temp_dir, "agent.json5", json5);
        let json5_config: TestAgentConfig = load_config(json5_path.to_str().unwrap()).unwrap();
        assert_eq!(json5_config.agent.id, "test-001");
        assert_eq!(json5_config.agent.name, "Test Agent");
    }

    #[test]
    fn test_env_var_substitution_braced() {
        let temp_dir = TempDir::new().unwrap();

        unsafe { std::env::set_var("TEST_MODEL", "gpt-4-turbo"); }
        unsafe { std::env::set_var("TEST_KEY", "sk-test-key-123"); }

        // YAML with env vars
        let yaml = r#"
agent:
  id: test-001
  name: Test Agent
llm:
  provider: openai
  model: ${TEST_MODEL}
  api_key: ${TEST_KEY}
"#;
        let yaml_path = create_test_file(&temp_dir, "agent.yml", yaml);
        let yaml_config: TestAgentConfig = load_config(yaml_path.to_str().unwrap()).unwrap();
        assert_eq!(yaml_config.llm.unwrap().model, "gpt-4-turbo");

        // JSON with env vars
        let json = r#"{
    "agent": {
        "id": "test-001"
    },
    "llm": {
        "provider": "openai",
        "model": "${TEST_MODEL}",
        "api_key": "${TEST_KEY}"
    }
}"#;
        let json_path = create_test_file(&temp_dir, "agent.json", json);
        let json_config: TestAgentConfig = load_config(json_path.to_str().unwrap()).unwrap();
        assert_eq!(json_config.llm.unwrap().model, "gpt-4-turbo");

        // TOML with env vars
        let toml = r#"
[agent]
id = "test-001"

[llm]
provider = "openai"
model = "${TEST_MODEL}"
api_key = "${TEST_KEY}"
"#;
        let toml_path = create_test_file(&temp_dir, "agent.toml", toml);
        let toml_config: TestAgentConfig = load_config(toml_path.to_str().unwrap()).unwrap();
        assert_eq!(toml_config.llm.unwrap().model, "gpt-4-turbo");

        unsafe { std::env::remove_var("TEST_MODEL"); }
        unsafe { std::env::remove_var("TEST_KEY"); }
    }

    #[test]
    fn test_env_var_substitution_unbraced() {
        let temp_dir = TempDir::new().unwrap();

        unsafe { std::env::set_var("TEST_PROVIDER", "ollama"); }

        let yaml = r#"
agent:
  id: test-001
llm:
  provider: $TEST_PROVIDER
  model: llama2
"#;
        let yaml_path = create_test_file(&temp_dir, "agent.yml", yaml);
        let yaml_config: TestAgentConfig = load_config(yaml_path.to_str().unwrap()).unwrap();
        assert_eq!(yaml_config.llm.unwrap().provider, "ollama");

        unsafe { std::env::remove_var("TEST_PROVIDER"); }
    }

    #[test]
    fn test_merge_configs_from_multiple_sources() {
        let base = r#"
{
    "agent": {
        "id": "base-001",
        "name": "Base Agent"
    },
    "llm": {
        "provider": "openai",
        "model": "gpt-3.5-turbo"
    }
}
"#;

        let override_config = r#"
{
    "llm": {
        "model": "gpt-4"
    },
    "runtime": {
        "max_concurrent_tasks": 20
    }
}
"#;

        let merged: TestAgentConfig = merge_configs(&[
            (base, FileFormat::Json),
            (override_config, FileFormat::Json),
        ]).unwrap();

        // Should have base values with override applied
        assert_eq!(merged.agent.id, "base-001");
        assert_eq!(merged.llm.unwrap().model, "gpt-4");
        assert_eq!(merged.runtime.unwrap().max_concurrent_tasks.unwrap(), 20);
    }

    #[test]
    fn test_load_merged_from_files() {
        let temp_dir = TempDir::new().unwrap();

        // Base config
        let base = r#"
agent:
  id: base-001
  name: Base Agent
llm:
  provider: openai
  model: gpt-3.5-turbo
"#;
        let base_path = create_test_file(&temp_dir, "base.yml", base);

        // Override config
        let override_config = r#"
llm:
  model: gpt-4
runtime:
  max_concurrent_tasks: 20
"#;
        let override_path = create_test_file(&temp_dir, "override.yml", override_config);

        let merged: TestAgentConfig = load_merged(&[
            base_path.to_str().unwrap(),
            override_path.to_str().unwrap(),
        ]).unwrap();

        assert_eq!(merged.agent.id, "base-001");
        assert_eq!(merged.llm.unwrap().model, "gpt-4");
        assert_eq!(merged.runtime.unwrap().max_concurrent_tasks.unwrap(), 20);
    }

    #[test]
    fn test_env_var_with_env_override() {
        let temp_dir = TempDir::new().unwrap();

        unsafe { std::env::set_var("MYAPP_LLM__MODEL", "gpt-4-from-env"); }

        let yaml = r#"
agent:
  id: test-001
llm:
  model: gpt-3.5-turbo
"#;
        let yaml_path = create_test_file(&temp_dir, "agent.yml", yaml);

        let config: TestAgentConfig = load_with_env(
            yaml_path.to_str().unwrap(),
            "MYAPP"
        ).unwrap();

        assert_eq!(config.llm.unwrap().model, "gpt-4-from-env");

        unsafe { std::env::remove_var("MYAPP_LLM__MODEL"); }
    }

    #[test]
    fn test_missing_env_var_preserved() {
        let result = substitute_env_vars("url: ${MISSING_VAR}");
        assert_eq!(result, "url: ${MISSING_VAR}");

        let result = substitute_env_vars("url: $ANOTHER_MISSING");
        assert_eq!(result, "url: $ANOTHER_MISSING");
    }

    #[test]
    fn test_partial_env_var_substitution() {
        unsafe { std::env::set_var("HOST", "localhost"); }
        unsafe { std::env::set_var("PORT", "8080"); }

        let result = substitute_env_vars("url: http://${HOST}:${PORT}/api");
        assert_eq!(result, "url: http://localhost:8080/api");

        unsafe { std::env::remove_var("HOST"); }
        unsafe { std::env::remove_var("PORT"); }
    }

    #[test]
    fn test_detect_format_from_extension() {
        assert_eq!(detect_format("config.yaml").unwrap(), FileFormat::Yaml);
        assert_eq!(detect_format("config.yml").unwrap(), FileFormat::Yaml);
        assert_eq!(detect_format("config.toml").unwrap(), FileFormat::Toml);
        assert_eq!(detect_format("config.json").unwrap(), FileFormat::Json);
        assert_eq!(detect_format("config.ini").unwrap(), FileFormat::Ini);
        assert_eq!(detect_format("config.ron").unwrap(), FileFormat::Ron);
        assert_eq!(detect_format("config.json5").unwrap(), FileFormat::Json5);

        assert!(detect_format("config.txt").is_err());
        assert!(detect_format("config.unknown").is_err());
    }

    #[test]
    fn test_complex_nested_config() {
        let yaml = r#"
agent:
  id: complex-001
  name: Complex Agent
  description: |
    A multi-line
    description
llm:
  provider: openai
  model: gpt-4
  api_key: ${OPENAI_API_KEY}
  temperature: 0.7
  max_tokens: 4096
  extra:
    top_p: 0.9
    frequency_penalty: 0.0
runtime:
  max_concurrent_tasks: 10
  default_timeout_secs: 30
  extra:
    enable_cache: true
    cache_ttl: 3600
"#;

        let config: TestAgentConfig = from_str(yaml, FileFormat::Yaml).unwrap();
        assert_eq!(config.agent.id, "complex-001");
        assert_eq!(config.llm.as_ref().unwrap().temperature.unwrap(), 0.7);
        assert_eq!(config.runtime.as_ref().unwrap().max_concurrent_tasks.unwrap(), 10);
    }

    #[test]
    fn test_array_config() {
        let json = r#"{
    "agent": {
        "id": "array-test",
        "capabilities": ["llm", "tools", "memory", "streaming"]
    }
}"#;

        #[derive(Debug, Deserialize)]
        struct ArrayTestConfig {
            agent: AgentWithArray,
        }

        #[derive(Debug, Deserialize)]
        struct AgentWithArray {
            id: String,
            capabilities: Vec<String>,
        }

        let config: ArrayTestConfig = from_str(json, FileFormat::Json).unwrap();
        assert_eq!(config.agent.capabilities.len(), 4);
        assert!(config.agent.capabilities.contains(&"streaming".to_string()));
    }

    #[test]
    fn test_special_characters_in_values() {
        let yaml = r#"
agent:
  id: "agent-with-special-chars"
  description: "A test with special chars: @#$%^&*()_+-=[]{}|;':\",./<>?"
llm:
  system_prompt: |
    You are a helpful assistant.
    Remember: "Always be helpful!"
"#;

        let config: TestAgentConfig = from_str(yaml, FileFormat::Yaml).unwrap();
        assert_eq!(config.agent.id, "agent-with-special-chars");
        assert!(config.agent.description.unwrap().contains("@#$%"));
    }
}
