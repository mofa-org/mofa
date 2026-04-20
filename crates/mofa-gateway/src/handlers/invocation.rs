//! InvocationTarget routing for the Gateway chat-completions endpoint.
//!
//! [`InvocationRouter`] resolves an incoming OpenAI-compatible request to an
//! [`InvocationTarget`] by consulting the [`AgentRegistry`] first, then
//! falling back to the local [`InferenceOrchestrator`] when no registered
//! agent matches the requested model name.
//!
//! # Dispatch flow
//!
//! ```text
//! POST /v1/chat/completions { "model": "my-agent" }
//!            │
//!            ▼
//!   InvocationRouter::resolve("my-agent")
//!            │
//!   ┌────────┴────────────────────────────┐
//!   │  AgentRegistry contains "my-agent"? │
//!   └────────┬────────────────────────────┘
//!            │ yes                 │ no
//!            ▼                     ▼
//!   InvocationTarget::Agent   InvocationTarget::LocalInference
//!            │                     │
//!            ▼                     ▼
//!   [registry metadata]    InferenceBridge::run_chat_completion
//! ```

use std::sync::Arc;

use mofa_kernel::gateway::InvocationTarget;
use mofa_runtime::agent::registry::AgentRegistry;

use crate::{
    error::GatewayError,
    inference_bridge::{ChatCompletionRequest, ChatCompletionResponse, InferenceBridge},
};

/// Routes `/v1/chat/completions` requests through [`InvocationTarget`] dispatch.
///
/// Constructed once at server startup and shared via [`axum::Extension`].
pub struct InvocationRouter {
    registry: Arc<AgentRegistry>,
    bridge: Arc<InferenceBridge>,
}

impl InvocationRouter {
    /// Create a new router backed by the given registry and inference bridge.
    pub fn new(registry: Arc<AgentRegistry>, bridge: Arc<InferenceBridge>) -> Self {
        Self { registry, bridge }
    }

    /// Resolve the [`InvocationTarget`] for the given model name.
    ///
    /// Resolution order:
    /// 1. If the `AgentRegistry` contains an agent whose `agent_id` exactly
    ///    matches `model`, return [`InvocationTarget::Agent`].
    /// 2. Otherwise return [`InvocationTarget::LocalInference`] so the
    ///    request falls through to the `InferenceOrchestrator`.
    ///
    /// This two-step fallback ensures that raw model names continue to work
    /// without an explicit registry entry, while named agents always win when
    /// present.
    pub async fn resolve(&self, model: &str) -> InvocationTarget {
        if self.registry.contains(model).await {
            tracing::debug!(
                target = "mofa_gateway::invocation",
                model = model,
                "InvocationTarget resolved → Agent"
            );
            InvocationTarget::Agent {
                agent_id: model.to_string(),
            }
        } else {
            tracing::debug!(
                target = "mofa_gateway::invocation",
                model = model,
                "InvocationTarget resolved → LocalInference (no registry entry)"
            );
            InvocationTarget::LocalInference {
                model: model.to_string(),
            }
        }
    }

    /// Execute the dispatch described by `target` and return an
    /// OpenAI-compatible response.
    ///
    /// # Current behaviour
    ///
    /// * `Agent`          — look up agent metadata in the registry and include
    ///   the agent description in the response.  Full agent invocation
    ///   (streaming, tool calls) is planned for a follow-up PR once the
    ///   agent execution protocol is finalised.
    /// * `LocalInference` — delegates to [`InferenceBridge::run_chat_completion`].
    /// * `Proxy`          — returns an internal error; Proxy dispatch is
    ///   handled by the dedicated proxy module (`crates/mofa-gateway/src/proxy/`).
    pub async fn dispatch(
        &self,
        target: InvocationTarget,
        request: ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, GatewayError> {
        tracing::info!(
            target = "mofa_gateway::invocation",
            invocation_target = %target,
            model = %request.model,
            "dispatching chat completion"
        );

        match target {
            InvocationTarget::Agent { ref agent_id } => {
                // Retrieve agent metadata from the registry.
                let content = match self.registry.get_metadata(agent_id).await {
                    Some(meta) => meta
                        .description
                        .unwrap_or_else(|| {
                            format!(
                                "Agent '{}' (name: '{}') is registered and ready.",
                                agent_id, meta.name
                            )
                        }),
                    None => {
                        // Race condition: agent was present at resolve() time but
                        // deregistered before dispatch.  Fall through to bridge.
                        tracing::warn!(
                            target = "mofa_gateway::invocation",
                            agent_id = agent_id,
                            "Agent disappeared between resolve and dispatch — falling back to LocalInference"
                        );
                        return self.bridge.run_chat_completion(request).await;
                    }
                };

                let prompt_tokens: u32 = request
                    .messages
                    .iter()
                    .map(|m| (m.content.len() as f32 / 4.0) as u32)
                    .sum();
                let completion_tokens = (content.len() as f32 / 4.0) as u32;

                Ok(ChatCompletionResponse {
                    id: format!("chatcmpl-{}", uuid::Uuid::new_v4()),
                    object_type: "chat.completion".to_string(),
                    created: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .unwrap_or(0),
                    model: request.model,
                    choices: vec![crate::inference_bridge::Choice {
                        index: 0,
                        message: crate::inference_bridge::Message {
                            role: "assistant".to_string(),
                            content,
                        },
                        finish_reason: "stop".to_string(),
                    }],
                    usage: crate::inference_bridge::Usage {
                        prompt_tokens,
                        completion_tokens,
                        total_tokens: prompt_tokens + completion_tokens,
                    },
                })
            }

            InvocationTarget::LocalInference { .. } => {
                // Existing path: delegate entirely to the InferenceBridge.
                self.bridge.run_chat_completion(request).await
            }

            InvocationTarget::Proxy { ref url } => {
                // Proxy dispatch is handled by `crates/mofa-gateway/src/proxy/`.
                Err(GatewayError::Internal(format!(
                    "Proxy dispatch to '{}' is not supported through InvocationRouter; \
                     use the dedicated proxy module instead.",
                    url
                )))
            }

            // Forward-compatibility: handle any future variants added to the
            // non_exhaustive InvocationTarget without a panic.
            _ => Err(GatewayError::Internal(
                "Unknown InvocationTarget variant — upgrade mofa-gateway to support it."
                    .to_string(),
            )),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use mofa_foundation::inference::OrchestratorConfig;
    use mofa_kernel::agent::{
        capabilities::AgentCapabilities,
        context::AgentContext,
        core::MoFAAgent,
        error::AgentResult,
        types::{AgentInput, AgentOutput, AgentState},
    };
    use mofa_runtime::agent::registry::AgentRegistry;
    use async_trait::async_trait;
    use tokio::sync::RwLock;

    // ── Minimal in-process test agent ────────────────────────────────────────

    struct StubAgent {
        id: String,
        name: String,
        state: AgentState,
        capabilities: AgentCapabilities,
    }

    impl StubAgent {
        fn new(id: &str, name: &str) -> Self {
            Self {
                id: id.to_string(),
                name: name.to_string(),
                state: AgentState::Created,
                capabilities: AgentCapabilities::default(),
            }
        }
    }

    #[async_trait]
    impl MoFAAgent for StubAgent {
        fn id(&self) -> &str { &self.id }
        fn name(&self) -> &str { &self.name }
        fn capabilities(&self) -> &AgentCapabilities { &self.capabilities }
        fn state(&self) -> AgentState { self.state.clone() }
        async fn initialize(&mut self, _ctx: &AgentContext) -> AgentResult<()> {
            self.state = AgentState::Ready;
            Ok(())
        }
        async fn execute(&mut self, _input: AgentInput, _ctx: &AgentContext) -> AgentResult<AgentOutput> {
            Ok(AgentOutput::text("stub"))
        }
        async fn shutdown(&mut self) -> AgentResult<()> {
            self.state = AgentState::Shutdown;
            Ok(())
        }
    }

    // ── Helpers ──────────────────────────────────────────────────────────────

    fn make_bridge() -> Arc<InferenceBridge> {
        Arc::new(InferenceBridge::new(OrchestratorConfig::default()))
    }

    async fn make_router_with_agent(agent_id: &str) -> InvocationRouter {
        let registry = Arc::new(AgentRegistry::new());
        let agent = Arc::new(RwLock::new(StubAgent::new(agent_id, agent_id)));
        registry.register(agent).await.expect("registration should succeed");
        InvocationRouter::new(registry, make_bridge())
    }

    fn make_router_empty() -> InvocationRouter {
        InvocationRouter::new(Arc::new(AgentRegistry::new()), make_bridge())
    }

    fn make_request(model: &str) -> ChatCompletionRequest {
        ChatCompletionRequest {
            model: model.to_string(),
            messages: vec![crate::inference_bridge::Message {
                role: "user".to_string(),
                content: "Hello".to_string(),
            }],
            max_tokens: Some(64),
            temperature: Some(0.7),
            stream: Some(false),
        }
    }

    // ── resolve() ────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn resolve_known_agent_returns_agent_target() {
        let router = make_router_with_agent("summariser").await;
        let target = router.resolve("summariser").await;
        assert_eq!(
            target,
            InvocationTarget::Agent { agent_id: "summariser".to_string() }
        );
    }

    #[tokio::test]
    async fn resolve_unknown_model_returns_local_inference() {
        let router = make_router_empty();
        let target = router.resolve("gpt-4o").await;
        assert_eq!(
            target,
            InvocationTarget::LocalInference { model: "gpt-4o".to_string() }
        );
    }

    // ── dispatch() ───────────────────────────────────────────────────────────

    #[tokio::test]
    async fn dispatch_local_inference_returns_valid_response() {
        let router = make_router_empty();
        let target = InvocationTarget::LocalInference { model: "test-model".to_string() };
        let resp = router.dispatch(target, make_request("test-model")).await.unwrap();
        assert!(resp.id.starts_with("chatcmpl-"));
        assert_eq!(resp.object_type, "chat.completion");
        assert_eq!(resp.choices.len(), 1);
    }

    #[tokio::test]
    async fn dispatch_agent_returns_valid_response() {
        let router = make_router_with_agent("mofa-kernel").await;
        let target = InvocationTarget::Agent { agent_id: "mofa-kernel".to_string() };
        let resp = router.dispatch(target, make_request("mofa-kernel")).await.unwrap();
        assert!(resp.id.starts_with("chatcmpl-"));
        assert_eq!(resp.choices[0].message.role, "assistant");
        // The content should mention the agent name
        assert!(resp.choices[0].message.content.contains("mofa-kernel"));
    }

    #[tokio::test]
    async fn dispatch_proxy_returns_error() {
        let router = make_router_empty();
        let target = InvocationTarget::Proxy { url: "https://api.openai.com".to_string() };
        let err = router.dispatch(target, make_request("gpt-4")).await;
        assert!(err.is_err());
        let msg = format!("{}", err.unwrap_err());
        assert!(msg.contains("proxy") || msg.contains("dedicated"));
    }
}
