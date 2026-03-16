//! Tests for MockTool accessor helpers: last_call(), nth_call()

use mofa_foundation::agent::components::tool::SimpleTool;
use mofa_kernel::agent::components::tool::ToolInput;
use mofa_testing::tools::MockTool;
use serde_json::json;

#[tokio::test]
async fn last_call_returns_none_when_never_called() {
    let tool = MockTool::new("test", "test", json!({}));
    assert!(tool.last_call().await.is_none());
}

#[tokio::test]
async fn last_call_returns_most_recent_call() {
    let tool = MockTool::new("test", "test", json!({}));
    tool.execute(ToolInput::from_json(json!({"id": 1}))).await;
    tool.execute(ToolInput::from_json(json!({"id": 2}))).await;

    let last = tool.last_call().await.unwrap();
    assert_eq!(last.arguments, json!({"id": 2}));
}

#[tokio::test]
async fn nth_call_returns_first_call() {
    let tool = MockTool::new("test", "test", json!({}));
    tool.execute(ToolInput::from_json(json!({"id": 1}))).await;
    tool.execute(ToolInput::from_json(json!({"id": 2}))).await;

    let first = tool.nth_call(0).await.unwrap();
    assert_eq!(first.arguments, json!({"id": 1}));
}

#[tokio::test]
async fn nth_call_returns_none_for_out_of_bounds() {
    let tool = MockTool::new("test", "test", json!({}));
    tool.execute(ToolInput::from_json(json!({"id": 1}))).await;
    assert!(tool.nth_call(1).await.is_none());
}

#[tokio::test]
async fn nth_call_works_after_interleaved_failures() {
    let tool = MockTool::new("test", "test", json!({}));
    tool.fail_next(1, "error").await;

    // Call 0 (fails)
    let res1 = tool.execute(ToolInput::from_json(json!({"id": 1}))).await;
    assert!(!res1.success);

    // Call 1 (succeeds)
    let res2 = tool.execute(ToolInput::from_json(json!({"id": 2}))).await;
    assert!(res2.success);

    let first = tool.nth_call(0).await.unwrap();
    let second = tool.nth_call(1).await.unwrap();

    assert_eq!(first.arguments, json!({"id": 1}));
    assert_eq!(second.arguments, json!({"id": 2}));
}
