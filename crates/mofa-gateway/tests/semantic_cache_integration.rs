//! Integration tests for Gateway Chat Semantic Cache
//!
//! Validates:
//! - Semantic cache hit for near-identical queries
//! - Agent tenant isolation (zero cross-agent leakage)
//! - Disabled cache passthrough
//! - Execution count verification (LLM execution bypassed on cache hit)

use async_trait::async_trait;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use mofa_gateway::handlers::chat::ChatResponse;
use mofa_gateway::middleware::semantic_cache::{
    MockPromptEmbedder, SemanticCache, SemanticCacheConfig,
};
use mofa_gateway::server::{GatewayServer, ServerConfig};
use mofa_kernel::agent::{
    AgentCapabilities, AgentContext, AgentInput, AgentOutput, AgentResult, AgentState, MoFAAgent,
};
use mofa_runtime::agent::registry::AgentRegistry;
use serde_json::json;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tokio::sync::RwLock;
use tower::ServiceExt;

/// Mock agent that counts how many times its `execute` method is invoked.
struct MockCountingAgent {
    id: String,
    name: String,
    capabilities: AgentCapabilities,
    state: AgentState,
    call_count: Arc<AtomicUsize>,
}

impl MockCountingAgent {
    fn new(id: &str, call_count: Arc<AtomicUsize>) -> Self {
        Self {
            id: id.to_string(),
            name: format!("Agent-{}", id),
            capabilities: AgentCapabilities::default(),
            state: AgentState::Ready,
            call_count,
        }
    }
}

#[async_trait]
impl MoFAAgent for MockCountingAgent {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn capabilities(&self) -> &AgentCapabilities {
        &self.capabilities
    }

    fn state(&self) -> AgentState {
        self.state.clone()
    }

    async fn initialize(&mut self, _ctx: &AgentContext) -> AgentResult<()> {
        self.state = AgentState::Ready;
        Ok(())
    }

    async fn execute(
        &mut self,
        input: AgentInput,
        _ctx: &AgentContext,
    ) -> AgentResult<AgentOutput> {
        let count = self.call_count.fetch_add(1, Ordering::SeqCst);
        let text = match input {
            AgentInput::Text(t) => t,
            _ => "default".to_string(),
        };
        Ok(AgentOutput::text(format!("reply #{}: {}", count + 1, text)))
    }

    async fn shutdown(&mut self) -> AgentResult<()> {
        self.state = AgentState::Shutdown;
        Ok(())
    }
}

#[tokio::test]
async fn test_gateway_chat_semantic_cache_hit_and_bypass() {
    let registry = Arc::new(AgentRegistry::new());
    let agent_calls = Arc::new(AtomicUsize::new(0));
    let agent = Arc::new(RwLock::new(MockCountingAgent::new(
        "agent-helper",
        agent_calls.clone(),
    )));
    registry.register(agent).await.unwrap();

    // Prepare mock embedder with near-identical queries
    let embedder = Arc::new(MockPromptEmbedder::new(3));
    embedder.register("What is MoFA?", vec![1.0, 0.0, 0.0]);
    embedder.register("What is MoFA framework?", vec![0.98, 0.19, 0.0]); // sim ~ 0.98
    embedder.register("Tell me a joke", vec![0.0, 1.0, 0.0]);

    let cache_config = SemanticCacheConfig {
        enabled: true,
        threshold: 0.90,
        ..Default::default()
    };
    let semantic_cache = Arc::new(SemanticCache::new(cache_config, embedder));

    let server_config = ServerConfig::new()
        .with_host("127.0.0.1")
        .with_port(0)
        .with_cors(false)
        .with_rate_limit(10_000, Duration::from_secs(60));

    let app = GatewayServer::new(server_config, registry)
        .with_semantic_cache(semantic_cache.clone())
        .build_router();

    // Request 1: "What is MoFA?" -> Cache miss, executes agent
    let req1 = Request::builder()
        .method("POST")
        .uri("/agents/agent-helper/chat")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({"message": "What is MoFA?"}).to_string(),
        ))
        .unwrap();

    let res1 = app.clone().oneshot(req1).await.unwrap();
    assert_eq!(res1.status(), StatusCode::OK);
    let bytes1 = axum::body::to_bytes(res1.into_body(), usize::MAX)
        .await
        .unwrap();
    let body1: ChatResponse = serde_json::from_slice(&bytes1).unwrap();

    assert_eq!(body1.agent_id, "agent-helper");
    assert_eq!(body1.output, json!({"Text": "reply #1: What is MoFA?"}));
    assert_eq!(body1.cached, Some(false));
    assert_eq!(body1.cache_similarity, None);
    assert_eq!(agent_calls.load(Ordering::SeqCst), 1);

    // Request 2: "What is MoFA framework?" -> Semantic Cache HIT (sim > 0.90)!
    // Agent execution should be bypassed completely!
    let req2 = Request::builder()
        .method("POST")
        .uri("/agents/agent-helper/chat")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({"message": "What is MoFA framework?"}).to_string(),
        ))
        .unwrap();

    let res2 = app.clone().oneshot(req2).await.unwrap();
    assert_eq!(res2.status(), StatusCode::OK);
    let bytes2 = axum::body::to_bytes(res2.into_body(), usize::MAX)
        .await
        .unwrap();
    let body2: ChatResponse = serde_json::from_slice(&bytes2).unwrap();

    assert_eq!(body2.agent_id, "agent-helper");
    // Returns cached output from the first execution!
    assert_eq!(body2.output, json!({"Text": "reply #1: What is MoFA?"}));
    assert_eq!(body2.cached, Some(true));
    assert!(body2.cache_similarity.unwrap() >= 0.90);
    // Crucial check: Agent execute was NOT called a second time!
    assert_eq!(
        agent_calls.load(Ordering::SeqCst),
        1,
        "Agent execution should be bypassed on semantic cache hit"
    );

    // Request 3: "Tell me a joke" -> Different query -> Cache miss -> Executes agent
    let req3 = Request::builder()
        .method("POST")
        .uri("/agents/agent-helper/chat")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({"message": "Tell me a joke"}).to_string(),
        ))
        .unwrap();

    let res3 = app.clone().oneshot(req3).await.unwrap();
    assert_eq!(res3.status(), StatusCode::OK);
    let bytes3 = axum::body::to_bytes(res3.into_body(), usize::MAX)
        .await
        .unwrap();
    let body3: ChatResponse = serde_json::from_slice(&bytes3).unwrap();

    assert_eq!(body3.output, json!({"Text": "reply #2: Tell me a joke"}));
    assert_eq!(body3.cached, Some(false));
    assert_eq!(agent_calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn test_gateway_chat_agent_boundary_isolation() {
    let registry = Arc::new(AgentRegistry::new());

    let calls_a = Arc::new(AtomicUsize::new(0));
    let agent_a = Arc::new(RwLock::new(MockCountingAgent::new("agent-alpha", calls_a.clone())));
    registry.register(agent_a).await.unwrap();

    let calls_b = Arc::new(AtomicUsize::new(0));
    let agent_b = Arc::new(RwLock::new(MockCountingAgent::new("agent-beta", calls_b.clone())));
    registry.register(agent_b).await.unwrap();

    let embedder = Arc::new(MockPromptEmbedder::new(3));
    embedder.register("shared prompt", vec![1.0, 0.0, 0.0]);

    let cache = Arc::new(SemanticCache::new(
        SemanticCacheConfig::enabled(0.90),
        embedder,
    ));

    let app = GatewayServer::new(ServerConfig::new().with_cors(false), registry)
        .with_semantic_cache(cache)
        .build_router();

    // 1. Send to agent-alpha
    let req_a = Request::builder()
        .method("POST")
        .uri("/agents/agent-alpha/chat")
        .header("content-type", "application/json")
        .body(Body::from(json!({"message": "shared prompt"}).to_string()))
        .unwrap();

    let res_a = app.clone().oneshot(req_a).await.unwrap();
    assert_eq!(res_a.status(), StatusCode::OK);
    assert_eq!(calls_a.load(Ordering::SeqCst), 1);

    // 2. Send identical prompt to agent-beta -> MUST NOT HIT agent-alpha's cache!
    let req_b = Request::builder()
        .method("POST")
        .uri("/agents/agent-beta/chat")
        .header("content-type", "application/json")
        .body(Body::from(json!({"message": "shared prompt"}).to_string()))
        .unwrap();

    let res_b = app.clone().oneshot(req_b).await.unwrap();
    assert_eq!(res_b.status(), StatusCode::OK);
    let bytes_b = axum::body::to_bytes(res_b.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_b: ChatResponse = serde_json::from_slice(&bytes_b).unwrap();

    // Must be agent-beta output, not agent-alpha!
    assert_eq!(body_b.agent_id, "agent-beta");
    assert_eq!(body_b.output, json!({"Text": "reply #1: shared prompt"}));
    assert_eq!(body_b.cached, Some(false));
    assert_eq!(
        calls_b.load(Ordering::SeqCst),
        1,
        "Agent-beta should have executed its own handler"
    );
}

#[tokio::test]
async fn test_gateway_chat_disabled_cache_mode() {
    let registry = Arc::new(AgentRegistry::new());
    let calls = Arc::new(AtomicUsize::new(0));
    let agent = Arc::new(RwLock::new(MockCountingAgent::new("agent-plain", calls.clone())));
    registry.register(agent).await.unwrap();

    // Server WITHOUT semantic cache configured
    let app = GatewayServer::new(ServerConfig::new().with_cors(false), registry).build_router();

    let req = Request::builder()
        .method("POST")
        .uri("/agents/agent-plain/chat")
        .header("content-type", "application/json")
        .body(Body::from(json!({"message": "hello world"}).to_string()))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: ChatResponse = serde_json::from_slice(&bytes).unwrap();

    assert_eq!(body.output, json!({"Text": "reply #1: hello world"}));
    assert_eq!(body.cached, None, "When cache is disabled, cached field is omitted");
    assert_eq!(body.cache_similarity, None);
}

