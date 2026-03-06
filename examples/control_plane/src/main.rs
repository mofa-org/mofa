//! Control plane example
//!
//! Demonstrates how to start the MoFA API gateway with a pre-registered agent
//! factory, then interact with it over HTTP.
//!
//! The example registers an `echo` factory that simply returns the input as
//! its response.  It then starts the gateway on port 8090 and makes a few
//! sample HTTP calls to show the API surface.
//!
//! # Usage
//!
//! ```bash
//! cargo run -p control_plane
//! ```
//!
//! In a second terminal you can then run:
//!
//! ```bash
//! # list agents (initially empty)
//! curl http://localhost:8090/agents
//!
//! # create an echo agent
//! curl -X POST http://localhost:8090/agents \
//!   -H 'content-type: application/json' \
//!   -d '{"id":"echo-1","name":"Echo Agent","agent_type":"echo"}'
//!
//! # send a chat message
//! curl -X POST http://localhost:8090/agents/echo-1/chat \
//!   -H 'content-type: application/json' \
//!   -d '{"message":"hello world"}'
//!
//! # check status
//! curl http://localhost:8090/agents/echo-1/status
//!
//! # health & readiness
//! curl http://localhost:8090/health
//! curl http://localhost:8090/ready
//!
//! # stop the agent
//! curl -X POST http://localhost:8090/agents/echo-1/stop
//!
//! # delete the agent
//! curl -X DELETE http://localhost:8090/agents/echo-1
//! ```

use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

use mofa_kernel::agent::capabilities::AgentCapabilities;
use mofa_kernel::agent::config::AgentConfig;
use mofa_kernel::agent::context::AgentContext;
use mofa_kernel::agent::core::MoFAAgent;
use mofa_kernel::agent::error::AgentResult;
use mofa_kernel::agent::registry::AgentFactory;
use mofa_kernel::agent::types::{AgentInput, AgentOutput, AgentState};

use mofa_runtime::agent::registry::AgentRegistry;

use mofa_gateway::{GatewayConfig, GatewayServer};

// ─────────────────────────────────────────────────────────────────────────────
// Echo agent: echoes the input back as its output
// ─────────────────────────────────────────────────────────────────────────────

struct EchoAgent {
    id: String,
    name: String,
    capabilities: AgentCapabilities,
    state: AgentState,
}

impl EchoAgent {
    fn new(id: &str, name: &str) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            capabilities: AgentCapabilities::builder().with_tag("echo").build(),
            state: AgentState::Created,
        }
    }
}

#[async_trait]
impl MoFAAgent for EchoAgent {
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

    async fn execute(&mut self, input: AgentInput, _ctx: &AgentContext) -> AgentResult<AgentOutput> {
        let response = format!("echo: {}", input.to_text());
        Ok(AgentOutput::text(response))
    }

    async fn shutdown(&mut self) -> AgentResult<()> {
        self.state = AgentState::Shutdown;
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Factory that creates EchoAgent instances
// ─────────────────────────────────────────────────────────────────────────────

struct EchoAgentFactory;

#[async_trait]
impl AgentFactory for EchoAgentFactory {
    async fn create(&self, config: AgentConfig) -> AgentResult<Arc<RwLock<dyn MoFAAgent>>> {
        let agent = EchoAgent::new(&config.id, &config.name);
        Ok(Arc::new(RwLock::new(agent)))
    }

    fn type_id(&self) -> &str {
        "echo"
    }

    fn default_capabilities(&self) -> AgentCapabilities {
        AgentCapabilities::builder().with_tag("echo").build()
    }

    fn description(&self) -> Option<&str> {
        Some("Creates agents that echo their input back as output")
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Entry point
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info,mofa_gateway=debug")
        .init();

    // Build a shared registry and register the echo factory
    let registry = Arc::new(AgentRegistry::new());
    registry
        .register_factory(Arc::new(EchoAgentFactory))
        .await?;

    info!("registered 'echo' agent factory");
    info!("available agent types: {:?}", registry.list_factory_types().await);

    // Start the gateway (blocks until Ctrl-C)
    let config = GatewayConfig::new()
        .with_host("127.0.0.1")
        .with_port(8090);

    info!("starting control-plane on http://127.0.0.1:8090");
    info!("try: curl http://127.0.0.1:8090/health");
    info!("try: curl -X POST http://127.0.0.1:8090/agents \\");
    info!("      -H 'content-type: application/json' \\");
    info!("      -d '{{\"id\":\"echo-1\",\"name\":\"Echo Agent\",\"agent_type\":\"echo\"}}'");

    GatewayServer::new(config, registry)
        .start()
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    Ok(())
}
