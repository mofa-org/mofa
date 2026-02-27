use mofa_testing::tools::MockTool;
use mofa_kernel::agent::components::tool::ToolInput;
use mofa_foundation::agent::components::tool::SimpleTool;
use serde_json::json;

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
