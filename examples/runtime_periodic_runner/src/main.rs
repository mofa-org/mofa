use anyhow::Result;
use async_trait::async_trait;
use mofa_runtime::agent::capabilities::{AgentCapabilities, AgentCapabilitiesBuilder};
use mofa_runtime::agent::context::AgentContext;
use mofa_runtime::agent::core::MoFAAgent;
use mofa_runtime::agent::error::AgentResult;
use mofa_runtime::agent::types::{AgentInput, AgentOutput, AgentState};
use mofa_runtime::agent::{AgentRunner, PeriodicRunConfig};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

struct HealthProbeAgent {
    id: String,
    name: String,
    state: AgentState,
    capabilities: AgentCapabilities,
    run_count: u64,
}

impl HealthProbeAgent {
    fn new(id: &str, name: &str) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            state: AgentState::Created,
            capabilities: AgentCapabilitiesBuilder::new()
                .tags(vec!["periodic".to_string(), "health-probe".to_string()])
                .build(),
            run_count: 0,
        }
    }
}

#[async_trait]
impl MoFAAgent for HealthProbeAgent {
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
        self.state = AgentState::Executing;
        self.run_count += 1;

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let payload = format!(
            "run={} ts={} task={}",
            self.run_count,
            timestamp,
            input.to_text()
        );

        self.state = AgentState::Ready;
        Ok(AgentOutput::text(payload))
    }

    async fn shutdown(&mut self) -> AgentResult<()> {
        self.state = AgentState::Shutdown;
        Ok(())
    }

    fn state(&self) -> AgentState {
        self.state.clone()
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== Runtime Periodic Runner Example ===");
    println!("Use case: periodic health probe collection with bounded runs.\n");

    let agent = HealthProbeAgent::new("periodic-health-probe", "PeriodicHealthProbe");
    let mut runner = AgentRunner::new(agent).await?;

    println!("Scenario 1: immediate execution (run_immediately=true)");
    let immediate_outputs = runner
        .run_periodic(
            AgentInput::text("collect-health-snapshot"),
            PeriodicRunConfig {
                interval: Duration::from_millis(300),
                max_runs: 3,
                run_immediately: true,
            },
        )
        .await?;
    for (idx, output) in immediate_outputs.iter().enumerate() {
        println!("  immediate run {} -> {}", idx + 1, output.to_text());
    }

    println!("\nScenario 2: delayed first execution (run_immediately=false)");
    let delayed_started = Instant::now();
    let delayed_outputs = runner
        .run_periodic(
            AgentInput::text("collect-delayed-health-snapshot"),
            PeriodicRunConfig {
                interval: Duration::from_millis(400),
                max_runs: 2,
                run_immediately: false,
            },
        )
        .await?;
    println!(
        "  delayed scenario elapsed before completion: {} ms",
        delayed_started.elapsed().as_millis()
    );
    for (idx, output) in delayed_outputs.iter().enumerate() {
        println!("  delayed run {} -> {}", idx + 1, output.to_text());
    }

    let stats = runner.stats().await;
    println!(
        "\nRunner stats: total={}, success={}, failed={}, avg_ms={:.2}",
        stats.total_executions,
        stats.successful_executions,
        stats.failed_executions,
        stats.avg_execution_time_ms
    );

    runner.shutdown().await?;
    println!("Done.");
    Ok(())
}
