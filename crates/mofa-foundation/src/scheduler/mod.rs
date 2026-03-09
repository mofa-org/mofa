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

pub mod persistence;

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use cron::Schedule;
use tokio::sync::{RwLock, Semaphore, oneshot};
use tokio::task::JoinHandle;
use tokio::time::{Duration, Instant, interval};

#[cfg(feature = "scheduler-telemetry")]
use metrics::{counter, gauge, histogram};

use mofa_kernel::agent::context::AgentContext;
use mofa_kernel::agent::core::MoFAAgent;
use mofa_kernel::agent::types::{AgentInput, AgentOutput};
use mofa_kernel::scheduler::{
    AgentScheduler, Clock, MissedTickPolicy, ScheduleDefinition, ScheduleHandle, ScheduleInfo,
    SchedulerError, SystemClock,
};
use mofa_runtime::agent::execution::{ExecutionEngine, ExecutionOptions, ExecutionResult};
use mofa_runtime::agent::registry::AgentRegistry;

use crate::scheduler::persistence::SchedulePersistence;

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
        let next_run_ms = self.calculate_next_run_time(clock);

        ScheduleInfo::new(
            self.definition.schedule_id.clone(),
            self.definition.agent_id.clone(),
            next_run_ms,
            if last_run == 0 {
                None
            } else {
                Some(last_run as u64)
            },
            self.consecutive_failures.load(Ordering::Relaxed),
            self.paused.load(Ordering::Acquire),
        )
    }

    /// Calculate the next run time based on schedule type and current time.
    fn calculate_next_run_time(&self, clock: &dyn Clock) -> Option<u64> {
        if self.paused.load(Ordering::Acquire) {
            return None; // No next run if paused
        }

        let now_ms = clock.now_millis();

        if let Some(cron_expr) = &self.definition.cron_expression {
            // For cron schedules, find the next occurrence
            if let Ok(schedule) = cron_expr.parse::<Schedule>() {
                let now = DateTime::from_timestamp_millis(now_ms as i64)?
                    .with_timezone(&Utc);
                if let Some(next) = schedule.upcoming(Utc).next() {
                    return Some(next.timestamp_millis() as u64);
                }
            }
        } else if let Some(interval_ms) = self.definition.interval_ms {
            // For interval schedules, add interval to last run time or current time
            let last_run_u64 = self.last_run_ms.load(Ordering::Relaxed) as u64;
            let base_time = if last_run_u64 > 0 {
                last_run_u64 + interval_ms
            } else {
                now_ms + interval_ms
            };
            return Some(base_time);
        }

        None
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
    /// Optional persistence backend for saving/loading schedules.
    persistence: Option<SchedulePersistence>,
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
            persistence: None,
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
            persistence: None,
        }
    }

    /// Enable persistence for this scheduler.
    ///
    /// When persistence is enabled:
    /// - `start()` will load and re-register previously saved schedules
    /// - `register()` and `unregister()` will automatically persist changes
    ///
    /// # Parameters
    ///
    /// - `file_path`: Path where schedule definitions will be saved/loaded
    ///
    /// # Returns
    ///
    /// Returns self for method chaining.
    pub fn with_persistence(mut self, file_path: impl AsRef<std::path::Path>) -> Self {
        self.persistence = Some(SchedulePersistence::new(file_path));
        self
    }

    /// Start the scheduler by loading any persisted schedules.
    ///
    /// This method should be called after creating the scheduler and registering
    /// any agents, but before the application starts accepting requests.
    ///
    /// If persistence is not enabled, this is a no-op.
    ///
    /// # Errors
    ///
    /// Returns `SchedulerError::PersistenceError` if loading fails.
    /// Returns `SchedulerError::AgentNotFound` if a persisted schedule references
    /// an agent that is no longer registered.
    pub async fn start(&self) -> Result<(), SchedulerError> {
        if let Some(persistence) = &self.persistence {
            let definitions = persistence.load().await?;
            for def in definitions {
                // Validate agent still exists
                if self.registry.get(&def.agent_id).await.is_none() {
                    return Err(SchedulerError::AgentNotFound(def.agent_id));
                }

                // Re-register the schedule (this will also persist it again)
                self.register(def).await?;
            }
        }
        Ok(())
    }

    /// Get all current schedule definitions for persistence.
    async fn list_definitions(&self) -> Vec<ScheduleDefinition> {
        let schedules = self.schedules.read().await;
        schedules
            .values()
            .map(|entry| entry.definition.clone())
            .collect()
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

        // Persist the updated schedule list if persistence is enabled
        if let Some(persistence) = &self.persistence {
            let definitions = self.list_definitions().await;
            if let Err(e) = persistence.save(&definitions).await {
                // Log the error but don't fail the registration
                tracing::error!("Failed to persist schedule registration: {}", e);
            }
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

        // Persist the updated schedule list if persistence is enabled
        if let Some(persistence) = &self.persistence {
            let definitions = schedules
                .values()
                .map(|entry| entry.definition.clone())
                .collect::<Vec<_>>();
            if let Err(e) = persistence.save(&definitions).await {
                // Log the error but don't fail the unregistration
                tracing::error!("Failed to persist schedule unregistration: {}", e);
            }
        }

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
                                        #[cfg(feature = "scheduler-telemetry")]
                                        counter!("mofa_scheduler_missed_ticks_total", "schedule_id" => schedule_id.clone()).increment(1);
                                        tracing::debug!("Global concurrency limit reached for schedule {}", schedule_id);
                                        continue;
                                    }
                                };

                                let schedule_permit = match per_schedule_semaphore.try_acquire() {
                                    Ok(permit) => permit,
                                    Err(_) => {
                                        #[cfg(feature = "scheduler-telemetry")]
                                        counter!("mofa_scheduler_missed_ticks_total", "schedule_id" => schedule_id.clone()).increment(1);
                                        tracing::debug!("Schedule concurrency limit reached for schedule {}", schedule_id);
                                        drop(global_permit); // Release global permit
                                        continue;
                                    }
                                };

                                // Record active run
                                #[cfg(feature = "scheduler-telemetry")]
                                gauge!("mofa_scheduler_active_runs", "schedule_id" => schedule_id.clone()).increment(1.0);

                                // Execute agent
                                let registry_clone = Arc::clone(&registry);
                                let agent_id_clone = agent_id.clone();
                                let input_clone = input_template.clone();
                                let schedule_id_clone = schedule_id.clone();

                                tokio::spawn(async move {
                                    let start_time = std::time::Instant::now();
                                    let engine = ExecutionEngine::new(registry_clone);
                                    match engine.execute(&agent_id_clone, input_clone, Default::default()).await {
                                        Ok(result) => {
                                            #[cfg(feature = "scheduler-telemetry")]
                                            {
                                                counter!("mofa_scheduler_executions_total",
                                                    "schedule_id" => schedule_id_clone.clone(),
                                                    "agent_id" => agent_id_clone.clone(),
                                                    "status" => "success"
                                                ).increment(1);
                                                histogram!("mofa_scheduler_execution_duration_seconds",
                                                    "schedule_id" => schedule_id_clone.clone(),
                                                    "agent_id" => agent_id_clone.clone()
                                                ).record(start_time.elapsed().as_secs_f64());
                                                gauge!("mofa_scheduler_last_run_timestamp_ms",
                                                    "schedule_id" => schedule_id_clone.clone()
                                                ).set(chrono::Utc::now().timestamp_millis() as f64);
                                            }
                                            tracing::info!("Schedule {} executed successfully: {:?}", schedule_id_clone, result.status);
                                        }
                                        Err(e) => {
                                            #[cfg(feature = "scheduler-telemetry")]
                                            counter!("mofa_scheduler_executions_total",
                                                "schedule_id" => schedule_id_clone.clone(),
                                                "agent_id" => agent_id_clone.clone(),
                                                "status" => "error"
                                            ).increment(1);
                                            tracing::error!("Schedule {} execution failed: {}", schedule_id_clone, e);
                                        }
                                    }

                                    // Record active run completion
                                    #[cfg(feature = "scheduler-telemetry")]
                                    gauge!("mofa_scheduler_active_runs", "schedule_id" => schedule_id_clone).decrement(1.0);

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

    #[tokio::test]
    async fn test_next_run_time_calculation() {
        use std::sync::Arc;
        use tokio::sync::Semaphore;

        // Test interval schedule
        let def = ScheduleDefinition::new_interval(
            "test-interval",
            "test-agent",
            5000, // 5 seconds
            1,
            AgentInput::text("test"),
            MissedTickPolicy::Skip,
        ).unwrap();

        let entry = ScheduleEntry::new(
            def,
            tokio::spawn(async {}), // dummy handle
            Arc::new(Semaphore::new(1)),
        );

        let mock_clock = MockClock { current_time: 1000000 }; // 1M ms
        let info = entry.to_info(&mock_clock);

        // Should calculate next run as current time + interval since no last run
        assert_eq!(info.next_run_ms, Some(1000000 + 5000));

        // Test with last run time
        entry.last_run_ms.store(900000, Ordering::Relaxed); // 900K ms
        let info2 = entry.to_info(&mock_clock);
        assert_eq!(info2.next_run_ms, Some(900000 + 5000));

        // Test paused schedule
        entry.paused.store(true, Ordering::Release);
        let info3 = entry.to_info(&mock_clock);
        assert_eq!(info3.next_run_ms, None); // No next run when paused
    }

    // Mock clock for testing
    struct MockClock {
        current_time: u64,
    }

    impl Clock for MockClock {
        fn now_millis(&self) -> u64 {
            self.current_time
        }
    }
}
// Memory-budgeted scheduler for inference orchestration
//
// This module provides admission control under memory constraints for inference
// requests. It is **architecturally separate** from the adapter registry
// (`adapter/`) because scheduling is a dynamic runtime concern, while adapter
// discovery is a static capability resolution concern.
//
// # Architecture
//
// ```text
// ┌─────────────────────────┐
// │   Adapter Registry      │  ← static: "which backends can run this model?"
// │   (adapter/)            │
// └──────────┬──────────────┘
//            │ candidates
//            ▼
// ┌─────────────────────────┐
// │   Memory Scheduler      │  ← dynamic: "should we admit this request now?"
// │   (scheduler/)          │
// └──────────┬──────────────┘
//            │ Accept / Defer / Reject
//            ▼
// ┌─────────────────────────┐
// │   Inference Execution   │
// └─────────────────────────┘
// ```
//
// # Phase 1: Rule-based baseline
//
// - `AdmissionDecision`: Accept / Defer / Reject with structured metadata
// - `MemoryPolicy`: deterministic threshold-based admission control
// - `StabilityControl`: cooldown/hysteresis to prevent profile thrashing
// - `DeferredQueue`: age-aware fairness for deferred requests
//
// # Example
//
// ```rust,ignore
// use mofa_foundation::scheduler::{MemoryScheduler, MemoryPolicy, MemoryBudget};
//
// let policy = MemoryPolicy::default();
// let budget = MemoryBudget::new(16_384); // 16 GB
// let mut scheduler = MemoryScheduler::new(policy, budget);
//
// let decision = scheduler.evaluate(2048); // request needs 2 GB
// match decision.outcome {
//     AdmissionOutcome::Accept => { scheduler.allocate(2048); }
//     AdmissionOutcome::Defer  => { scheduler.defer("req-1", 2048); }
//     AdmissionOutcome::Reject => { /* drop request */ }
// }
// ```

mod admission;
pub mod clock;
mod budget;
mod deferred;
mod stability;

pub use admission::{AdmissionDecision, AdmissionOutcome};
pub use budget::MemoryBudget;
pub use clock::SystemClock;
pub use deferred::{DeferredQueue, DeferredRequest};
pub use stability::StabilityControl;

use tracing::warn;

// ============================================================================
// Memory Policy
// ============================================================================

/// Threshold-based memory policy for admission control.
///
/// Defines three zones:
/// - **Accept zone**: usage ≤ `defer_threshold` → accept immediately
/// - **Defer zone**: `defer_threshold` < usage ≤ `reject_threshold` → queue for retry
/// - **Reject zone**: usage > `reject_threshold` → reject outright
#[derive(Debug, Clone)]
pub struct MemoryPolicy {
    /// Total memory capacity in MB.
    pub capacity_mb: u64,
    /// Fraction of capacity at which deferral begins (0.0–1.0).
    pub defer_at: f64,
    /// Fraction of capacity at which rejection begins (0.0–1.0).
    pub reject_at: f64,
    /// Maximum number of deferred requests.
    pub max_deferred: usize,
    /// Maximum retry attempts before a deferred request is rejected.
    pub max_retries: u32,
}

impl Default for MemoryPolicy {
    fn default() -> Self {
        Self {
            capacity_mb: 16_384, // 16 GB
            defer_at: 0.75,      // defer above 75%
            reject_at: 0.90,     // reject above 90%
            max_deferred: 100,
            max_retries: 3,
        }
    }
}

impl MemoryPolicy {
    /// Create a policy with explicit capacity and thresholds.
    pub fn new(capacity_mb: u64, defer_at: f64, reject_at: f64) -> Self {
        Self {
            capacity_mb,
            defer_at: defer_at.clamp(0.0, 1.0),
            reject_at: reject_at.clamp(defer_at, 1.0),
            ..Default::default()
        }
    }

    /// Absolute MB threshold for deferral.
    pub fn defer_threshold_mb(&self) -> u64 {
        (self.capacity_mb as f64 * self.defer_at) as u64
    }

    /// Absolute MB threshold for rejection.
    pub fn reject_threshold_mb(&self) -> u64 {
        (self.capacity_mb as f64 * self.reject_at) as u64
    }
}

// ============================================================================
// Memory Scheduler
// ============================================================================

/// The memory-budgeted scheduler.
///
/// Combines a `MemoryPolicy`, `MemoryBudget`, `StabilityControl`, and
/// `DeferredQueue` to provide admission control for inference requests.
#[derive(Debug)]
pub struct MemoryScheduler {
    policy: MemoryPolicy,
    budget: MemoryBudget,
    stability: StabilityControl,
    deferred: DeferredQueue,
    active_count: usize,
}

impl MemoryScheduler {
    /// Create a new scheduler with the given policy and budget.
    pub fn new(policy: MemoryPolicy, budget: MemoryBudget) -> Self {
        let max_deferred = policy.max_deferred;
        let max_retries = policy.max_retries;
        Self {
            policy,
            budget,
            stability: StabilityControl::default(),
            deferred: DeferredQueue::new(max_deferred, max_retries),
            active_count: 0,
        }
    }

    /// Create a scheduler with default policy for a given total memory.
    pub fn with_capacity(capacity_mb: u64) -> Self {
        let policy = MemoryPolicy {
            capacity_mb,
            ..Default::default()
        };
        let budget = MemoryBudget::new(capacity_mb);
        Self::new(policy, budget)
    }

    /// Evaluate whether a request requiring `required_mb` should be admitted.
    ///
    /// This is a **read-only** check — it does not allocate memory.
    /// Call `allocate()` after an `Accept` decision to actually reserve memory.
    pub fn evaluate(&self, required_mb: u64) -> AdmissionDecision {
        let current = self.budget.used_mb();
        let projected = current + required_mb;
        let available = self.budget.available_mb();

        if projected > self.policy.reject_threshold_mb() {
            AdmissionDecision {
                outcome: AdmissionOutcome::Reject,
                reason: format!(
                    "Projected usage {}MB exceeds reject threshold {}MB",
                    projected,
                    self.policy.reject_threshold_mb()
                ),
                current_usage_mb: current,
                required_mb,
                available_mb: available,
            }
        } else if projected > self.policy.defer_threshold_mb() {
            AdmissionDecision {
                outcome: AdmissionOutcome::Defer,
                reason: format!(
                    "Projected usage {}MB exceeds defer threshold {}MB",
                    projected,
                    self.policy.defer_threshold_mb()
                ),
                current_usage_mb: current,
                required_mb,
                available_mb: available,
            }
        } else {
            AdmissionDecision {
                outcome: AdmissionOutcome::Accept,
                reason: "Within budget".to_string(),
                current_usage_mb: current,
                required_mb,
                available_mb: available,
            }
        }
    }

    /// Allocate memory for an accepted request.
    ///
    /// Returns `true` if allocation succeeded, `false` if insufficient memory.
    pub fn allocate(&mut self, amount_mb: u64) -> bool {
        if self.budget.allocate(amount_mb) {
            self.active_count += 1;
            true
        } else {
            false
        }
    }

    /// Release memory when a request completes.
    pub fn release(&mut self, amount_mb: u64) {
        self.budget.release(amount_mb);
        self.active_count = self.active_count.saturating_sub(1);
    }

    /// Defer a request (add to the fairness queue).
    ///
    /// Returns `true` if the request was queued, `false` if the queue is full.
    pub fn defer(&mut self, id: impl Into<String>, required_mb: u64) -> bool {
        let request = DeferredRequest::new(id.into(), required_mb);
        let ok = self.deferred.enqueue(request);
        if !ok {
            warn!("Deferred queue is full, cannot defer request");
        }
        ok
    }

    /// Try to process the next deferred request that fits in available memory.
    ///
    /// Uses **age-aware** fairness: oldest request that fits is dequeued first,
    /// preventing starvation of small requests behind large ones.
    pub fn try_dequeue(&mut self) -> Option<DeferredRequest> {
        let available = self.budget.available_mb();
        self.deferred.dequeue_oldest_fitting(available)
    }

    /// Drain expired requests (exceeded max retries).
    pub fn drain_expired(&mut self) -> Vec<DeferredRequest> {
        self.deferred.drain_expired()
    }

    /// Check if the stability control allows a profile switch.
    pub fn can_switch_profile(&self) -> bool {
        self.stability.can_switch()
    }

    /// Record a profile switch for stability cooldown.
    pub fn record_switch(&mut self) {
        self.stability.record_switch();
    }

    /// Check if a memory change is significant (exceeds hysteresis threshold).
    pub fn is_significant_change(&self, new_usage_mb: u64) -> bool {
        self.stability.is_significant_change(new_usage_mb)
    }

    /// Update the stability control's memory reading.
    pub fn update_memory_reading(&mut self, usage_mb: u64) {
        self.stability.update_reading(usage_mb);
    }

    // -- Accessors --

    /// Current memory usage in MB.
    pub fn used_mb(&self) -> u64 {
        self.budget.used_mb()
    }

    /// Available memory in MB.
    pub fn available_mb(&self) -> u64 {
        self.budget.available_mb()
    }

    /// Usage as a percentage (0.0–100.0).
    pub fn usage_percent(&self) -> f64 {
        self.budget.usage_percent()
    }

    /// Number of currently active requests.
    pub fn active_count(&self) -> usize {
        self.active_count
    }

    /// Number of deferred requests waiting in the queue.
    pub fn deferred_count(&self) -> usize {
        self.deferred.len()
    }

    /// Get the policy reference.
    pub fn policy(&self) -> &MemoryPolicy {
        &self.policy
    }
}
