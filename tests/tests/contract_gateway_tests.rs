use async_trait::async_trait;
use axum::{
    Json,
    body::to_bytes,
    extract::{Path, State},
    http::{HeaderMap, HeaderName, HeaderValue, StatusCode},
    response::IntoResponse,
};
use mofa_gateway::{
    handlers::chat::{ChatRequest, chat},
    middleware::RateLimiter,
    state::AppState,
};
use mofa_runtime::agent::{
    capabilities::AgentCapabilities,
    context::AgentContext,
    core::MoFAAgent,
    error::AgentResult,
    registry::AgentRegistry,
    types::{AgentInput, AgentOutput, AgentState},
};
use mofa_testing::{assert_error_contains, load_fixture};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

#[derive(Debug, Deserialize)]
struct GatewayContractFixture {
    case_name: String,
    kind: Option<String>,
    target: Option<String>,
    #[serde(default = "default_true")]
    agent_present: bool,
    agent_state: Option<String>,
    #[serde(default)]
    request: GatewayRequestFixture,
    #[serde(default)]
    agent_response: GatewayResponseFixture,
    expected: GatewayExpectedFixture,
}

#[derive(Debug, Default, Deserialize)]
struct GatewayRequestFixture {
    message: Option<String>,
    data: Option<Value>,
    session_id: Option<String>,
    #[serde(default)]
    headers: HashMap<String, String>,
}

#[derive(Debug, Default, Deserialize)]
struct GatewayResponseFixture {
    text: Option<String>,
    json: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct GatewayExpectedFixture {
    status: u16,
    output_text: Option<String>,
    output_json: Option<Value>,
    session_id: Option<String>,
    #[serde(default)]
    session_generated: bool,
    expected_input_text: Option<String>,
    expected_input_json: Option<Value>,
    error_contains: Option<String>,
}

fn default_true() -> bool {
    true
}

#[derive(Clone)]
struct ContractAgent {
    id: String,
    name: String,
    state: AgentState,
    capabilities: AgentCapabilities,
    response: AgentOutput,
    last_input: Arc<RwLock<Option<AgentInput>>>,
}

impl ContractAgent {
    fn new(id: &str, state: AgentState, response: AgentOutput) -> Self {
        Self {
            id: id.to_string(),
            name: "Contract Agent".to_string(),
            state,
            capabilities: AgentCapabilities::builder().with_tag("contract").build(),
            response,
            last_input: Arc::new(RwLock::new(None)),
        }
    }
}

#[async_trait]
impl MoFAAgent for ContractAgent {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn capabilities(&self) -> &AgentCapabilities {
        &self.capabilities
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
        *self.last_input.write().await = Some(input);
        Ok(self.response.clone())
    }

    async fn shutdown(&mut self) -> AgentResult<()> {
        self.state = AgentState::Shutdown;
        Ok(())
    }

    fn state(&self) -> AgentState {
        self.state.clone()
    }
}

fn parse_agent_state(raw: Option<&str>) -> AgentState {
    match raw.unwrap_or("Ready") {
        "Created" => AgentState::Created,
        "Ready" => AgentState::Ready,
        "Running" => AgentState::Running,
        "Paused" => AgentState::Paused,
        "Shutdown" => AgentState::Shutdown,
        other => AgentState::Error(other.to_string()),
    }
}

fn fixture_response_body(fixture: &GatewayResponseFixture) -> AgentOutput {
    if let Some(value) = &fixture.json {
        AgentOutput::json(value.clone())
    } else if let Some(text) = &fixture.text {
        AgentOutput::text(text.clone())
    } else {
        AgentOutput::text("ok")
    }
}

fn make_headers(headers: &HashMap<String, String>) -> HeaderMap {
    let mut map = HeaderMap::new();
    for (key, value) in headers {
        let name = HeaderName::from_bytes(key.as_bytes()).expect("valid header name");
        let value = HeaderValue::from_str(value).expect("valid header value");
        map.insert(name, value);
    }
    map
}

async fn run_gateway_fixture(relative_path: &str) {
    let fixture: GatewayContractFixture = load_fixture(relative_path).expect("fixture must load");
    assert_eq!(fixture.kind.as_deref(), Some("contract"));
    assert_eq!(fixture.target.as_deref(), Some("gateway-chat"));

    let registry = Arc::new(AgentRegistry::new());
    let rate_limiter = Arc::new(RateLimiter::new(100, Duration::from_secs(60)));
    let app_state = Arc::new(AppState::new(registry.clone(), rate_limiter));

    let mut registered_agent = None;
    if fixture.agent_present {
        let agent = ContractAgent::new(
            "contract-agent",
            parse_agent_state(fixture.agent_state.as_deref()),
            fixture_response_body(&fixture.agent_response),
        );
        let agent = Arc::new(RwLock::new(agent));
        registry
            .register(agent.clone())
            .await
            .expect("register agent");
        registered_agent = Some(agent);
    }

    let result = chat(
        State(app_state),
        Path("contract-agent".to_string()),
        make_headers(&fixture.request.headers),
        Json(ChatRequest {
            message: fixture.request.message.clone(),
            data: fixture.request.data.clone(),
            session_id: fixture.request.session_id.clone(),
        }),
    )
    .await;

    let response = match result {
        Ok(success) => success.into_response(),
        Err(err) => err.into_response(),
    };

    assert_eq!(
        response.status(),
        StatusCode::from_u16(fixture.expected.status).expect("valid status"),
        "fixture '{}' returned unexpected status",
        fixture.case_name
    );

    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body");
    let parsed: Value = serde_json::from_slice(&body).expect("json response");

    if let Some(expected_text) = &fixture.expected.output_text {
        assert_eq!(parsed["output"], json!({ "Text": expected_text }));
    }

    if let Some(expected_json) = &fixture.expected.output_json {
        assert_eq!(parsed["output"], json!({ "Json": expected_json }));
    }

    if let Some(expected_session_id) = &fixture.expected.session_id {
        assert_eq!(parsed["session_id"].as_str(), Some(expected_session_id.as_str()));
    }

    if fixture.expected.session_generated {
        let session_id = parsed["session_id"].as_str().expect("session_id");
        assert!(!session_id.trim().is_empty());
    }

    if let Some(expected_error) = &fixture.expected.error_contains {
        let error = parsed
            .get("error")
            .and_then(Value::as_str)
            .expect("error response");
        assert_error_contains(error, expected_error);
    }

    if let Some(agent) = registered_agent {
        let last_input_store = {
            let guard = agent.read().await;
            guard.last_input.clone()
        };
        let last_input = last_input_store.read().await.clone();

        if let Some(expected_text) = &fixture.expected.expected_input_text {
            assert_eq!(
                last_input.as_ref().and_then(AgentInput::as_text),
                Some(expected_text.as_str())
            );
        }

        if let Some(expected_json) = &fixture.expected.expected_input_json {
            let actual_json = last_input
                .as_ref()
                .map(AgentInput::to_json)
                .expect("agent should have received input");
            assert_eq!(&actual_json, expected_json);
        }
    }
}

#[tokio::test]
async fn gateway_contract_message_only() {
    run_gateway_fixture("contracts/gateway/message_only.yaml").await;
}

#[tokio::test]
async fn gateway_contract_data_precedence() {
    run_gateway_fixture("contracts/gateway/data_precedence.json").await;
}

#[tokio::test]
async fn gateway_contract_preserves_session_id() {
    run_gateway_fixture("contracts/gateway/preserved_session.yaml").await;
}

#[tokio::test]
async fn gateway_contract_generates_session_id() {
    run_gateway_fixture("contracts/gateway/generated_session.json").await;
}

#[tokio::test]
async fn gateway_contract_invalid_state() {
    run_gateway_fixture("contracts/gateway/invalid_state.yaml").await;
}

#[tokio::test]
async fn gateway_contract_missing_agent() {
    run_gateway_fixture("contracts/gateway/missing_agent.json").await;
}
