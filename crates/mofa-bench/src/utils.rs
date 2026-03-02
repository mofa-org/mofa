//! Benchmark Utilities
//!
//! Helper functions to generate sample data fixtures of varying sizes
//! for use in benchmark harnesses.

use mofa_kernel::agent::types::{
    AgentInput, AgentOutput, ReasoningStep, ReasoningStepType, TokenUsage,
    ToolUsage,
};
use mofa_kernel::agent::{AgentCapabilities, AgentMetadata, AgentState};
use mofa_kernel::message::{AgentMessage, TaskPriority, TaskRequest, TaskStatus};
use serde_json::json;
use std::collections::HashMap;

// ============================================================================
// AgentInput fixtures
// ============================================================================

/// Create a small text AgentInput (~20 bytes).
pub fn small_text_input() -> AgentInput {
    AgentInput::text("Hello, agent!")
}

/// Create a medium text AgentInput (~1KB).
pub fn medium_text_input() -> AgentInput {
    let text = "The quick brown fox jumps over the lazy dog. ".repeat(23);
    AgentInput::text(text)
}

/// Create a large text AgentInput (~10KB).
pub fn large_text_input() -> AgentInput {
    let text = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. ".repeat(180);
    AgentInput::text(text)
}

/// Create a JSON AgentInput with nested structure.
pub fn json_input() -> AgentInput {
    AgentInput::json(json!({
        "task": "analyze",
        "documents": [
            {"id": 1, "content": "First document content for analysis", "metadata": {"source": "web", "timestamp": "2025-01-01T00:00:00Z"}},
            {"id": 2, "content": "Second document with different content", "metadata": {"source": "file", "timestamp": "2025-01-02T00:00:00Z"}},
            {"id": 3, "content": "Third document for comprehensive testing", "metadata": {"source": "api", "timestamp": "2025-01-03T00:00:00Z"}}
        ],
        "options": {
            "max_tokens": 1000,
            "temperature": 0.7,
            "top_p": 0.9,
            "format": "markdown"
        }
    }))
}

/// Create a map AgentInput.
pub fn map_input() -> AgentInput {
    let mut map = HashMap::new();
    map.insert("role".to_string(), json!("assistant"));
    map.insert("task".to_string(), json!("summarize"));
    map.insert(
        "context".to_string(),
        json!("Previous conversation about Rust programming"),
    );
    map.insert("constraints".to_string(), json!(["be concise", "use examples", "cite sources"]));
    AgentInput::map(map)
}

/// Create a binary AgentInput (~1KB).
pub fn binary_input() -> AgentInput {
    AgentInput::Binary([0xDE, 0xAD, 0xBE, 0xEF].repeat(256))
}

// ============================================================================
// AgentOutput fixtures
// ============================================================================

/// Create a simple text AgentOutput.
pub fn simple_text_output() -> AgentOutput {
    AgentOutput::text("Here is the analysis result.")
}

/// Create a rich AgentOutput with tool usage, reasoning, and token stats.
pub fn rich_output() -> AgentOutput {
    AgentOutput::text("Based on my analysis of the documents, here are the key findings.")
        .with_duration(1500)
        .with_tool_usage(ToolUsage::success(
            "web_search",
            json!({"query": "rust performance benchmarking"}),
            json!({"results": ["criterion", "divan", "iai"]}),
            250,
        ))
        .with_tool_usage(ToolUsage::success(
            "file_reader",
            json!({"path": "/data/report.md"}),
            json!({"content": "Report contents..."}),
            50,
        ))
        .with_reasoning_step(ReasoningStep::new(
            ReasoningStepType::Thought,
            "I need to analyze the documents for key themes",
            1,
        ))
        .with_reasoning_step(ReasoningStep::new(
            ReasoningStepType::Action,
            "Searching for relevant information",
            2,
        ))
        .with_reasoning_step(ReasoningStep::new(
            ReasoningStepType::Observation,
            "Found 3 relevant documents",
            3,
        ))
        .with_reasoning_step(ReasoningStep::new(
            ReasoningStepType::FinalAnswer,
            "Here are the synthesized findings",
            4,
        ))
        .with_token_usage(TokenUsage {
            prompt_tokens: 500,
            completion_tokens: 200,
            total_tokens: 700,
        })
        .with_metadata("model", json!("gpt-4"))
        .with_metadata("temperature", json!(0.7))
}

/// Create a large AgentOutput (~10KB text + metadata).
pub fn large_output() -> AgentOutput {
    let long_text = "This is a detailed analysis paragraph. ".repeat(300);
    let mut output = AgentOutput::text(long_text).with_duration(5000);

    for i in 0..20 {
        output = output.with_tool_usage(ToolUsage::success(
            format!("tool_{i}"),
            json!({"input": format!("data_{i}")}),
            json!({"result": format!("output_{i}")}),
            100 + i as u64,
        ));
    }

    output.with_token_usage(TokenUsage {
        prompt_tokens: 2000,
        completion_tokens: 5000,
        total_tokens: 7000,
    })
}

// ============================================================================
// AgentMessage fixtures
// ============================================================================

/// Create a TaskRequest message.
pub fn task_request_message() -> AgentMessage {
    AgentMessage::TaskRequest {
        task_id: "task-001".to_string(),
        content: "Analyze the quarterly report and provide a summary".to_string(),
    }
}

/// Create a TaskResponse message.
pub fn task_response_message() -> AgentMessage {
    AgentMessage::TaskResponse {
        task_id: "task-001".to_string(),
        result: "The quarterly report shows a 15% increase in revenue.".to_string(),
        status: TaskStatus::Success,
    }
}

/// Create a StateSync message.
pub fn state_sync_message() -> AgentMessage {
    AgentMessage::StateSync {
        agent_id: "agent-analyzer-001".to_string(),
        state: AgentState::Running,
    }
}

/// Create a StreamMessage.
pub fn stream_message() -> AgentMessage {
    AgentMessage::StreamMessage {
        stream_id: "stream-001".to_string(),
        message: b"chunk of streaming data for analysis".to_vec(),
        sequence: 42,
    }
}

// ============================================================================
// AgentMetadata fixtures
// ============================================================================

/// Create a sample AgentMetadata.
pub fn sample_agent_metadata(id: &str) -> AgentMetadata {
    AgentMetadata {
        id: id.to_string(),
        name: format!("Agent-{id}"),
        description: Some(format!("Benchmark agent {id}")),
        version: Some("1.0.0".to_string()),
        capabilities: AgentCapabilities::default(),
        state: AgentState::Ready,
    }
}

// ============================================================================
// TaskRequest fixtures
// ============================================================================

/// Create a sample TaskRequest struct.
pub fn sample_task_request() -> TaskRequest {
    TaskRequest {
        task_id: "bench-task-001".to_string(),
        content: "Perform a comprehensive analysis of the data set".to_string(),
        priority: TaskPriority::Normal,
        deadline: Some(std::time::Duration::from_secs(300)),
        metadata: {
            let mut m = std::collections::HashMap::new();
            m.insert("source".to_string(), "benchmark".to_string());
            m.insert("category".to_string(), "analysis".to_string());
            m
        },
    }
}
