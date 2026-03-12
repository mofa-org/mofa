//! Cron-based scheduler for periodic agent execution.
//!
//! Provides [`CronScheduler`], a concrete implementation of [`AgentScheduler`] that
//! supports both cron expressions and fixed-interval scheduling with bounded concurrency.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use async_trait::async_trait;
use chrono::Utc;
use cron::Schedule;
use tokio::sync::{RwLock, Semaphore, oneshot};
use tokio::task::JoinHandle;
use tokio::time::{Duration, interval};

use mofa_kernel::scheduler::{
    AgentScheduler, Clock, MissedTickPolicy, ScheduleDefinition, ScheduleHandle, ScheduleInfo,
    ScheduledAgentRunner, SchedulerError,
};

use super::clock::SystemClock;

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
    ///
    /// Uses [`AtomicU64`] to correctly represent current epoch values
    /// (~1.7 trillion ms), which far exceed [`u32::MAX`] (~4.3 billion ms ≈ 49 days).
    last_run_ms: Arc<AtomicU64>,
    /// Consecutive failure count.
    consecutive_failures: Arc<AtomicU32>,
}

impl ScheduleEntry {
    fn new(
        definition: ScheduleDefinition,
        task_handle: JoinHandle<()>,
        semaphore: Arc<Semaphore>,
        last_run_ms: Arc<AtomicU64>,
    ) -> Self {
        Self {
            semaphore,
            paused: Arc::new(AtomicBool::new(false)),
            task_handle,
            last_run_ms,
            consecutive_failures: Arc::new(AtomicU32::new(0)),
            definition,
        }
    }

    /// Convert to a monitoring snapshot.
    fn to_info(&self, clock: &dyn Clock) -> ScheduleInfo {
        let last_run_raw = self.last_run_ms.load(Ordering::Relaxed);
        let last_run = if last_run_raw == 0 {
            None
        } else {
            Some(last_run_raw)
        };
        ScheduleInfo::new(
            self.definition.schedule_id.clone(),
            self.definition.agent_id.clone(),
            self.calculate_next_run_ms(clock),
            last_run,
            self.consecutive_failures.load(Ordering::Relaxed),
            self.paused.load(Ordering::Acquire),
        )
    }

    /// Compute when this schedule will next fire.
    ///
    /// Returns `None` if:
    /// - The schedule is paused.
    /// - The cron expression produces no future occurrences.
    fn calculate_next_run_ms(&self, clock: &dyn Clock) -> Option<u64> {
        if self.paused.load(Ordering::Acquire) {
            return None;
        }

        if let Some(cron_expr) = &self.definition.cron_expression {
            // Parse expression and find the next UTC occurrence.
            cron_expr.parse::<Schedule>().ok().and_then(|sched| {
                sched
                    .upcoming(Utc)
                    .next()
                    .map(|dt| u64::try_from(dt.timestamp_millis()).unwrap_or(u64::MAX))
            })
        } else if let Some(interval_ms) = self.definition.interval_ms {
            // next = last_run + interval; on first tick use now + interval.
            let last = self.last_run_ms.load(Ordering::Relaxed);
            let base = if last == 0 { clock.now_millis() } else { last };
            Some(base.saturating_add(interval_ms))
        } else {
            None
        }
    }
}

impl Drop for ScheduleEntry {
    fn drop(&mut self) {
        self.task_handle.abort();
    }
}

// ============================================================================
// CronScheduler - Main implementation
// ============================================================================

/// A concrete implementation of [`AgentScheduler`] that supports cron expressions
/// and interval-based scheduling with bounded concurrency.
///
/// # Concurrency Control
///
/// Uses a two-level semaphore system:
/// - **Global semaphore**: Caps total concurrent executions across all schedules.
/// - **Per-schedule semaphore**: Enforces `max_concurrent` per schedule.
///
/// # Scheduling Modes
///
/// - **Interval**: Uses `tokio::time::interval` for fixed-period scheduling.
/// - **Cron**: Uses the `cron` crate to compute next execution times.
///
/// # Example
///
/// ```rust,ignore
/// use mofa_foundation::scheduler::CronScheduler;
/// use mofa_kernel::scheduler::{ScheduleDefinition, MissedTickPolicy};
/// use mofa_kernel::agent::types::AgentInput;
///
/// let scheduler = CronScheduler::new(engine, 10);
///
/// let handle = scheduler.register(
///     ScheduleDefinition::new_cron(
///         "report-gen",
///         "reporting-agent",
///         "0 */5 * * * *", // every 5 minutes
///         1,
///         AgentInput::text("generate report"),
///         MissedTickPolicy::Skip,
///     ).unwrap()
/// ).await.unwrap();
/// ```
pub struct CronScheduler {
    /// Runner used to execute agents on each scheduled tick.
    runner: Arc<dyn ScheduledAgentRunner>,
    /// Global semaphore capping total concurrent agent executions.
    global_semaphore: Arc<Semaphore>,
    /// Map of schedule_id → internal schedule state.
    schedules: Arc<RwLock<HashMap<String, ScheduleEntry>>>,
    /// Clock for time operations (injectable for testing).
    clock: Arc<dyn Clock>,
}

impl CronScheduler {
    /// Create a new scheduler with the given runner and global concurrency limit.
    ///
    /// # Parameters
    ///
    /// - `runner`: Implementation of [`ScheduledAgentRunner`] used to execute agents.
    /// - `global_max_concurrent`: Maximum total concurrent agent executions across all
    ///   schedules. Pass `usize::MAX` to disable the global limit.
    ///
    /// # Panics
    ///
    /// Panics if `global_max_concurrent` is 0.
    pub fn new(runner: Arc<dyn ScheduledAgentRunner>, global_max_concurrent: usize) -> Self {
        assert!(
            global_max_concurrent > 0,
            "global_max_concurrent must be > 0"
        );
        Self {
            runner,
            global_semaphore: Arc::new(Semaphore::new(global_max_concurrent)),
            schedules: Arc::new(RwLock::new(HashMap::new())),
            clock: Arc::new(SystemClock),
        }
    }

    /// Create a scheduler with a custom clock (for testing).
    #[cfg(test)]
    pub(super) fn with_clock(
        runner: Arc<dyn ScheduledAgentRunner>,
        global_max_concurrent: usize,
        clock: Arc<dyn Clock>,
    ) -> Self {
        assert!(
            global_max_concurrent > 0,
            "global_max_concurrent must be > 0"
        );
        Self {
            runner,
            global_semaphore: Arc::new(Semaphore::new(global_max_concurrent)),
            schedules: Arc::new(RwLock::new(HashMap::new())),
            clock,
        }
    }
}

// ============================================================================
// AgentScheduler trait implementation
// ============================================================================

impl AgentScheduler for CronScheduler {
    async fn register(&self, def: ScheduleDefinition) -> Result<ScheduleHandle, SchedulerError> {
        // Validate cron expression up-front so the error is immediate.
        if let Some(cron_expr) = &def.cron_expression {
            if let Err(e) = cron_expr.parse::<Schedule>() {
                return Err(SchedulerError::InvalidCron(
                    cron_expr.clone(),
                    e.to_string(),
                ));
            }
        }

        // Reject duplicate schedule IDs.
        {
            let schedules = self.schedules.read().await;
            if schedules.contains_key(&def.schedule_id) {
                return Err(SchedulerError::AlreadyExists(def.schedule_id));
            }
        }

        let per_schedule_semaphore = Arc::new(Semaphore::new(def.max_concurrent));
        // Create the shared AtomicU64 before spawning so both the task and the
        // monitoring entry point to the same counter.
        let last_run_ms = Arc::new(AtomicU64::new(0));
        let (cancel_tx, cancel_rx) = oneshot::channel();
        let schedule_id = def.schedule_id.clone();

        let task_handle = self.spawn_schedule_task(
            def.clone(),
            cancel_rx,
            Arc::clone(&per_schedule_semaphore),
            Arc::clone(&last_run_ms),
        );

        let entry = ScheduleEntry::new(def, task_handle, per_schedule_semaphore, last_run_ms);
        {
            let mut schedules = self.schedules.write().await;
            schedules.insert(entry.definition.schedule_id.clone(), entry);
        }

        Ok(ScheduleHandle::new(schedule_id, cancel_tx))
    }

    async fn unregister(&self, schedule_id: &str) -> Result<(), SchedulerError> {
        let mut schedules = self.schedules.write().await;
        let entry = schedules
            .remove(schedule_id)
            .ok_or_else(|| SchedulerError::NotFound(schedule_id.to_string()))?;
        drop(entry); // Drop aborts the background task via the Drop impl.
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

    async fn resume(&self, schedule_id: &str) -> Result<(), SchedulerError> {
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
    fn spawn_schedule_task(
        &self,
        def: ScheduleDefinition,
        mut cancel_rx: oneshot::Receiver<()>,
        per_schedule_semaphore: Arc<Semaphore>,
        last_run_ms: Arc<AtomicU64>,
    ) -> JoinHandle<()> {
        let runner = Arc::clone(&self.runner);
        let global_semaphore = Arc::clone(&self.global_semaphore);
        let schedule_id = def.schedule_id.clone();
        let agent_id = def.agent_id.clone();
        let input_template = def.input_template.clone();
        let cron_expression = def.cron_expression.clone();
        let interval_ms = def.interval_ms;

        tokio::spawn(async move {
            let mut timing = if let Some(cron_expr) = &cron_expression {
                ScheduleTiming::Cron(cron_expr.parse().unwrap())
            } else if let Some(ms) = interval_ms {
                ScheduleTiming::Interval(interval(Duration::from_millis(ms)))
            } else {
                return; // prevented by ScheduleDefinition constructors
            };

            loop {
                tokio::select! {
                    _ = &mut cancel_rx => {
                        tracing::debug!("Schedule {} cancelled", schedule_id);
                        return;
                    }

                    tick_result = timing.next_tick() => {
                        match tick_result {
                            Ok(()) => {
                                let global_permit =
                                    match Arc::clone(&global_semaphore).try_acquire_owned() {
                                        Ok(p) => p,
                                        Err(_) => {
                                            tracing::debug!(
                                                "Global concurrency limit reached for schedule {}",
                                                schedule_id
                                            );
                                            continue;
                                        }
                                    };

                                let schedule_permit =
                                    match Arc::clone(&per_schedule_semaphore).try_acquire_owned() {
                                        Ok(p) => p,
                                        Err(_) => {
                                            tracing::debug!(
                                                "Per-schedule concurrency limit reached for schedule {}",
                                                schedule_id
                                            );
                                            drop(global_permit);
                                            continue;
                                        }
                                    };

                                let runner_clone = Arc::clone(&runner);
                                let agent_id_clone = agent_id.clone();
                                let input_clone = input_template.clone();
                                let schedule_id_clone = schedule_id.clone();
                                let last_run_ms_clone = Arc::clone(&last_run_ms);

                                tokio::spawn(async move {
                                    match runner_clone
                                        .run_scheduled(&agent_id_clone, input_clone)
                                        .await
                                    {
                                        Ok(()) => {
                                            let now = u64::try_from(
                                                Utc::now().timestamp_millis(),
                                            )
                                            .unwrap_or(u64::MAX);
                                            last_run_ms_clone.store(now, Ordering::Relaxed);
                                            tracing::info!(
                                                "Schedule {} executed successfully",
                                                schedule_id_clone
                                            );
                                        }
                                        Err(e) => {
                                            tracing::error!(
                                                "Schedule {} execution failed: {}",
                                                schedule_id_clone,
                                                e
                                            );
                                        }
                                    }
                                    // Permits released on drop.
                                    drop(schedule_permit);
                                    drop(global_permit);
                                });
                            }
                            Err(e) => {
                                tracing::error!(
                                    "Timing error for schedule {}: {}",
                                    schedule_id,
                                    e
                                );
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

enum ScheduleTiming {
    Interval(tokio::time::Interval),
    Cron(Schedule),
}

impl ScheduleTiming {
    async fn next_tick(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        match self {
            ScheduleTiming::Interval(iv) => {
                iv.tick().await;
                Ok(())
            }
            ScheduleTiming::Cron(schedule) => {
                let now = Utc::now();
                if let Some(next) = schedule.upcoming(Utc).next() {
                    let duration = next.signed_duration_since(now);
                    if duration > chrono::Duration::zero() {
                        tokio::time::sleep(duration.to_std()?).await;
                    }
                } else {
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
    use std::sync::Arc;
    use std::sync::atomic::Ordering;
    use tokio::sync::Semaphore;

    use super::*;
    use mofa_kernel::agent::types::AgentInput;

    struct MockRunner;

    #[async_trait]
    impl ScheduledAgentRunner for MockRunner {
        async fn run_scheduled(
            &self,
            _agent_id: &str,
            _input: AgentInput,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            Ok(())
        }
    }

    fn make_test_scheduler(global_cap: usize) -> CronScheduler {
        CronScheduler::new(Arc::new(MockRunner), global_cap)
    }

    #[tokio::test]
    async fn test_register_duplicate_schedule() {
        let scheduler = make_test_scheduler(10);

        let _handle = scheduler
            .register(
                ScheduleDefinition::new_interval(
                    "dup",
                    "agent",
                    1000,
                    1,
                    AgentInput::text("x"),
                    MissedTickPolicy::Skip,
                )
                .unwrap(),
            )
            .await
            .unwrap();

        let result = scheduler
            .register(
                ScheduleDefinition::new_interval(
                    "dup",
                    "agent",
                    1000,
                    1,
                    AgentInput::text("x"),
                    MissedTickPolicy::Skip,
                )
                .unwrap(),
            )
            .await;

        assert!(matches!(result, Err(SchedulerError::AlreadyExists(id)) if id == "dup"));
    }

    #[tokio::test]
    async fn test_register_invalid_cron() {
        let scheduler = make_test_scheduler(10);
        let result = scheduler
            .register(
                ScheduleDefinition::new_cron(
                    "test",
                    "agent",
                    "not a cron",
                    1,
                    AgentInput::text("x"),
                    MissedTickPolicy::Skip,
                )
                .unwrap(),
            )
            .await;
        assert!(matches!(result, Err(SchedulerError::InvalidCron(_, _))));
    }

    #[tokio::test]
    async fn test_list_empty() {
        assert!(make_test_scheduler(10).list().await.is_empty());
    }

    #[tokio::test]
    async fn test_pause_nonexistent() {
        let r = make_test_scheduler(10).pause("x").await;
        assert!(matches!(r, Err(SchedulerError::NotFound(id)) if id == "x"));
    }

    #[tokio::test]
    async fn test_resume_nonexistent() {
        let r = make_test_scheduler(10).resume("x").await;
        assert!(matches!(r, Err(SchedulerError::NotFound(id)) if id == "x"));
    }

    #[tokio::test]
    async fn test_unregister_nonexistent() {
        let r = make_test_scheduler(10).unregister("x").await;
        assert!(matches!(r, Err(SchedulerError::NotFound(id)) if id == "x"));
    }

    #[tokio::test]
    async fn test_next_run_time_calculation() {
        struct FixedClock(u64);
        impl Clock for FixedClock {
            fn now_millis(&self) -> u64 {
                self.0
            }
        }

        let clock = FixedClock(1_000_000);

        let def = ScheduleDefinition::new_interval(
            "t",
            "agent",
            5_000,
            1,
            AgentInput::text("x"),
            MissedTickPolicy::Skip,
        )
        .unwrap();

        let last_run_ms = Arc::new(AtomicU64::new(0));
        let entry = ScheduleEntry::new(
            def,
            tokio::spawn(async {}),
            Arc::new(Semaphore::new(1)),
            Arc::clone(&last_run_ms),
        );

        // Never-run: next = now + interval.
        let info = entry.to_info(&clock);
        assert_eq!(info.next_run_ms, Some(1_000_000 + 5_000));
        assert_eq!(info.last_run_ms, None);

        // After a tick: next = last_run + interval.
        last_run_ms.store(900_000, Ordering::Relaxed);
        let info2 = entry.to_info(&clock);
        assert_eq!(info2.next_run_ms, Some(900_000 + 5_000));
        assert_eq!(info2.last_run_ms, Some(900_000));

        // Paused: no next_run_ms.
        entry.paused.store(true, Ordering::Release);
        assert_eq!(entry.to_info(&clock).next_run_ms, None);
    }
}
