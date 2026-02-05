//! 推理组件
//!
//! 从 kernel 层导入 Reasoner trait，提供具体实现

use mofa_kernel::agent::components::reasoner::{
    Reasoner, ReasoningResult,
};
use mofa_kernel::agent::context::AgentContext;
use mofa_kernel::agent::types::AgentInput;
use mofa_kernel::agent::{AgentResult, capabilities::ReasoningStrategy};
use async_trait::async_trait;

// ============================================================================
// 具体推理器实现
// ============================================================================

/// 直接推理器
///
/// 最简单的推理器，直接返回输入作为响应
pub struct DirectReasoner;

#[async_trait]
impl Reasoner for DirectReasoner {
    async fn reason(&self, input: &AgentInput, _ctx: &AgentContext) -> AgentResult<ReasoningResult> {
        Ok(ReasoningResult::respond(input.to_text()))
    }

    fn strategy(&self) -> ReasoningStrategy {
        ReasoningStrategy::Direct
    }

    fn name(&self) -> &str {
        "direct"
    }

    fn description(&self) -> Option<&str> {
        Some("直接推理器，将输入作为输出")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_direct_reasoner() {
        let reasoner = DirectReasoner;
        let ctx = AgentContext::new("test");
        let input = AgentInput::text("Hello, world!");

        let result = reasoner.reason(&input, &ctx).await.unwrap();

        match result.decision {
            Decision::Respond { content } => {
                assert_eq!(content, "Hello, world!");
            }
            _ => panic!("Expected Respond decision"),
        }
    }

    #[test]
    fn test_reasoning_result_builder() {
        let result = ReasoningResult::respond("Hello")
            .with_thought(ThoughtStep::thought("Thinking...", 1))
            .with_confidence(0.9);

        assert_eq!(result.thoughts.len(), 1);
        assert_eq!(result.confidence, 0.9);
    }

    #[test]
    fn test_tool_call_decision() {
        let result = ReasoningResult::call_tool("calculator", serde_json::json!({"a": 1, "b": 2}));

        match result.decision {
            Decision::CallTool { tool_name, arguments } => {
                assert_eq!(tool_name, "calculator");
                assert_eq!(arguments["a"], 1);
            }
            _ => panic!("Expected CallTool decision"),
        }
    }
}
