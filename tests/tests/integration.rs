use mofa_testing::backend::MockLLMBackend;
use mofa_testing::bus::MockAgentBus;
use mofa_testing::tools::MockTool;
use mofa_foundation::orchestrator::ModelOrchestrator;
use mofa_foundation::agent::components::tool::SimpleTool;
use mofa_kernel::agent::components::tool::ToolInput;
use serde_json::json;
use futures::stream::StreamExt;

#[tokio::test]
async fn test_mock_llm_backend() {
    let mut backend = MockLLMBackend::new();
    
    // Add specific mock responses for certain prompts
    backend.add_mock_response("hello", "Hi there! How can I help you?");
    backend.set_fallback_response("I don't understand that prompt.");

    // Retrieve generation and test it
    let mut stream = backend.generate("mock-id", "User says: hello").unwrap();
    let mut response_str = String::new();
    while let Some(chunk) = stream.next().await {
        response_str.push_str(&chunk.unwrap());
    }

    // Note: because we mapped with split_whitespace() and space appended:
    assert_eq!(response_str.trim(), "Hi there! How can I help you?");

    // Test fallback
    let mut stream2 = backend.generate("mock-id", "unknown prompt").unwrap();
    let mut response_str2 = String::new();
    while let Some(chunk) = stream2.next().await {
        response_str2.push_str(&chunk.unwrap());
    }
    assert_eq!(response_str2.trim(), "I don't understand that prompt.");
}

#[tokio::test]
async fn test_mock_tool() {
    let mock_tool = MockTool::new(
        "calculator", 
        "Adds two numbers", 
        json!({"type": "object"})
    );

    assert_eq!(mock_tool.call_count().await, 0);

    // Provide some input
    let _ = mock_tool.execute(ToolInput::from_json(json!({"a": 1, "b": 2}))).await;
    
    // Assert using standard methods
    assert_eq!(mock_tool.call_count().await, 1);
    let history = mock_tool.history().await;
    assert_eq!(history.len(), 1);
    
    // Assert using our macro
    mofa_testing::assert_tool_called!(mock_tool, 1);
}
