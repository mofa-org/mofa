use mofa_testing::agent_runner::AgentTestRunner;

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
