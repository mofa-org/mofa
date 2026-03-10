//! Tests for assertion macros: assert_tool_called!, assert_tool_called_with!,
//! assert_infer_called!, assert_bus_message_sent!.

use mofa_foundation::agent::components::tool::SimpleTool;
use mofa_foundation::orchestrator::{ModelOrchestrator, ModelProviderConfig, ModelType};
use mofa_kernel::agent::components::tool::ToolInput;
use mofa_kernel::bus::CommunicationMode;
use mofa_kernel::message::AgentMessage;
use mofa_testing::backend::MockLLMBackend;
use mofa_testing::bus::MockAgentBus;
use mofa_testing::tools::MockTool;
use serde_json::json;
use std::collections::HashMap;

fn make_config(name: &str) -> ModelProviderConfig {
    ModelProviderConfig {
        model_name: name.into(),
        model_path: "/mock".into(),
        device: "cpu".into(),
        model_type: ModelType::Llm,
        max_context_length: None,
        quantization: None,
        extra_config: HashMap::new(),
    }
}

// ===================================================================
// assert_tool_called!
// ===================================================================

#[tokio::test]
async fn assert_tool_called_passes_when_tool_was_called() {
    let tool = MockTool::new("search", "Search tool", json!({"type": "object"}));
    tool.execute(ToolInput::from_json(json!({"q": "rust"}))).await;

    mofa_testing::assert_tool_called!(tool, "search");
}

#[tokio::test]
#[should_panic(expected = "to be called at least once")]
async fn assert_tool_called_panics_when_tool_never_called() {
    let tool = MockTool::new("search", "Search tool", json!({"type": "object"}));
    mofa_testing::assert_tool_called!(tool, "search");
}

// ===================================================================
// assert_tool_called_with!
// ===================================================================

#[tokio::test]
async fn assert_tool_called_with_passes_on_matching_arguments() {
    let tool = MockTool::new("search", "Search tool", json!({"type": "object"}));
    tool.execute(ToolInput::from_json(json!({"query": "rust"}))).await;

    mofa_testing::assert_tool_called_with!(tool, json!({"query": "rust"}));
}

#[tokio::test]
#[should_panic(expected = "no matching call found")]
async fn assert_tool_called_with_panics_on_mismatched_arguments() {
    let tool = MockTool::new("search", "Search tool", json!({"type": "object"}));
    tool.execute(ToolInput::from_json(json!({"query": "python"}))).await;

    mofa_testing::assert_tool_called_with!(tool, json!({"query": "rust"}));
}

// ===================================================================
// assert_infer_called!
// ===================================================================

#[tokio::test]
async fn assert_infer_called_passes_after_infer_call() {
    let backend = MockLLMBackend::new();
    backend.add_response("hi", "hello");
    backend.register_model(make_config("m")).await.unwrap();
    backend.load_model("m").await.unwrap();
    backend.infer("m", "hi").await.unwrap();

    mofa_testing::assert_infer_called!(backend);
}

#[tokio::test]
#[should_panic(expected = "call_count is 0")]
async fn assert_infer_called_panics_when_call_count_is_zero() {
    let backend = MockLLMBackend::new();
    mofa_testing::assert_infer_called!(backend);
}

// ===================================================================
// assert_bus_message_sent!
// ===================================================================

#[tokio::test]
async fn assert_bus_message_sent_passes_when_sender_matches() {
    let bus = MockAgentBus::new();
    let msg = AgentMessage::TaskRequest {
        task_id: "t-1".into(),
        content: "ping".into(),
    };
    let _ = bus
        .send_and_capture("agent-1", CommunicationMode::Broadcast, msg)
        .await;

    mofa_testing::assert_bus_message_sent!(bus, "agent-1");
}

#[tokio::test]
#[should_panic(expected = "Expected a message from sender")]
async fn assert_bus_message_sent_panics_when_no_message_from_sender() {
    let bus = MockAgentBus::new();
    let msg = AgentMessage::TaskRequest {
        task_id: "t-1".into(),
        content: "ping".into(),
    };
    let _ = bus
        .send_and_capture("agent-1", CommunicationMode::Broadcast, msg)
        .await;

    mofa_testing::assert_bus_message_sent!(bus, "agent-2");
}
