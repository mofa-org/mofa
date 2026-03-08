//! Tests for failure injection, sequenced responses, rate limiting, and MockClock.

use mofa_foundation::agent::components::tool::SimpleTool;
use mofa_foundation::orchestrator::{
    ModelOrchestrator, ModelProviderConfig, ModelType, OrchestratorError,
};
use mofa_kernel::agent::components::tool::{ToolInput, ToolResult};
use mofa_kernel::bus::CommunicationMode;
use mofa_kernel::message::AgentMessage;
use mofa_testing::backend::MockLLMBackend;
use mofa_testing::bus::MockAgentBus;
use mofa_testing::clock::{Clock, MockClock};
use mofa_testing::tools::MockTool;
use serde_json::json;
use std::collections::HashMap;
use std::time::Duration;

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
// MockLLMBackend — failure queue
// ===================================================================

#[tokio::test]
async fn backend_fail_next_drains_fifo() {
    let backend = MockLLMBackend::new();
    backend.add_response("hi", "hello");
    backend.register_model(make_config("m")).await.unwrap();
    backend.load_model("m").await.unwrap();

    backend.fail_next(2, OrchestratorError::InferenceFailed("boom".into()));

    let r1 = backend.infer("m", "hi").await;
    assert!(r1.is_err());
    let r2 = backend.infer("m", "hi").await;
    assert!(r2.is_err());
    // Third call succeeds — queue drained
    let r3 = backend.infer("m", "hi").await;
    assert_eq!(r3.unwrap(), "hello");
}

#[tokio::test]
async fn backend_fail_next_zero_is_noop() {
    let backend = MockLLMBackend::new();
    backend.add_response("x", "ok");
    backend.register_model(make_config("m")).await.unwrap();
    backend.load_model("m").await.unwrap();

    backend.fail_next(0, OrchestratorError::InferenceFailed("unused".into()));
    assert_eq!(backend.infer("m", "x").await.unwrap(), "ok");
}

// ===================================================================
// MockLLMBackend — pattern-based failures
// ===================================================================

#[tokio::test]
async fn backend_fail_on_pattern_match() {
    let backend = MockLLMBackend::new();
    backend.add_response("safe", "ok");
    backend.register_model(make_config("m")).await.unwrap();
    backend.load_model("m").await.unwrap();

    backend.fail_on("danger", OrchestratorError::Other("blocked".into()));

    let err = backend.infer("m", "this is danger zone").await;
    assert!(err.is_err());

    let ok = backend.infer("m", "safe input").await;
    assert_eq!(ok.unwrap(), "ok");
}

#[tokio::test]
async fn backend_fail_on_does_not_match_unrelated() {
    let backend = MockLLMBackend::new();
    backend.register_model(make_config("m")).await.unwrap();
    backend.load_model("m").await.unwrap();

    backend.fail_on("specific", OrchestratorError::Other("nope".into()));

    let result = backend.infer("m", "something else").await;
    assert!(result.is_ok());
}

// ===================================================================
// MockLLMBackend — failure queue drains before pattern check
// ===================================================================

#[tokio::test]
async fn backend_fail_next_takes_priority_over_fail_on() {
    let backend = MockLLMBackend::new();
    backend.register_model(make_config("m")).await.unwrap();
    backend.load_model("m").await.unwrap();

    backend.fail_on("anything", OrchestratorError::Other("pattern".into()));
    backend.fail_next(1, OrchestratorError::InferenceFailed("queue".into()));

    let err = backend.infer("m", "anything").await.unwrap_err();
    match err {
        OrchestratorError::InferenceFailed(msg) => assert_eq!(msg, "queue"),
        other => panic!("expected InferenceFailed, got: {other}"),
    }
}

// ===================================================================
// MockLLMBackend — response sequences
// ===================================================================

#[tokio::test]
async fn backend_response_sequence_returns_in_order() {
    let backend = MockLLMBackend::new();
    backend.register_model(make_config("m")).await.unwrap();
    backend.load_model("m").await.unwrap();

    backend.add_response_sequence("greet", vec!["Hi!", "Hello again!", "Still here."]);

    assert_eq!(backend.infer("m", "greet me").await.unwrap(), "Hi!");
    assert_eq!(
        backend.infer("m", "greet me").await.unwrap(),
        "Hello again!"
    );
    assert_eq!(backend.infer("m", "greet me").await.unwrap(), "Still here.");
    // Last value repeats
    assert_eq!(backend.infer("m", "greet me").await.unwrap(), "Still here.");
}

#[tokio::test]
async fn backend_sequence_takes_priority_over_static_rule() {
    let backend = MockLLMBackend::new();
    backend.register_model(make_config("m")).await.unwrap();
    backend.load_model("m").await.unwrap();

    backend.add_response("hello", "static");
    backend.add_response_sequence("hello", vec!["seq1", "seq2"]);

    assert_eq!(backend.infer("m", "hello").await.unwrap(), "seq1");
    assert_eq!(backend.infer("m", "hello").await.unwrap(), "seq2");
    // After sequence exhausted (last repeats)
    assert_eq!(backend.infer("m", "hello").await.unwrap(), "seq2");
}

#[tokio::test]
async fn backend_unmatched_sequence_falls_through() {
    let backend = MockLLMBackend::new();
    backend.register_model(make_config("m")).await.unwrap();
    backend.load_model("m").await.unwrap();

    backend.add_response_sequence("greet", vec!["Hi!"]);
    backend.add_response("other", "static");

    assert_eq!(backend.infer("m", "other topic").await.unwrap(), "static");
}

// ===================================================================
// MockLLMBackend — rate limiting
// ===================================================================

#[tokio::test]
async fn backend_rate_limit_blocks_after_max_calls() {
    let backend = MockLLMBackend::new();
    backend.register_model(make_config("m")).await.unwrap();
    backend.load_model("m").await.unwrap();

    backend.set_rate_limit(3);

    assert!(backend.infer("m", "1").await.is_ok());
    assert!(backend.infer("m", "2").await.is_ok());
    assert!(backend.infer("m", "3").await.is_ok());
    // 4th call exceeds limit
    let err = backend.infer("m", "4").await;
    assert!(err.is_err());
}

#[tokio::test]
async fn backend_rate_limit_reset_allows_more_calls() {
    let backend = MockLLMBackend::new();
    backend.register_model(make_config("m")).await.unwrap();
    backend.load_model("m").await.unwrap();

    backend.set_rate_limit(1);
    assert!(backend.infer("m", "a").await.is_ok());
    assert!(backend.infer("m", "b").await.is_err());

    backend.reset_rate_limit();
    assert!(backend.infer("m", "c").await.is_ok());
}

// ===================================================================
// MockLLMBackend — call counter
// ===================================================================

#[tokio::test]
async fn backend_call_count_tracks_all_invocations() {
    let backend = MockLLMBackend::new();
    backend.register_model(make_config("m")).await.unwrap();
    backend.load_model("m").await.unwrap();

    assert_eq!(backend.call_count(), 0);
    let _ = backend.infer("m", "a").await;
    let _ = backend.infer("m", "b").await;
    assert_eq!(backend.call_count(), 2);
}

#[tokio::test]
async fn backend_call_count_includes_failed_calls() {
    let backend = MockLLMBackend::new();
    backend.register_model(make_config("m")).await.unwrap();
    backend.load_model("m").await.unwrap();

    backend.fail_next(1, OrchestratorError::InferenceFailed("err".into()));
    let _ = backend.infer("m", "a").await;

    assert_eq!(backend.call_count(), 1);
}

#[tokio::test]
async fn backend_call_count_reset() {
    let backend = MockLLMBackend::new();
    backend.register_model(make_config("m")).await.unwrap();
    backend.load_model("m").await.unwrap();

    let _ = backend.infer("m", "a").await;
    assert_eq!(backend.call_count(), 1);

    backend.reset_call_count();
    assert_eq!(backend.call_count(), 0);
}

// ===================================================================
// MockTool — failure queue
// ===================================================================

#[tokio::test]
async fn tool_fail_next_returns_failures_then_succeeds() {
    let tool = MockTool::new("t", "desc", json!({"type": "object"}));
    tool.fail_next(2, "transient error").await;

    let r1 = tool.execute(ToolInput::from_json(json!({}))).await;
    assert!(!r1.success);
    assert_eq!(r1.error.as_deref(), Some("transient error"));

    let r2 = tool.execute(ToolInput::from_json(json!({}))).await;
    assert!(!r2.success);

    // Third call succeeds — queue drained
    let r3 = tool.execute(ToolInput::from_json(json!({}))).await;
    assert!(r3.success);
}

#[tokio::test]
async fn tool_fail_next_zero_is_noop() {
    let tool = MockTool::new("t", "desc", json!({"type": "object"}));
    tool.fail_next(0, "unused").await;

    let result = tool.execute(ToolInput::from_json(json!({}))).await;
    assert!(result.success);
}

// ===================================================================
// MockTool — input-pattern failures
// ===================================================================

#[tokio::test]
async fn tool_fail_on_input_matches_exact_json() {
    let tool = MockTool::new("t", "desc", json!({"type": "object"}));
    tool.fail_on_input(json!({"action": "delete"}), "forbidden")
        .await;

    let bad = tool
        .execute(ToolInput::from_json(json!({"action": "delete"})))
        .await;
    assert!(!bad.success);
    assert_eq!(bad.error.as_deref(), Some("forbidden"));

    let good = tool
        .execute(ToolInput::from_json(json!({"action": "read"})))
        .await;
    assert!(good.success);
}

#[tokio::test]
async fn tool_fail_on_input_records_call_even_on_failure() {
    let tool = MockTool::new("t", "desc", json!({"type": "object"}));
    tool.fail_on_input(json!({"x": 1}), "boom").await;

    tool.execute(ToolInput::from_json(json!({"x": 1}))).await;
    assert_eq!(tool.call_count().await, 1);
}

// ===================================================================
// MockTool — result sequences
// ===================================================================

#[tokio::test]
async fn tool_result_sequence_drains_then_falls_back() {
    let tool = MockTool::new("t", "desc", json!({"type": "object"}));
    tool.add_result_sequence(vec![
        ToolResult::success_text("first"),
        ToolResult::failure("transient"),
        ToolResult::success_text("recovered"),
    ])
    .await;

    let r1 = tool.execute(ToolInput::from_json(json!({}))).await;
    assert!(r1.success);
    assert_eq!(r1.output, json!("first"));

    let r2 = tool.execute(ToolInput::from_json(json!({}))).await;
    assert!(!r2.success);

    let r3 = tool.execute(ToolInput::from_json(json!({}))).await;
    assert!(r3.success);
    assert_eq!(r3.output, json!("recovered"));

    // Sequence exhausted — falls back to stubbed_result
    let r4 = tool.execute(ToolInput::from_json(json!({}))).await;
    assert!(r4.success);
    assert_eq!(r4.output, json!("Mock execution default"));
}

#[tokio::test]
async fn tool_fail_next_takes_priority_over_sequence() {
    let tool = MockTool::new("t", "desc", json!({"type": "object"}));
    tool.add_result_sequence(vec![ToolResult::success_text("seq")])
        .await;
    tool.fail_next(1, "queue wins").await;

    let r = tool.execute(ToolInput::from_json(json!({}))).await;
    assert!(!r.success);
    assert_eq!(r.error.as_deref(), Some("queue wins"));

    // Now sequence kicks in
    let r2 = tool.execute(ToolInput::from_json(json!({}))).await;
    assert_eq!(r2.output, json!("seq"));
}

// ===================================================================
// MockTool — call history preserved across all modes
// ===================================================================

#[tokio::test]
async fn tool_call_history_includes_all_invocations() {
    let tool = MockTool::new("t", "desc", json!({"type": "object"}));
    tool.fail_next(1, "err").await;

    tool.execute(ToolInput::from_json(json!({"a": 1}))).await;
    tool.execute(ToolInput::from_json(json!({"b": 2}))).await;

    let history = tool.history().await;
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].arguments, json!({"a": 1}));
    assert_eq!(history[1].arguments, json!({"b": 2}));
}

// ===================================================================
// MockAgentBus — failure queue
// ===================================================================

#[tokio::test]
async fn bus_fail_next_send_returns_errors() {
    let bus = MockAgentBus::new();
    bus.fail_next_send(2, "channel closed").await;

    let msg = AgentMessage::TaskRequest {
        task_id: "t1".into(),
        content: "ping".into(),
    };
    let mode = CommunicationMode::Broadcast;

    let r1 = bus.send_and_capture("a", mode.clone(), msg.clone()).await;
    assert!(r1.is_err());

    let r2 = bus.send_and_capture("a", mode.clone(), msg.clone()).await;
    assert!(r2.is_err());

    // Messages are still captured even when send fails
    assert_eq!(bus.message_count().await, 2);
}

#[tokio::test]
async fn bus_fail_next_send_drains_then_proceeds() {
    let bus = MockAgentBus::new();
    bus.fail_next_send(1, "temporary").await;

    let msg = AgentMessage::TaskRequest {
        task_id: "t1".into(),
        content: "ping".into(),
    };
    let mode = CommunicationMode::Broadcast;

    let r1 = bus.send_and_capture("a", mode.clone(), msg.clone()).await;
    assert!(r1.is_err());

    // Second call — queue drained, proceeds to inner bus
    // (inner bus may still error if no receiver, but that's a different error)
    let _ = bus.send_and_capture("a", mode.clone(), msg.clone()).await;
    assert_eq!(bus.message_count().await, 2);
}

#[tokio::test]
async fn bus_fail_next_send_zero_is_noop() {
    let bus = MockAgentBus::new();
    bus.fail_next_send(0, "unused").await;

    let msg = AgentMessage::TaskRequest {
        task_id: "t1".into(),
        content: "ping".into(),
    };
    // Should not inject a failure — proceeds directly to inner bus
    let _ = bus
        .send_and_capture("a", CommunicationMode::Broadcast, msg)
        .await;
    assert_eq!(bus.message_count().await, 1);
}

// ===================================================================
// MockClock — basic operations
// ===================================================================

#[tokio::test]
async fn clock_starts_at_zero() {
    let clock = MockClock::new();
    assert_eq!(clock.now_millis(), 0);
}

#[tokio::test]
async fn clock_starting_at_custom_time() {
    let clock = MockClock::starting_at(Duration::from_secs(100));
    assert_eq!(clock.now_millis(), 100_000);
}

#[tokio::test]
async fn clock_advance_adds_time() {
    let clock = MockClock::new();
    clock.advance(Duration::from_secs(30));
    assert_eq!(clock.now_millis(), 30_000);
    clock.advance(Duration::from_millis(500));
    assert_eq!(clock.now_millis(), 30_500);
}

#[tokio::test]
async fn clock_set_overrides_time() {
    let clock = MockClock::new();
    clock.advance(Duration::from_secs(100));
    clock.set(Duration::from_secs(5));
    assert_eq!(clock.now_millis(), 5_000);
}

// ===================================================================
// MockClock — auto-advance
// ===================================================================

#[tokio::test]
async fn clock_auto_advance_increments_on_each_read() {
    let clock = MockClock::new();
    clock.set_auto_advance(Duration::from_millis(100));

    assert_eq!(clock.now_millis(), 0);
    assert_eq!(clock.now_millis(), 100);
    assert_eq!(clock.now_millis(), 200);
}

#[tokio::test]
async fn clock_clear_auto_advance_stops_incrementing() {
    let clock = MockClock::new();
    clock.set_auto_advance(Duration::from_millis(50));

    assert_eq!(clock.now_millis(), 0);
    assert_eq!(clock.now_millis(), 50);

    clock.clear_auto_advance();
    assert_eq!(clock.now_millis(), 100);
    assert_eq!(clock.now_millis(), 100); // no longer advancing
}

#[tokio::test]
async fn clock_auto_advance_combines_with_manual_advance() {
    let clock = MockClock::new();
    clock.set_auto_advance(Duration::from_millis(10));

    assert_eq!(clock.now_millis(), 0); // reads 0, then advances to 10
    clock.advance(Duration::from_secs(1)); // jumps to 1010
    assert_eq!(clock.now_millis(), 1010); // reads 1010, then advances to 1020
    assert_eq!(clock.now_millis(), 1020);
}

// ===================================================================
// MockClock — SystemClock (smoke test)
// ===================================================================

#[tokio::test]
async fn system_clock_returns_nonzero() {
    let clock = mofa_testing::SystemClock;
    assert!(clock.now_millis() > 0);
}

// ===================================================================
// Integration: failure injection + call counting together
// ===================================================================

#[tokio::test]
async fn backend_combined_fail_next_and_rate_limit() {
    let backend = MockLLMBackend::new();
    backend.register_model(make_config("m")).await.unwrap();
    backend.load_model("m").await.unwrap();

    backend.fail_next(1, OrchestratorError::InferenceFailed("boom".into()));
    backend.set_rate_limit(2);

    // Call 1: fail_next fires (rate limit counter still increments)
    let r1 = backend.infer("m", "a").await;
    assert!(r1.is_err());

    // Call 2, 3: rate limit allows (fail_next drained)
    let r2 = backend.infer("m", "b").await;
    assert!(r2.is_ok());
    let r3 = backend.infer("m", "c").await;
    assert!(r3.is_ok());

    // Call 4: rate limit exceeded
    let r4 = backend.infer("m", "d").await;
    assert!(r4.is_err());

    assert_eq!(backend.call_count(), 4);
}
