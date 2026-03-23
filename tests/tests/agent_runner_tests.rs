use mofa_testing::agent_runner::AgentTestRunner;
use mofa_testing::tools::MockTool;
use serde_json::json;

#[tokio::test]
async fn agent_runner_executes_and_captures_output() {
    let mut runner = AgentTestRunner::new().await.expect("runner initializes");
    runner
        .mock_llm()
        .add_response("Mocked response")
        .await;

    let result = runner
        .run_text("hello")
        .await
        .expect("run should succeed");

    assert!(result.is_success());
    assert_eq!(result.output_text().as_deref(), Some("Mocked response"));
    assert_eq!(
        result.metadata.session_id.as_deref(),
        Some(runner.session_id())
    );
    assert_eq!(result.metadata.execution_id, runner.execution_id());
    assert_eq!(result.metadata.runner_stats_before.total_executions, 0);
    assert_eq!(result.metadata.runner_stats_after.total_executions, 1);
    assert!(result.metadata.session_snapshot.is_some());
    let snapshot = result.metadata.session_snapshot.as_ref().unwrap();
    assert_eq!(snapshot.len(), 2);

    runner.shutdown().await.expect("shutdown succeeds");
}

#[tokio::test]
async fn agent_runner_creates_isolated_workspaces() {
    let mut runner_a = AgentTestRunner::new().await.expect("runner A initializes");
    let mut runner_b = AgentTestRunner::new().await.expect("runner B initializes");

    assert_ne!(runner_a.workspace(), runner_b.workspace());

    runner_a
        .mock_llm()
        .add_response("Response A")
        .await;
    runner_b
        .mock_llm()
        .add_response("Response B")
        .await;

    let _ = runner_a
        .run_text("hi")
        .await
        .expect("runner A executes");
    let _ = runner_b
        .run_text("hi")
        .await
        .expect("runner B executes");

    let session_a = runner_a
        .workspace()
        .join("sessions")
        .join(format!("{}.jsonl", runner_a.session_id()));
    let session_b = runner_b
        .workspace()
        .join("sessions")
        .join(format!("{}.jsonl", runner_b.session_id()));

    assert!(session_a.exists());
    assert!(session_b.exists());

    runner_a.shutdown().await.expect("shutdown A");
    runner_b.shutdown().await.expect("shutdown B");
}

#[tokio::test]
async fn agent_runner_executes_tool_calls() {
    let mut runner = AgentTestRunner::new().await.expect("runner initializes");

    let tool = MockTool::new(
        "echo_tool",
        "Echo the provided input",
        json!({
            "type": "object",
            "properties": {
                "input": { "type": "string" }
            },
            "required": ["input"]
        }),
    );

    runner
        .register_simple_tool(tool.clone())
        .await
        .expect("tool registered");

    runner
        .mock_llm()
        .add_tool_call_response("echo_tool", json!({ "input": "ping" }), None)
        .await;
    runner
        .mock_llm()
        .add_response("Final response")
        .await;

    let result = runner
        .run_text("use tool")
        .await
        .expect("run should succeed");

    assert_eq!(result.output_text().as_deref(), Some("Final response"));
    assert_eq!(tool.call_count().await, 1);
    let last_call = tool.last_call().await.expect("tool call captured");
    assert_eq!(last_call.arguments, json!({ "input": "ping" }));

    runner.shutdown().await.expect("shutdown succeeds");
}

#[tokio::test]
async fn agent_runner_loads_bootstrap_files() {
    let mut runner = AgentTestRunner::new().await.expect("runner initializes");
    runner
        .write_bootstrap_file("AGENTS.md", "Bootstrap content for agent test.")
        .expect("bootstrap file written");

    runner
        .mock_llm()
        .add_response("Bootstrapped response")
        .await;

    let _ = runner
        .run_text("check prompt")
        .await
        .expect("run should succeed");

    let request = runner
        .mock_llm()
        .last_request()
        .await
        .expect("request captured");
    let system_message = request
        .messages
        .first()
        .and_then(|msg| msg.content.as_deref())
        .expect("system message content");

    assert!(system_message.contains("AGENTS.md"));
    assert!(system_message.contains("Bootstrap content for agent test."));

    runner.shutdown().await.expect("shutdown succeeds");
}

#[tokio::test]
async fn agent_runner_captures_llm_failure() {
    let mut runner = AgentTestRunner::new().await.expect("runner initializes");
    runner
        .mock_llm()
        .add_error_response("mock failure")
        .await;

    let result = runner
        .run_text("trigger failure")
        .await
        .expect("run should return result");

    assert!(!result.is_success());
    let error = result.error.expect("error captured");
    assert!(error.to_string().contains("mock failure"));
    assert_eq!(result.metadata.runner_stats_after.failed_executions, 1);

    runner.shutdown().await.expect("shutdown succeeds");
}
