use async_trait::async_trait;
use mofa_runtime::agent::capabilities::{AgentCapabilities, AgentCapabilitiesBuilder};
use mofa_runtime::agent::context::AgentContext;
use mofa_runtime::agent::core::MoFAAgent;
use mofa_runtime::agent::error::AgentResult;
use mofa_runtime::agent::types::{AgentInput, AgentOutput, AgentState};
use mofa_runtime::agent::{
    AgentRunner, CronMisfirePolicy, CronRunConfig, PeriodicMissedTickPolicy, PeriodicRunConfig,
};
use std::str::FromStr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

struct HeartbeatAgent {
    id: String,
    name: String,
    state: AgentState,
    capabilities: AgentCapabilities,
    run_count: u64,
}

impl HeartbeatAgent {
    fn new(id: &str, name: &str) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            state: AgentState::Created,
            capabilities: AgentCapabilitiesBuilder::new()
                .tags(vec!["periodic".to_string(), "heartbeat".to_string()])
                .build(),
            run_count: 0,
        }
    }
}

#[async_trait]
impl MoFAAgent for HeartbeatAgent {
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

fn every_second_cron_expression() -> String {
    for expression in ["*/1 * * * * * *", "*/1 * * * * *"] {
        if cron::Schedule::from_str(expression).is_ok() {
            return expression.to_string();
        }
    }

    // Fallback; this should work for cron crate formats that support seconds.
    "*/1 * * * * * *".to_string()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Runtime Periodic Runner (Phase 2) Example ===");

    let cron_expression = every_second_cron_expression();

    let mut runner =
        AgentRunner::new(HeartbeatAgent::new("periodic-heartbeat", "Heartbeat")).await?;

    println!("\nScenario 1: interval scheduling with policy control");
    let interval_outputs = runner
        .run_periodic_with_policy(
            AgentInput::text("collect-interval-metrics"),
            PeriodicRunConfig {
                interval: Duration::from_millis(300),
                max_runs: 3,
                run_immediately: true,
            },
            PeriodicMissedTickPolicy::Skip,
        )
        .await?;

    for (idx, output) in interval_outputs.iter().enumerate() {
        println!("  interval run {} -> {}", idx + 1, output.to_text());
    }

    println!("\nScenario 2: cron scheduling (skip misfires)");
    let cron_skip_outputs = runner
        .run_periodic_cron(
            AgentInput::text("collect-cron-metrics-skip"),
            CronRunConfig {
                expression: cron_expression.clone(),
                max_runs: 2,
                run_immediately: true,
                misfire_policy: CronMisfirePolicy::Skip,
            },
        )
        .await?;

    for (idx, output) in cron_skip_outputs.iter().enumerate() {
        println!("  cron(skip) run {} -> {}", idx + 1, output.to_text());
    }

    println!("\nScenario 3: cron scheduling (run-once misfire policy)");
    let cron_run_once_outputs = runner
        .run_periodic_cron(
            AgentInput::text("collect-cron-metrics-run-once"),
            CronRunConfig {
                expression: cron_expression,
                max_runs: 2,
                run_immediately: true,
                misfire_policy: CronMisfirePolicy::RunOnce,
            },
        )
        .await?;

    for (idx, output) in cron_run_once_outputs.iter().enumerate() {
        println!("  cron(run-once) run {} -> {}", idx + 1, output.to_text());
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
