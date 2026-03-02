//! Cron Scheduler Example
//!
//! This example demonstrates the CronScheduler with persistence and telemetry.
//! It creates a simple agent that logs messages and schedules it to run every minute.
//!
//! Features demonstrated:
//! - Cron expression scheduling
//! - Schedule persistence (survives process restarts)
//! - Prometheus telemetry (when scheduler-telemetry feature is enabled)
//! - Graceful shutdown handling

use std::sync::Arc;

use async_trait::async_trait;
use mofa_foundation::scheduler::CronScheduler;
use mofa_kernel::agent::context::AgentContext;
use mofa_kernel::agent::core::MoFAAgent;
use mofa_kernel::agent::types::{AgentInput, AgentOutput};
use mofa_kernel::scheduler::{AgentScheduler, ScheduleDefinition};
use mofa_runtime::agent::registry::AgentRegistry;
use tokio::signal;

/// Simple logging agent for demonstration
struct LoggingAgent {
    id: String,
    capabilities: mofa_kernel::agent::capabilities::AgentCapabilities,
}

impl LoggingAgent {
    fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            capabilities: mofa_kernel::agent::capabilities::AgentCapabilities::default(),
        }
    }
}

#[async_trait]
impl MoFAAgent for LoggingAgent {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.id
    }

    fn capabilities(&self) -> &mofa_kernel::agent::capabilities::AgentCapabilities {
        &self.capabilities
    }

    async fn initialize(
        &mut self,
        _ctx: &AgentContext,
    ) -> mofa_kernel::agent::error::AgentResult<()> {
        println!("Initializing logging agent: {}", self.id);
        Ok(())
    }

    async fn execute(
        &mut self,
        input: AgentInput,
        _ctx: &AgentContext,
    ) -> mofa_kernel::agent::error::AgentResult<AgentOutput> {
        let message = match input {
            AgentInput::Text(text) => text,
            AgentInput::Json(data) => format!("{:?}", data),
            AgentInput::Map(data) => format!("{:?}", data),
            AgentInput::Binary(data) => format!("<{} bytes of binary data>", data.len()),
            AgentInput::Multimodal(data) => format!("{:?}", data),
            AgentInput::Texts(texts) => texts.join(" "),
            AgentInput::Empty => "Empty input".to_string(),
            _ => "Unknown input type".to_string(),
        };

        println!("[{}] {}", chrono::Utc::now().format("%H:%M:%S"), message);
        Ok(AgentOutput::text(format!("Logged: {}", message)))
    }

    async fn shutdown(&mut self) -> mofa_kernel::agent::error::AgentResult<()> {
        println!("Shutting down logging agent: {}", self.id);
        Ok(())
    }

    async fn interrupt(
        &mut self,
    ) -> mofa_kernel::agent::error::AgentResult<mofa_kernel::agent::types::InterruptResult> {
        Ok(mofa_kernel::agent::types::InterruptResult::Acknowledged)
    }

    fn state(&self) -> mofa_kernel::agent::types::AgentState {
        mofa_kernel::agent::types::AgentState::Ready
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    println!("Starting Cron Scheduler Example");
    println!("===================================");

    // Create agent registry
    let registry = Arc::new(AgentRegistry::new());

    // Create and register logging agent
    let logging_agent = Arc::new(tokio::sync::RwLock::new(LoggingAgent::new("logger")));
    registry.register(logging_agent).await?;
    println!("Registered logging agent");

    // Create scheduler with persistence and telemetry
    let scheduler = CronScheduler::new(registry, 10) // Global concurrency limit of 10
        .with_persistence("schedules.json"); // Enable persistence

    println!("Created scheduler with persistence enabled");

    // Start scheduler (loads any previously saved schedules)
    scheduler.start().await?;
    println!("Started scheduler and loaded persisted schedules");

    // Schedule the logging agent to run every minute (only if not already loaded from persistence)
    let schedule_id = "log-every-minute";
    if scheduler.list().await.iter().any(|info| info.schedule_id == schedule_id) {
        println!("Schedule '{}' already loaded from persistence", schedule_id);
    } else {
        let schedule_def = ScheduleDefinition::new_cron(
            schedule_id,
            "logger",
            "0 * * * * *", // Every minute at second 0
            1, // Max concurrent executions
            AgentInput::text("Scheduled log message"),
            mofa_kernel::scheduler::MissedTickPolicy::Skip,
        )?;

        let _handle = scheduler.register(schedule_def).await?;
        println!("Scheduled logging agent to run every minute");
        println!("Schedule persisted to schedules.json");
    }

    // Print telemetry info if enabled
    #[cfg(feature = "scheduler-telemetry")]
    {
        println!("Telemetry enabled - Prometheus metrics available:");
        println!("   - mofa_scheduler_executions_total");
        println!("   - mofa_scheduler_missed_ticks_total");
        println!("   - mofa_scheduler_active_runs");
        println!("   - mofa_scheduler_last_run_timestamp_ms");
        println!("   - mofa_scheduler_execution_duration_seconds");
    }

    #[cfg(not(feature = "scheduler-telemetry"))]
    {
        println!("Telemetry disabled - enable with: --features scheduler-telemetry");
    }

    println!("\nRunning... Press Ctrl+C to stop");
    println!("   The agent will log a message every minute at :00 seconds");
    println!("   Schedules are automatically saved and will resume after restart\n");

    // Wait for shutdown signal
    signal::ctrl_c().await?;
    println!("\nReceived shutdown signal, exiting gracefully...");

    Ok(())
}