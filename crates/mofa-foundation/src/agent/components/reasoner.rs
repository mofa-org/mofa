//! 推理组件
//! Reasoning component
//!
//! 从 kernel 层导入 Reasoner trait，提供具体实现
//! Import Reasoner trait from kernel layer, providing concrete implementation

use async_trait::async_trait;
use mofa_kernel::agent::components::reasoner::{Reasoner, ReasoningResult};
use mofa_kernel::agent::context::AgentContext;
use mofa_kernel::agent::types::AgentInput;
use mofa_kernel::agent::{AgentResult, capabilities::ReasoningStrategy};

// ============================================================================
// 具体推理器实现
// Concrete reasoner implementation
// ============================================================================

/// 直接推理器
/// Direct Reasoner
///
/// 最简单的推理器，直接返回输入作为响应
/// Simplest reasoner, directly returns input as response
pub struct DirectReasoner;

#[async_trait]
impl Reasoner for DirectReasoner {
    async fn reason(
        &self,
        input: &AgentInput,
        _ctx: &AgentContext,
    ) -> AgentResult<ReasoningResult> {
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
        // Direct reasoner, uses input as output
    }
}
