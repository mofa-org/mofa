//! Integration tests for the mofa-testing framework.

use mofa_foundation::agent::components::tool::SimpleTool;
use mofa_foundation::orchestrator::{ModelOrchestrator, ModelProviderConfig, ModelType};
use mofa_kernel::agent::components::tool::ToolInput;
use mofa_testing::backend::MockLLMBackend;
use mofa_testing::bus::MockAgentBus;
use mofa_testing::tools::MockTool;
use serde_json::json;
use std::collections::HashMap;

// ===================================================================
// MockLLMBackend tests
// ===================================================================

#[tokio::test]
async fn backend_infer_returns_matching_response() {
    let backend = MockLLMBackend::new();
    backend.add_response("hello", "Hi there!");
    backend.add_response("weather", "Sunny today.");

    // Register + load a dummy model so infer() has context
    let config = ModelProviderConfig {
        model_name: "test-llm".into(),
        model_path: "/mock".into(),
        device: "cpu".into(),
        model_type: ModelType::Llm,
        max_context_length: None,
        quantization: None,
        extra_config: HashMap::new(),
    };
    backend.register_model(config).await.unwrap();
    backend.load_model("test-llm").await.unwrap();

    let resp = backend.infer("test-llm", "say hello").await.unwrap();
    assert_eq!(resp, "Hi there!");

    let resp = backend
        .infer("test-llm", "what is the weather?")
        .await
        .unwrap();
    assert_eq!(resp, "Sunny today.");
}

#[tokio::test]
async fn backend_infer_returns_fallback_on_no_match() {
    let mut backend = MockLLMBackend::new();
    backend.set_fallback("I don't understand.");

    let resp = backend.infer("any", "random input").await.unwrap();
    assert_eq!(resp, "I don't understand.");
}

#[tokio::test]
async fn backend_first_match_wins() {
    let backend = MockLLMBackend::new();
    // Both keys appear in the prompt; "hello" is added first → wins
    backend.add_response("hello", "first");
    backend.add_response("world", "second");

    let resp = backend.infer("x", "hello world").await.unwrap();
    assert_eq!(resp, "first", "first-match semantics must hold");
}

#[tokio::test]
async fn backend_register_load_unload_lifecycle() {
    let backend = MockLLMBackend::new();

    let config = ModelProviderConfig {
        model_name: "model-a".into(),
        model_path: "/mock".into(),
        device: "cpu".into(),
        model_type: ModelType::Llm,
        max_context_length: None,
        quantization: None,
        extra_config: HashMap::new(),
    };

    backend.register_model(config).await.unwrap();
    assert!(backend.list_models().contains(&"model-a".to_string()));
    assert!(!backend.is_model_loaded("model-a"));

    backend.load_model("model-a").await.unwrap();
    assert!(backend.is_model_loaded("model-a"));
    assert!(
        backend
            .list_loaded_models()
            .contains(&"model-a".to_string())
    );

    backend.unload_model("model-a").await.unwrap();
    assert!(!backend.is_model_loaded("model-a"));

    backend.unregister_model("model-a").await.unwrap();
    assert!(!backend.list_models().contains(&"model-a".to_string()));
}

#[tokio::test]
async fn backend_load_unregistered_model_returns_error() {
    let backend = MockLLMBackend::new();
    let result = backend.load_model("nonexistent").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn backend_statistics_reflect_state() {
    let backend = MockLLMBackend::new();
    let stats = backend.get_statistics().unwrap();
    assert_eq!(stats.loaded_models_count, 0);

    let config = ModelProviderConfig {
        model_name: "m".into(),
        model_path: "/m".into(),
        device: "cpu".into(),
        model_type: ModelType::Llm,
        max_context_length: None,
        quantization: None,
        extra_config: HashMap::new(),
    };
    backend.register_model(config).await.unwrap();
    backend.load_model("m").await.unwrap();

    let stats = backend.get_statistics().unwrap();
    assert_eq!(stats.loaded_models_count, 1);
}

#[tokio::test]
async fn backend_eviction_clears_loaded() {
    let backend = MockLLMBackend::new();

    let config = ModelProviderConfig {
        model_name: "m".into(),
        model_path: "/m".into(),
        device: "cpu".into(),
        model_type: ModelType::Llm,
        max_context_length: None,
        quantization: None,
        extra_config: HashMap::new(),
    };
    backend.register_model(config).await.unwrap();
    backend.load_model("m").await.unwrap();

    let evicted = backend.trigger_eviction(0).await.unwrap();
    assert_eq!(evicted, 1);
    assert!(!backend.is_model_loaded("m"));
}

#[tokio::test]
async fn backend_memory_threshold_roundtrip() {
    let backend = MockLLMBackend::new();
    backend.set_memory_threshold(1024).await.unwrap();
    assert_eq!(backend.get_memory_threshold(), 1024);
}

#[tokio::test]
async fn backend_idle_timeout_roundtrip() {
    let backend = MockLLMBackend::new();
    backend.set_idle_timeout_secs(60).await.unwrap();
    assert_eq!(backend.get_idle_timeout_secs(), 60);
}

// ===================================================================
// MockAgentBus tests
// ===================================================================

#[tokio::test]
async fn bus_captures_messages() {
    let bus = MockAgentBus::new();
    assert_eq!(bus.message_count().await, 0);

    // send_and_capture records even though routing might fail
    // (no receiver registered); we only care about capture here
    let msg = mofa_kernel::message::AgentMessage::TaskRequest {
        task_id: "test-1".into(),
        content: "ping".into(),
    };
    let mode = mofa_kernel::bus::CommunicationMode::Broadcast;
    let _ = bus.send_and_capture("agent-1", mode, msg).await;

    assert_eq!(bus.message_count().await, 1);
}

#[tokio::test]
async fn bus_clear_history_resets() {
    let bus = MockAgentBus::new();
    let msg = mofa_kernel::message::AgentMessage::TaskRequest {
        task_id: "test-1".into(),
        content: "ping".into(),
    };
    let mode = mofa_kernel::bus::CommunicationMode::Broadcast;
    let _ = bus.send_and_capture("a", mode, msg).await;
    assert_eq!(bus.message_count().await, 1);

    bus.clear_history().await;
    assert_eq!(bus.message_count().await, 0);
}

// ===================================================================
// MockTool tests
// ===================================================================

#[tokio::test]
async fn tool_starts_with_zero_calls() {
    let tool = MockTool::new("calc", "Adds numbers", json!({"type": "object"}));
    assert_eq!(tool.call_count().await, 0);
    assert!(tool.history().await.is_empty());
}

#[tokio::test]
async fn tool_records_calls_and_returns_stub() {
    let tool = MockTool::new("calc", "Adds numbers", json!({"type": "object"}));

    let result = tool
        .execute(ToolInput::from_json(json!({"a": 1, "b": 2})))
        .await;
    assert!(result.success);

    assert_eq!(tool.call_count().await, 1);

    let history = tool.history().await;
    assert_eq!(history.len(), 1);
}

#[tokio::test]
async fn tool_custom_stub_result() {
    let tool = MockTool::new("fail", "Always fails", json!({"type": "object"}));
    tool.set_result(mofa_kernel::agent::components::tool::ToolResult::failure(
        "simulated error",
    ))
    .await;

    let result = tool.execute(ToolInput::from_json(json!({"x": 1}))).await;
    assert!(!result.success);
}

#[tokio::test]
async fn tool_assert_macro_passes() {
    let tool = MockTool::new("m", "desc", json!({"type": "object"}));
    tool.execute(ToolInput::from_json(json!({}))).await;
    tool.execute(ToolInput::from_json(json!({}))).await;

    mofa_testing::assert_tool_called!(tool, 2);
}
