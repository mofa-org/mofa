//! Swarm DAG schedulers (sequential + parallel).

use std::collections::{HashSet, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use futures::future::{BoxFuture, join_all};
use petgraph::graph::NodeIndex;
use serde::{Deserialize, Serialize};
use tokio::sync::Semaphore;
use tokio::time::timeout;
use tracing::{error, info, instrument, warn};

use mofa_kernel::agent::types::error::{GlobalError, GlobalResult};

use crate::swarm::{CoordinationPattern, SubtaskDAG, SwarmSubtask};

/// task executor used by schedulers
pub type SubtaskExecutorFn =
    Arc<dyn Fn(NodeIndex, SwarmSubtask) -> BoxFuture<'static, GlobalResult<String>> + Send + Sync>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskOutcome {
    Success(String),
    Failure(String),
    Skipped(String),
}

impl TaskOutcome {
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success(_))
    }

    pub fn output(&self) -> Option<&str> {
        if let Self::Success(s) = self {
            Some(s)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskExecutionResult {
    pub task_id: String,
    pub node_index: usize,
    pub outcome: TaskOutcome,
    pub wall_time: Duration,
    /// Microseconds elapsed from scheduler start until this task began executing.
    /// Zero for skipped tasks. Provides microsecond precision for accurate peak_concurrency computation.
    #[serde(default)]
    pub start_offset_us: u64,
    pub attempt: u32,
}

impl TaskExecutionResult {
    fn success(
        task: &SwarmSubtask,
        idx: NodeIndex,
        output: String,
        elapsed: Duration,
        start_offset_us: u64,
    ) -> Self {
        Self {
            task_id: task.id.clone(),
            node_index: idx.index(),
            outcome: TaskOutcome::Success(output),
            wall_time: elapsed,
            start_offset_us,
            attempt: 1,
        }
    }

    fn failure(
        task: &SwarmSubtask,
        idx: NodeIndex,
        error: String,
        elapsed: Duration,
        start_offset_us: u64,
    ) -> Self {
        Self {
            task_id: task.id.clone(),
            node_index: idx.index(),
            outcome: TaskOutcome::Failure(error),
            wall_time: elapsed,
            start_offset_us,
            attempt: 1,
        }
    }

    fn skipped(task: &SwarmSubtask, idx: NodeIndex, reason: String) -> Self {
        Self {
            task_id: task.id.clone(),
            node_index: idx.index(),
            outcome: TaskOutcome::Skipped(reason),
            wall_time: Duration::ZERO,
            start_offset_us: 0,
            attempt: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerSummary {
    pub pattern: CoordinationPattern,
    pub total_tasks: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub skipped: usize,
    pub total_wall_time: Duration,
    pub results: Vec<TaskExecutionResult>,
}

impl SchedulerSummary {
    pub fn success_rate(&self) -> f64 {
        if self.total_tasks == 0 {
            return 0.0;
        }
        self.succeeded as f64 / self.total_tasks as f64
    }

    pub fn is_fully_successful(&self) -> bool {
        self.failed == 0 && self.skipped == 0
    }

    pub fn successful_outputs(&self) -> Vec<&str> {
        self.results
            .iter()
            .filter_map(|r| r.outcome.output())
            .collect()
    }

    /// Returns results sorted by `start_offset_us` — the order tasks actually began executing.
    /// Skipped tasks are excluded since they have `start_offset_us = 0` and never ran.
    pub fn timeline(&self) -> Vec<&TaskExecutionResult> {
        let mut ordered: Vec<&TaskExecutionResult> = self
            .results
            .iter()
            .filter(|r| !matches!(r.outcome, TaskOutcome::Skipped(_)))
            .collect();
        ordered.sort_by_key(|r| r.start_offset_us);
        ordered
    }

    /// Maximum number of tasks that were executing concurrently at any instant.
    /// Computed from `start_offset_us` and `wall_time` (microsecond precision internally);
    /// skipped tasks are excluded.
    pub fn peak_concurrency(&self) -> usize {
        let mut events: Vec<(u64, i32)> = Vec::new();
        for r in &self.results {
            if matches!(r.outcome, TaskOutcome::Skipped(_)) {
                continue;
            }
            let start = r.start_offset_us;
            let duration = u64::try_from(r.wall_time.as_micros().max(1)).unwrap_or(u64::MAX);
            let end = start + duration;
            events.push((start, 1));
            events.push((end, -1));
        }
        if events.is_empty() {
            return 0;
        }
        events.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
        let mut current = 0i32;
        let mut peak = 0i32;
        for (_, delta) in events {
            current += delta;
            peak = peak.max(current);
        }
        peak as usize
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FailurePolicy {
    #[default]
    Continue,

    FailFastCascade,
}

#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct SwarmSchedulerConfig {
    pub task_timeout: Duration,
    pub failure_policy: FailurePolicy,
    pub concurrency_limit: Option<usize>,
    /// When `true` the scheduler validates DAG topology and returns a summary showing
    /// which tasks would execute in what order — without calling the executor at all.
    pub dry_run: bool,
}

impl Default for SwarmSchedulerConfig {
    fn default() -> Self {
        Self {
            task_timeout: Duration::from_secs(120),
            failure_policy: FailurePolicy::default(),
            concurrency_limit: None,
            dry_run: false,
        }
    }
}

#[async_trait]
pub trait SwarmScheduler: Send + Sync {
    fn pattern(&self) -> CoordinationPattern;

    async fn execute(
        &self,
        dag: &mut SubtaskDAG,
        executor: SubtaskExecutorFn,
    ) -> GlobalResult<SchedulerSummary>;
}

pub struct SequentialScheduler {
    pub config: SwarmSchedulerConfig,
}

impl SequentialScheduler {
    pub fn new() -> Self {
        Self {
            config: SwarmSchedulerConfig::default(),
        }
    }

    pub fn with_config(config: SwarmSchedulerConfig) -> Self {
        Self { config }
    }
}

impl Default for SequentialScheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SwarmScheduler for SequentialScheduler {
    fn pattern(&self) -> CoordinationPattern {
        CoordinationPattern::Sequential
    }

    #[instrument(
        skip(self, dag, executor),
        fields(pattern = "sequential", task_count = dag.task_count())
    )]
    async fn execute(
        &self,
        dag: &mut SubtaskDAG,
        executor: SubtaskExecutorFn,
    ) -> GlobalResult<SchedulerSummary> {
        let wall_start = Instant::now();
        let total = dag.task_count();

        let ordered_indices = dag.topological_order().map_err(|e| {
            GlobalError::runtime(format!("Sequential scheduling failed, DAG error: {e}"))
        })?;

        if self.config.dry_run {
            let mut results = Vec::with_capacity(ordered_indices.len());
            for &idx in &ordered_indices {
                let task = dag.get_task(idx).expect("missing idx").clone();
                results.push(TaskExecutionResult::skipped(&task, idx, "dry_run".into()));
            }
            return Ok(SchedulerSummary {
                pattern: CoordinationPattern::Sequential,
                total_tasks: total,
                succeeded: 0,
                failed: 0,
                skipped: total,
                total_wall_time: Duration::ZERO,
                results,
            });
        }

        let mut results = Vec::with_capacity(total);
        let mut succeeded = 0usize;
        let mut failed = 0usize;
        let mut skipped = 0usize;

        info!(task_count = total, "Sequential scheduler starting");

        for idx in ordered_indices {
            let task = &dag.get_task(idx).expect("Task missing by index");

            if !matches!(
                task.status,
                crate::swarm::SubtaskStatus::Pending | crate::swarm::SubtaskStatus::Ready
            ) {
                if matches!(task.status, crate::swarm::SubtaskStatus::Skipped) {
                    skipped += 1;
                    results.push(TaskExecutionResult::skipped(
                        task,
                        idx,
                        "skipped by cascade".into(),
                    ));
                    continue;
                }
                continue;
            }

            dag.mark_running(idx);
            let task_snapshot = dag.get_task(idx).unwrap().clone();
            let task_id = task_snapshot.id.clone();

            let offset_ms = wall_start.elapsed().as_micros() as u64;
            let start = Instant::now();
            info!(task_id = %task_id, task_desc = %task_snapshot.description, "Executing node");

            let outcome = timeout(
                self.config.task_timeout,
                executor(idx, task_snapshot.clone()),
            )
            .await;
            let elapsed = start.elapsed();

            match outcome {
                Ok(Ok(output)) => {
                    info!(task_id = %task_id, elapsed_ms = elapsed.as_millis(), "Succeeded");
                    dag.mark_complete_with_output(idx, Some(output.clone()));
                    results.push(TaskExecutionResult::success(
                        &task_snapshot,
                        idx,
                        output,
                        elapsed,
                        offset_ms,
                    ));
                    succeeded += 1;
                }
                Ok(Err(e)) => {
                    error!(task_id = %task_id, error = %e, "Failed");
                    let err_str = e.to_string();
                    dag.mark_failed(idx, err_str.clone());
                    results.push(TaskExecutionResult::failure(
                        &task_snapshot,
                        idx,
                        err_str,
                        elapsed,
                        offset_ms,
                    ));
                    failed += 1;

                    if self.config.failure_policy == FailurePolicy::FailFastCascade {
                        let newly_skipped = dag.cascade_skip(idx);
                        info!(cascaded = newly_skipped, "cascaded skip");
                    }
                }
                Err(_) => {
                    let msg = format!("timed out after {:?}", self.config.task_timeout);
                    error!(task_id = %task_id, "{msg}");
                    dag.mark_failed(idx, msg.clone());
                    results.push(TaskExecutionResult::failure(
                        &task_snapshot,
                        idx,
                        msg,
                        elapsed,
                        offset_ms,
                    ));
                    failed += 1;

                    if self.config.failure_policy == FailurePolicy::FailFastCascade {
                        dag.cascade_skip(idx);
                    }
                }
            }
        }

        Ok(SchedulerSummary {
            pattern: CoordinationPattern::Sequential,
            total_tasks: total,
            succeeded,
            failed,
            skipped,
            total_wall_time: wall_start.elapsed(),
            results,
        })
    }
}

pub struct ParallelScheduler {
    pub config: SwarmSchedulerConfig,
}

impl ParallelScheduler {
    pub fn new() -> Self {
        Self {
            config: SwarmSchedulerConfig::default(),
        }
    }

    pub fn with_config(config: SwarmSchedulerConfig) -> Self {
        Self { config }
    }
}

impl Default for ParallelScheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SwarmScheduler for ParallelScheduler {
    fn pattern(&self) -> CoordinationPattern {
        CoordinationPattern::Parallel
    }

    #[instrument(
        skip(self, dag, executor),
        fields(pattern = "parallel", task_count = dag.task_count())
    )]
    async fn execute(
        &self,
        dag: &mut SubtaskDAG,
        executor: SubtaskExecutorFn,
    ) -> GlobalResult<SchedulerSummary> {
        let wall_start = Instant::now();
        let total = dag.task_count();

        if self.config.dry_run {
            let ordered = dag.topological_order().unwrap_or_default();
            let mut results = Vec::with_capacity(ordered.len());
            for &idx in &ordered {
                let task = dag.get_task(idx).expect("missing idx").clone();
                results.push(TaskExecutionResult::skipped(&task, idx, "dry_run".into()));
            }
            return Ok(SchedulerSummary {
                pattern: self.pattern(),
                total_tasks: total,
                succeeded: 0,
                failed: 0,
                skipped: total,
                total_wall_time: Duration::ZERO,
                results,
            });
        }

        let semaphore = self
            .config
            .concurrency_limit
            .map(|n| Arc::new(Semaphore::new(n)));

        let mut results: Vec<TaskExecutionResult> = Vec::with_capacity(total);
        let mut succeeded = 0usize;
        let mut failed = 0usize;
        let mut skipped = 0usize;

        info!(
            task_count = total,
            concurrency_limit = ?self.config.concurrency_limit,
            "Parallel scheduler starting"
        );

        while !dag.is_complete() {
            let ready_indices = dag.ready_tasks();

            if ready_indices.is_empty() {
                warn!("DAG stalled: incomplete, but 0 ready tasks");
                break;
            }

            let wave_size = ready_indices.len();
            info!(wave_size, "dispatching wave");

            let mut wave_futures = Vec::with_capacity(wave_size);

            for &idx in &ready_indices {
                dag.mark_running(idx);
                let task_snapshot = dag.get_task(idx).expect("missing idx").clone();

                let exec = executor.clone();
                let sem = semaphore.clone();
                let timeout_dur = self.config.task_timeout;
                let offset_ms = wall_start.elapsed().as_micros() as u64;

                let fut = async move {
                    let _permit = if let Some(s) = sem {
                        Some(s.acquire_owned().await.expect("semaphore closed"))
                    } else {
                        None
                    };

                    let start = Instant::now();
                    match timeout(timeout_dur, exec(idx, task_snapshot.clone())).await {
                        Ok(Ok(output)) => TaskExecutionResult::success(
                            &task_snapshot,
                            idx,
                            output,
                            start.elapsed(),
                            offset_ms,
                        ),
                        Ok(Err(e)) => TaskExecutionResult::failure(
                            &task_snapshot,
                            idx,
                            e.to_string(),
                            start.elapsed(),
                            offset_ms,
                        ),
                        Err(_) => TaskExecutionResult::failure(
                            &task_snapshot,
                            idx,
                            format!("timed out after {:?}", timeout_dur),
                            start.elapsed(),
                            offset_ms,
                        ),
                    }
                };

                wave_futures.push(fut);
            }

            let wave_results = join_all(wave_futures).await;

            for result in wave_results {
                let idx = NodeIndex::new(result.node_index);

                if result.outcome.is_success() {
                    let text = result
                        .outcome
                        .output()
                        .expect("Success has output")
                        .to_string();
                    dag.mark_complete_with_output(idx, Some(text));
                    succeeded += 1;
                } else {
                    let err_msg = match &result.outcome {
                        TaskOutcome::Failure(e) => e.clone(),
                        TaskOutcome::Skipped(_) => {
                            unreachable!("Skipped state not emitted dynamically")
                        }
                        _ => unreachable!(),
                    };

                    dag.mark_failed(idx, err_msg);
                    failed += 1;

                    if self.config.failure_policy == FailurePolicy::FailFastCascade {
                        let cascaded = dag.cascade_skip(idx);
                        skipped += cascaded;
                        info!(cascaded, "cascaded skip");
                    }
                }

                results.push(result);
            }
        }

        for (idx, task) in dag.all_tasks() {
            if task.status == crate::swarm::SubtaskStatus::Pending {
                skipped += 1;
                results.push(TaskExecutionResult::skipped(
                    task,
                    idx,
                    "dag stalled".into(),
                ));
            } else if task.status == crate::swarm::SubtaskStatus::Skipped {
                if !results.iter().any(|r| r.node_index == idx.index()) {
                    results.push(TaskExecutionResult::skipped(task, idx, "skipped".into()));
                }
            }
        }

        Ok(SchedulerSummary {
            pattern: CoordinationPattern::Parallel,
            total_tasks: total,
            succeeded,
            failed,
            skipped,
            total_wall_time: wall_start.elapsed(),
            results,
        })
    }
}

fn inject_context(mut task: SwarmSubtask, label: &str, outputs: &[String]) -> SwarmSubtask {
    if outputs.is_empty() {
        return task;
    }
    let body = outputs.join("\n---\n");
    task.description = format!("{}\n\n## {}\n{}", task.description, label, body);
    task
}

async fn run_parallel_wave(
    indices: &[NodeIndex],
    dag: &mut SubtaskDAG,
    executor: &SubtaskExecutorFn,
    timeout_dur: Duration,
    sched_start: Instant,
    concurrency_limit: Option<usize>,
) -> Vec<TaskExecutionResult> {
    let semaphore = concurrency_limit.map(|n| Arc::new(Semaphore::new(n)));
    let mut futures = Vec::with_capacity(indices.len());

    for &idx in indices {
        dag.mark_running(idx);
        let task_snapshot = dag.get_task(idx).expect("missing idx").clone();
        let exec = executor.clone();
        let sem = semaphore.clone();
        let sched_start_copy = sched_start;

        let fut = async move {
            let _permit = if let Some(s) = sem {
                Some(s.acquire_owned().await.expect("semaphore closed"))
            } else {
                None
            };
            let offset_us = u64::try_from(sched_start_copy.elapsed().as_micros()).unwrap_or(u64::MAX);
            let start = Instant::now();
            match timeout(timeout_dur, exec(idx, task_snapshot.clone())).await {
                Ok(Ok(output)) => {
                    TaskExecutionResult::success(&task_snapshot, idx, output, start.elapsed(), offset_us)
                }
                Ok(Err(e)) => {
                    TaskExecutionResult::failure(&task_snapshot, idx, e.to_string(), start.elapsed(), offset_us)
                }
                Err(_) => TaskExecutionResult::failure(
                    &task_snapshot,
                    idx,
                    format!("timed out after {:?}", timeout_dur),
                    start.elapsed(),
                    offset_us,
                ),
            }
        };

        futures.push(fut);
    }

    join_all(futures).await
}

pub struct MapReduceScheduler {
    pub config: SwarmSchedulerConfig,
}

impl MapReduceScheduler {
    pub fn new() -> Self {
        Self {
            config: SwarmSchedulerConfig::default(),
        }
    }

    pub fn with_config(config: SwarmSchedulerConfig) -> Self {
        Self { config }
    }
}

impl Default for MapReduceScheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SwarmScheduler for MapReduceScheduler {
    fn pattern(&self) -> CoordinationPattern {
        CoordinationPattern::MapReduce
    }

    #[instrument(
        skip(self, dag, executor),
        fields(pattern = "map_reduce", task_count = dag.task_count())
    )]
    async fn execute(
        &self,
        dag: &mut SubtaskDAG,
        executor: SubtaskExecutorFn,
    ) -> GlobalResult<SchedulerSummary> {
        let wall_start = Instant::now();
        let total = dag.task_count();

        if self.config.dry_run {
            let ordered = dag.topological_order().unwrap_or_default();
            let mut results = Vec::with_capacity(ordered.len());
            for &idx in &ordered {
                let task = dag.get_task(idx).expect("missing idx").clone();
                results.push(TaskExecutionResult::skipped(&task, idx, "dry_run".into()));
            }
            return Ok(SchedulerSummary {
                pattern: self.pattern(),
                total_tasks: total,
                succeeded: 0,
                failed: 0,
                skipped: total,
                total_wall_time: Duration::ZERO,
                results,
            });
        }

        let all: Vec<NodeIndex> = dag.all_tasks().into_iter().map(|(idx, _)| idx).collect();
        let map_idxs: Vec<_> = all.iter().copied().filter(|&idx| dag.dependencies_of(idx).is_empty()).collect();
        let reduce_idxs: Vec<_> = all.iter().copied().filter(|&idx| dag.dependents_of(idx).is_empty()).collect();

        info!(
            mappers = map_idxs.len(),
            reducers = reduce_idxs.len(),
            "MapReduce scheduler starting"
        );

        let mut results: Vec<TaskExecutionResult> = Vec::with_capacity(total);
        let mut succeeded = 0usize;
        let mut failed = 0usize;

        let wave = run_parallel_wave(&map_idxs, dag, &executor, self.config.task_timeout, wall_start, self.config.concurrency_limit).await;

        let mut map_outputs: Vec<String> = Vec::new();
        for res in wave {
            let idx = NodeIndex::new(res.node_index);
            if res.outcome.is_success() {
                let out = res.outcome.output().unwrap().to_string();
                dag.mark_complete_with_output(idx, Some(out.clone()));
                map_outputs.push(out);
                succeeded += 1;
            } else {
                let err = match &res.outcome {
                    TaskOutcome::Failure(e) => e.clone(),
                    _ => "unknown error".into(),
                };
                warn!(task_id = %res.task_id, "mapper failed: {}", err);
                dag.mark_failed(idx, err);
                failed += 1;
            }
            results.push(res);
        }

        for &idx in &reduce_idxs {
            let task_snapshot = dag.get_task(idx).expect("missing reducer").clone();
            let injected = inject_context(task_snapshot, "Map Phase Outputs", &map_outputs);
            dag.mark_running(idx);

            let offset_ms = wall_start.elapsed().as_micros() as u64;
            let start = Instant::now();
            match timeout(self.config.task_timeout, executor(idx, injected)).await {
                Ok(Ok(output)) => {
                    let elapsed = start.elapsed();
                    dag.mark_complete_with_output(idx, Some(output.clone()));
                    results.push(TaskExecutionResult::success(
                        dag.get_task(idx).unwrap(),
                        idx,
                        output,
                        elapsed,
                        offset_ms,
                    ));
                    succeeded += 1;
                }
                Ok(Err(e)) => {
                    let elapsed = start.elapsed();
                    let err = e.to_string();
                    error!(error = %err, "reducer failed");
                    dag.mark_failed(idx, err.clone());
                    results.push(TaskExecutionResult::failure(
                        dag.get_task(idx).unwrap(),
                        idx,
                        err,
                        elapsed,
                        offset_ms,
                    ));
                    failed += 1;
                }
                Err(_) => {
                    let elapsed = start.elapsed();
                    let msg = format!("timed out after {:?}", self.config.task_timeout);
                    error!("reducer {}", msg);
                    dag.mark_failed(idx, msg.clone());
                    results.push(TaskExecutionResult::failure(
                        dag.get_task(idx).unwrap(),
                        idx,
                        msg,
                        elapsed,
                        offset_ms,
                    ));
                    failed += 1;
                }
            }
        }

        Ok(SchedulerSummary {
            pattern: CoordinationPattern::MapReduce,
            total_tasks: total,
            succeeded,
            failed,
            skipped: 0,
            total_wall_time: wall_start.elapsed(),
            results,
        })
    }
}

pub struct DebateScheduler {
    pub config: SwarmSchedulerConfig,
}

impl DebateScheduler {
    pub fn new() -> Self {
        Self {
            config: SwarmSchedulerConfig::default(),
        }
    }

    pub fn with_config(config: SwarmSchedulerConfig) -> Self {
        Self { config }
    }
}

impl Default for DebateScheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SwarmScheduler for DebateScheduler {
    fn pattern(&self) -> CoordinationPattern {
        CoordinationPattern::Debate
    }

    #[instrument(
        skip(self, dag, executor),
        fields(pattern = "debate", task_count = dag.task_count())
    )]
    async fn execute(
        &self,
        dag: &mut SubtaskDAG,
        executor: SubtaskExecutorFn,
    ) -> GlobalResult<SchedulerSummary> {
        let wall_start = Instant::now();
        let total = dag.task_count();

        if self.config.dry_run {
            let ordered = dag.topological_order().unwrap_or_default();
            let mut results = Vec::with_capacity(ordered.len());
            for &idx in &ordered {
                let task = dag.get_task(idx).expect("missing idx").clone();
                results.push(TaskExecutionResult::skipped(&task, idx, "dry_run".into()));
            }
            return Ok(SchedulerSummary {
                pattern: self.pattern(),
                total_tasks: total,
                succeeded: 0,
                failed: 0,
                skipped: total,
                total_wall_time: Duration::ZERO,
                results,
            });
        }

        let all: Vec<NodeIndex> = dag.all_tasks().into_iter().map(|(idx, _)| idx).collect();
        let debater_idxs: Vec<_> = all.iter().copied().filter(|&idx| dag.dependencies_of(idx).is_empty()).collect();
        let judge_idxs: Vec<_> = all.iter().copied().filter(|&idx| dag.dependents_of(idx).is_empty()).collect();

        info!(
            debaters = debater_idxs.len(),
            judges = judge_idxs.len(),
            "Debate scheduler starting"
        );

        let mut results: Vec<TaskExecutionResult> = Vec::with_capacity(total);
        let mut succeeded = 0usize;
        let mut failed = 0usize;

        let wave = run_parallel_wave(&debater_idxs, dag, &executor, self.config.task_timeout, wall_start, self.config.concurrency_limit).await;

        let mut debate_lines: Vec<String> = Vec::new();
        for res in wave {
            let idx = NodeIndex::new(res.node_index);
            if res.outcome.is_success() {
                let out = res.outcome.output().unwrap().to_string();
                dag.mark_complete_with_output(idx, Some(out.clone()));
                debate_lines.push(format!("**{}:** {}", res.task_id, out));
                succeeded += 1;
            } else {
                let err = match &res.outcome {
                    TaskOutcome::Failure(e) => e.clone(),
                    _ => "unknown error".into(),
                };
                warn!(task_id = %res.task_id, "debater failed: {}", err);
                dag.mark_failed(idx, err);
                failed += 1;
            }
            results.push(res);
        }

        for &idx in &judge_idxs {
            let task_snapshot = dag.get_task(idx).expect("missing judge").clone();
            let injected = inject_context(task_snapshot, "Debate Arguments", &debate_lines);
            dag.mark_running(idx);

            let offset_ms = wall_start.elapsed().as_micros() as u64;
            let start = Instant::now();
            match timeout(self.config.task_timeout, executor(idx, injected)).await {
                Ok(Ok(output)) => {
                    let elapsed = start.elapsed();
                    dag.mark_complete_with_output(idx, Some(output.clone()));
                    results.push(TaskExecutionResult::success(
                        dag.get_task(idx).unwrap(),
                        idx,
                        output,
                        elapsed,
                        offset_ms,
                    ));
                    succeeded += 1;
                }
                Ok(Err(e)) => {
                    let elapsed = start.elapsed();
                    let err = e.to_string();
                    error!(error = %err, "judge failed");
                    dag.mark_failed(idx, err.clone());
                    results.push(TaskExecutionResult::failure(
                        dag.get_task(idx).unwrap(),
                        idx,
                        err,
                        elapsed,
                        offset_ms,
                    ));
                    failed += 1;
                }
                Err(_) => {
                    let elapsed = start.elapsed();
                    let msg = format!("timed out after {:?}", self.config.task_timeout);
                    error!("judge {}", msg);
                    dag.mark_failed(idx, msg.clone());
                    results.push(TaskExecutionResult::failure(
                        dag.get_task(idx).unwrap(),
                        idx,
                        msg,
                        elapsed,
                        offset_ms,
                    ));
                    failed += 1;
                }
            }
        }

        Ok(SchedulerSummary {
            pattern: CoordinationPattern::Debate,
            total_tasks: total,
            succeeded,
            failed,
            skipped: 0,
            total_wall_time: wall_start.elapsed(),
            results,
        })
    }
}

pub struct ConsensusScheduler {
    pub config: SwarmSchedulerConfig,
}

impl ConsensusScheduler {
    pub fn new() -> Self {
        Self {
            config: SwarmSchedulerConfig::default(),
        }
    }

    pub fn with_config(config: SwarmSchedulerConfig) -> Self {
        Self { config }
    }
}

impl Default for ConsensusScheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SwarmScheduler for ConsensusScheduler {
    fn pattern(&self) -> CoordinationPattern {
        CoordinationPattern::Consensus
    }

    #[instrument(
        skip(self, dag, executor),
        fields(pattern = "consensus", task_count = dag.task_count())
    )]
    async fn execute(
        &self,
        dag: &mut SubtaskDAG,
        executor: SubtaskExecutorFn,
    ) -> GlobalResult<SchedulerSummary> {
        let wall_start = Instant::now();
        let total = dag.task_count();

        if self.config.dry_run {
            let ordered = dag.topological_order().unwrap_or_default();
            let mut results = Vec::with_capacity(ordered.len());
            for &idx in &ordered {
                let task = dag.get_task(idx).expect("missing idx").clone();
                results.push(TaskExecutionResult::skipped(&task, idx, "dry_run".into()));
            }
            return Ok(SchedulerSummary {
                pattern: self.pattern(),
                total_tasks: total,
                succeeded: 0,
                failed: 0,
                skipped: total,
                total_wall_time: Duration::ZERO,
                results,
            });
        }

        let all: Vec<NodeIndex> = dag.all_tasks().into_iter().map(|(idx, _)| idx).collect();
        let voter_idxs: Vec<_> = all.iter().copied().filter(|&idx| dag.dependencies_of(idx).is_empty()).collect();
        let aggregator_idxs: Vec<_> = all.iter().copied().filter(|&idx| dag.dependents_of(idx).is_empty()).collect();

        info!(
            voters = voter_idxs.len(),
            aggregators = aggregator_idxs.len(),
            "Consensus scheduler starting"
        );

        let mut results: Vec<TaskExecutionResult> = Vec::with_capacity(total);
        let mut succeeded = 0usize;
        let mut failed = 0usize;

        let wave = run_parallel_wave(&voter_idxs, dag, &executor, self.config.task_timeout, wall_start, self.config.concurrency_limit).await;

        let mut voter_outputs: Vec<String> = Vec::new();
        for res in wave {
            let idx = NodeIndex::new(res.node_index);
            if res.outcome.is_success() {
                let out = res.outcome.output().unwrap().to_string();
                dag.mark_complete_with_output(idx, Some(out.clone()));
                voter_outputs.push(out);
                succeeded += 1;
            } else {
                let err = match &res.outcome {
                    TaskOutcome::Failure(e) => e.clone(),
                    _ => "unknown error".into(),
                };
                warn!(task_id = %res.task_id, "voter failed: {}", err);
                dag.mark_failed(idx, err);
                failed += 1;
            }
            results.push(res);
        }

        let majority = {
            let total = voter_outputs.len();
            let mut counts: std::collections::HashMap<&str, usize> =
                std::collections::HashMap::new();
            for v in &voter_outputs {
                *counts.entry(v.as_str()).or_insert(0) += 1;
            }
            let max = counts.values().copied().max().unwrap_or(0);
            // Strict majority: candidate must have more than half of votes.
            // On ties (two candidates with equal max), no majority is declared.
            let winners: Vec<_> = counts
                .iter()
                .filter(|&(_, &c)| c == max)
                .collect();
            if max * 2 > total && winners.len() == 1 {
                winners.into_iter().next().map(|(k, _)| k.to_string())
            } else {
                None
            }
        };

        for &idx in &aggregator_idxs {
            let task_snapshot = dag.get_task(idx).expect("missing aggregator").clone();
            let mut injected =
                inject_context(task_snapshot, "Voter Outputs", &voter_outputs);
            if let Some(ref candidate) = majority {
                injected.description = format!(
                    "## Majority Candidate\n{}\n\n{}",
                    candidate, injected.description
                );
            }
            dag.mark_running(idx);

            let offset_ms = wall_start.elapsed().as_micros() as u64;
            let start = Instant::now();
            match timeout(self.config.task_timeout, executor(idx, injected)).await {
                Ok(Ok(output)) => {
                    let elapsed = start.elapsed();
                    dag.mark_complete_with_output(idx, Some(output.clone()));
                    results.push(TaskExecutionResult::success(
                        dag.get_task(idx).unwrap(),
                        idx,
                        output,
                        elapsed,
                        offset_ms,
                    ));
                    succeeded += 1;
                }
                Ok(Err(e)) => {
                    let elapsed = start.elapsed();
                    let err = e.to_string();
                    error!(error = %err, "aggregator failed");
                    dag.mark_failed(idx, err.clone());
                    results.push(TaskExecutionResult::failure(
                        dag.get_task(idx).unwrap(),
                        idx,
                        err,
                        elapsed,
                        offset_ms,
                    ));
                    failed += 1;
                }
                Err(_) => {
                    let elapsed = start.elapsed();
                    let msg = format!("timed out after {:?}", self.config.task_timeout);
                    error!("aggregator {}", msg);
                    dag.mark_failed(idx, msg.clone());
                    results.push(TaskExecutionResult::failure(
                        dag.get_task(idx).unwrap(),
                        idx,
                        msg,
                        elapsed,
                        offset_ms,
                    ));
                    failed += 1;
                }
            }
        }

        Ok(SchedulerSummary {
            pattern: CoordinationPattern::Consensus,
            total_tasks: total,
            succeeded,
            failed,
            skipped: 0,
            total_wall_time: wall_start.elapsed(),
            results,
        })
    }
}

pub struct RoutingScheduler {
    pub config: SwarmSchedulerConfig,
}

impl RoutingScheduler {
    pub fn new() -> Self {
        Self {
            config: SwarmSchedulerConfig::default(),
        }
    }

    pub fn with_config(config: SwarmSchedulerConfig) -> Self {
        Self { config }
    }
}

impl Default for RoutingScheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SwarmScheduler for RoutingScheduler {
    fn pattern(&self) -> CoordinationPattern {
        CoordinationPattern::Routing
    }

    #[instrument(
        skip(self, dag, executor),
        fields(pattern = "routing", task_count = dag.task_count())
    )]
    async fn execute(
        &self,
        dag: &mut SubtaskDAG,
        executor: SubtaskExecutorFn,
    ) -> GlobalResult<SchedulerSummary> {
        let wall_start = Instant::now();
        let total = dag.task_count();

        if self.config.dry_run {
            let ordered = dag.topological_order().unwrap_or_default();
            let mut results = Vec::with_capacity(ordered.len());
            for &idx in &ordered {
                let task = dag.get_task(idx).expect("missing idx").clone();
                results.push(TaskExecutionResult::skipped(&task, idx, "dry_run".into()));
            }
            return Ok(SchedulerSummary {
                pattern: self.pattern(),
                total_tasks: total,
                succeeded: 0,
                failed: 0,
                skipped: total,
                total_wall_time: Duration::ZERO,
                results,
            });
        }

        let all: Vec<NodeIndex> = dag.all_tasks().into_iter().map(|(idx, _)| idx).collect();
        let router_idxs: Vec<_> = all
            .iter()
            .filter(|&&idx| dag.dependencies_of(idx).is_empty())
            .copied()
            .collect();
        let specialist_idxs: Vec<_> = all
            .iter()
            .filter(|&&idx| !dag.dependencies_of(idx).is_empty())
            .copied()
            .collect();

        if router_idxs.len() != 1 {
            return Err(GlobalError::runtime(format!(
                "Routing pattern requires exactly 1 router task, found {}",
                router_idxs.len()
            )));
        }

        let router_idx = router_idxs[0];
        info!(
            specialists = specialist_idxs.len(),
            "Routing scheduler starting"
        );

        let mut results: Vec<TaskExecutionResult> = Vec::with_capacity(total);
        let mut succeeded = 0usize;
        let mut failed = 0usize;
        let mut skipped = 0usize;

        dag.mark_running(router_idx);
        let router_snapshot = dag.get_task(router_idx).unwrap().clone();
        let router_offset_ms = wall_start.elapsed().as_micros() as u64;
        let start = Instant::now();
        let router_output = match timeout(
            self.config.task_timeout,
            executor(router_idx, router_snapshot.clone()),
        )
        .await
        {
            Ok(Ok(out)) => {
                let elapsed = start.elapsed();
                dag.mark_complete_with_output(router_idx, Some(out.clone()));
                results.push(TaskExecutionResult::success(
                    &router_snapshot,
                    router_idx,
                    out.clone(),
                    elapsed,
                    router_offset_ms,
                ));
                succeeded += 1;
                out
            }
            Ok(Err(e)) => {
                let elapsed = start.elapsed();
                let err = e.to_string();
                error!(error = %err, "router failed");
                dag.mark_failed(router_idx, err.clone());
                results.push(TaskExecutionResult::failure(
                    &router_snapshot,
                    router_idx,
                    err,
                    elapsed,
                    router_offset_ms,
                ));
                failed += 1;
                for &idx in &specialist_idxs {
                    dag.mark_skipped(idx);
                    skipped += 1;
                    results.push(TaskExecutionResult::skipped(
                        dag.get_task(idx).unwrap(),
                        idx,
                        "router failed".into(),
                    ));
                }
                return Ok(SchedulerSummary {
                    pattern: CoordinationPattern::Routing,
                    total_tasks: total,
                    succeeded,
                    failed,
                    skipped,
                    total_wall_time: wall_start.elapsed(),
                    results,
                });
            }
            Err(_) => {
                let elapsed = start.elapsed();
                let msg = format!("timed out after {:?}", self.config.task_timeout);
                error!("router {}", msg);
                dag.mark_failed(router_idx, msg.clone());
                results.push(TaskExecutionResult::failure(
                    &router_snapshot,
                    router_idx,
                    msg,
                    elapsed,
                    router_offset_ms,
                ));
                failed += 1;
                for &idx in &specialist_idxs {
                    dag.mark_skipped(idx);
                    skipped += 1;
                    results.push(TaskExecutionResult::skipped(
                        dag.get_task(idx).unwrap(),
                        idx,
                        "router timed out".into(),
                    ));
                }
                return Ok(SchedulerSummary {
                    pattern: CoordinationPattern::Routing,
                    total_tasks: total,
                    succeeded,
                    failed,
                    skipped,
                    total_wall_time: wall_start.elapsed(),
                    results,
                });
            }
        };

        let router_lower = router_output.to_lowercase();
        let matched_idx = specialist_idxs.iter().find(|&&idx| {
            let task = dag.get_task(idx).unwrap();
            task.required_capabilities
                .iter()
                .any(|cap| router_lower.contains(&cap.to_lowercase()))
        });

        let chosen_idx = if let Some(&idx) = matched_idx {
            idx
        } else if let Some(&first) = specialist_idxs.first() {
            warn!("no capability match found; falling back to first specialist");
            first
        } else {
            return Err(GlobalError::runtime(
                "Routing pattern requires at least one specialist task",
            ));
        };

        for &idx in &specialist_idxs {
            if idx != chosen_idx {
                dag.mark_skipped(idx);
                skipped += 1;
                results.push(TaskExecutionResult::skipped(
                    dag.get_task(idx).unwrap(),
                    idx,
                    "not selected by router".into(),
                ));
            }
        }

        let spec_snapshot = dag.get_task(chosen_idx).unwrap().clone();
        let injected = inject_context(spec_snapshot, "Router Output", &[router_output]);
        dag.mark_running(chosen_idx);

        let spec_offset_ms = wall_start.elapsed().as_micros() as u64;
        let start = Instant::now();
        match timeout(self.config.task_timeout, executor(chosen_idx, injected)).await {
            Ok(Ok(output)) => {
                let elapsed = start.elapsed();
                dag.mark_complete_with_output(chosen_idx, Some(output.clone()));
                results.push(TaskExecutionResult::success(
                    dag.get_task(chosen_idx).unwrap(),
                    chosen_idx,
                    output,
                    elapsed,
                    spec_offset_ms,
                ));
                succeeded += 1;
            }
            Ok(Err(e)) => {
                let elapsed = start.elapsed();
                let err = e.to_string();
                error!(error = %err, "specialist failed");
                dag.mark_failed(chosen_idx, err.clone());
                results.push(TaskExecutionResult::failure(
                    dag.get_task(chosen_idx).unwrap(),
                    chosen_idx,
                    err,
                    elapsed,
                    spec_offset_ms,
                ));
                failed += 1;
            }
            Err(_) => {
                let elapsed = start.elapsed();
                let msg = format!("timed out after {:?}", self.config.task_timeout);
                error!("specialist {}", msg);
                dag.mark_failed(chosen_idx, msg.clone());
                results.push(TaskExecutionResult::failure(
                    dag.get_task(chosen_idx).unwrap(),
                    chosen_idx,
                    msg,
                    elapsed,
                    spec_offset_ms,
                ));
                failed += 1;
            }
        }

        Ok(SchedulerSummary {
            pattern: CoordinationPattern::Routing,
            total_tasks: total,
            succeeded,
            failed,
            skipped,
            total_wall_time: wall_start.elapsed(),
            results,
        })
    }
}

pub struct SupervisionScheduler {
    pub config: SwarmSchedulerConfig,
}

impl SupervisionScheduler {
    pub fn new() -> Self {
        Self {
            config: SwarmSchedulerConfig::default(),
        }
    }

    pub fn with_config(config: SwarmSchedulerConfig) -> Self {
        Self { config }
    }
}

impl Default for SupervisionScheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SwarmScheduler for SupervisionScheduler {
    fn pattern(&self) -> CoordinationPattern {
        CoordinationPattern::Supervision
    }

    #[instrument(
        skip(self, dag, executor),
        fields(pattern = "supervision", task_count = dag.task_count())
    )]
    async fn execute(
        &self,
        dag: &mut SubtaskDAG,
        executor: SubtaskExecutorFn,
    ) -> GlobalResult<SchedulerSummary> {
        let wall_start = Instant::now();
        let total = dag.task_count();

        if self.config.dry_run {
            let ordered = dag.topological_order().unwrap_or_default();
            let mut results = Vec::with_capacity(ordered.len());
            for &idx in &ordered {
                let task = dag.get_task(idx).expect("missing idx").clone();
                results.push(TaskExecutionResult::skipped(&task, idx, "dry_run".into()));
            }
            return Ok(SchedulerSummary {
                pattern: self.pattern(),
                total_tasks: total,
                succeeded: 0,
                failed: 0,
                skipped: total,
                total_wall_time: Duration::ZERO,
                results,
            });
        }

        let all: Vec<NodeIndex> = dag.all_tasks().into_iter().map(|(idx, _)| idx).collect();
        let worker_idxs: Vec<_> = all.iter().copied().filter(|&idx| dag.dependencies_of(idx).is_empty()).collect();
        let supervisor_idxs: Vec<_> = all.iter().copied().filter(|&idx| dag.dependents_of(idx).is_empty()).collect();

        info!(
            workers = worker_idxs.len(),
            supervisors = supervisor_idxs.len(),
            "Supervision scheduler starting"
        );

        let mut results: Vec<TaskExecutionResult> = Vec::with_capacity(total);
        let mut succeeded = 0usize;
        let mut failed = 0usize;

        let wave = run_parallel_wave(&worker_idxs, dag, &executor, self.config.task_timeout, wall_start, self.config.concurrency_limit).await;

        let mut worker_context_lines: Vec<String> = Vec::new();
        for res in wave {
            let idx = NodeIndex::new(res.node_index);
            if res.outcome.is_success() {
                let out = res.outcome.output().unwrap().to_string();
                dag.mark_complete_with_output(idx, Some(out.clone()));
                worker_context_lines.push(format!("{} (SUCCESS): {}", res.task_id, out));
                succeeded += 1;
            } else {
                let err = match &res.outcome {
                    TaskOutcome::Failure(e) => e.clone(),
                    _ => "unknown error".into(),
                };
                dag.mark_failed(idx, err.clone());
                worker_context_lines.push(format!("{} (FAILED): {}", res.task_id, err));
                failed += 1;
            }
            results.push(res);
        }

        for &idx in &supervisor_idxs {
            let task_snapshot = dag.get_task(idx).expect("missing supervisor").clone();
            let injected =
                inject_context(task_snapshot, "Worker Results", &worker_context_lines);
            dag.mark_running(idx);

            let offset_ms = wall_start.elapsed().as_micros() as u64;
            let start = Instant::now();
            match timeout(self.config.task_timeout, executor(idx, injected)).await {
                Ok(Ok(output)) => {
                    let elapsed = start.elapsed();
                    dag.mark_complete_with_output(idx, Some(output.clone()));
                    results.push(TaskExecutionResult::success(
                        dag.get_task(idx).unwrap(),
                        idx,
                        output,
                        elapsed,
                        offset_ms,
                    ));
                    succeeded += 1;
                }
                Ok(Err(e)) => {
                    let elapsed = start.elapsed();
                    let err = e.to_string();
                    error!(error = %err, "supervisor failed");
                    dag.mark_failed(idx, err.clone());
                    results.push(TaskExecutionResult::failure(
                        dag.get_task(idx).unwrap(),
                        idx,
                        err,
                        elapsed,
                        offset_ms,
                    ));
                    failed += 1;
                }
                Err(_) => {
                    let elapsed = start.elapsed();
                    let msg = format!("timed out after {:?}", self.config.task_timeout);
                    error!("supervisor {}", msg);
                    dag.mark_failed(idx, msg.clone());
                    results.push(TaskExecutionResult::failure(
                        dag.get_task(idx).unwrap(),
                        idx,
                        msg,
                        elapsed,
                        offset_ms,
                    ));
                    failed += 1;
                }
            }
        }

        Ok(SchedulerSummary {
            pattern: CoordinationPattern::Supervision,
            total_tasks: total,
            succeeded,
            failed,
            skipped: 0,
            total_wall_time: wall_start.elapsed(),
            results,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::swarm::{DependencyKind, SwarmSubtask};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_sequential_topological_order() {
        let mut dag = SubtaskDAG::new("test");
        let idx_a = dag.add_task(SwarmSubtask::new("A", "Task A"));
        let idx_b = dag.add_task(SwarmSubtask::new("B", "Task B"));
        let idx_c = dag.add_task(SwarmSubtask::new("C", "Task C"));

        dag.add_dependency(idx_a, idx_b).unwrap();
        dag.add_dependency(idx_b, idx_c).unwrap();

        let exec_order = Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let exec_order_clone = exec_order.clone();

        let executor: SubtaskExecutorFn = Arc::new(move |_idx, task| {
            let order = exec_order_clone.clone();
            let task_id = task.id.clone();
            Box::pin(async move {
                order.lock().await.push(task_id.clone());
                Ok(format!("{} done", task_id))
            })
        });

        let scheduler = SequentialScheduler::new();
        let summary = scheduler.execute(&mut dag, executor).await.unwrap();

        assert!(summary.is_fully_successful());
        assert_eq!(summary.succeeded, 3);

        let order = exec_order.lock().await;
        assert_eq!(*order, vec!["A", "B", "C"]);
    }

    #[tokio::test]
    async fn test_parallel_fail_fast_cascades() {
        let mut dag = SubtaskDAG::new("test");
        let idx_a = dag.add_task(SwarmSubtask::new("A", "Task A"));
        let idx_b = dag.add_task(SwarmSubtask::new("B", "Task B"));
        let idx_c = dag.add_task(SwarmSubtask::new("C", "Task C"));

        dag.add_dependency(idx_a, idx_b).unwrap();
        dag.add_dependency(idx_a, idx_c).unwrap();

        let executor: SubtaskExecutorFn = Arc::new(move |_idx, task| {
            Box::pin(async move {
                if task.id == "A" {
                    Err(GlobalError::runtime("A failed"))
                } else {
                    Ok("done".into())
                }
            })
        });

        let mut config = SwarmSchedulerConfig::default();
        config.failure_policy = FailurePolicy::FailFastCascade;
        let scheduler = ParallelScheduler::with_config(config);

        let summary = scheduler.execute(&mut dag, executor).await.unwrap();

        assert_eq!(summary.failed, 1);
        assert_eq!(summary.skipped, 2);
        assert_eq!(summary.succeeded, 0);

        assert_eq!(
            dag.get_task(idx_a).unwrap().status,
            crate::swarm::SubtaskStatus::Failed("Runtime error: A failed".into())
        );
        assert_eq!(
            dag.get_task(idx_b).unwrap().status,
            crate::swarm::SubtaskStatus::Skipped
        );
        assert_eq!(
            dag.get_task(idx_c).unwrap().status,
            crate::swarm::SubtaskStatus::Skipped
        );
    }

    #[tokio::test]
    async fn test_continue_lets_downstream_run() {
        let mut dag = SubtaskDAG::new("test");
        let idx_a = dag.add_task(SwarmSubtask::new("A", "Task A"));
        let idx_b = dag.add_task(SwarmSubtask::new("B", "Task B"));

        dag.add_dependency(idx_a, idx_b).unwrap();

        let executor: SubtaskExecutorFn = Arc::new(move |_idx, task| {
            Box::pin(async move {
                if task.id == "A" {
                    Err(GlobalError::runtime("A failed"))
                } else {
                    Ok("B ran anyway".into())
                }
            })
        });

        let scheduler = SequentialScheduler::new();
        let summary = scheduler.execute(&mut dag, executor).await.unwrap();

        assert_eq!(summary.failed, 1);
        assert_eq!(summary.skipped, 0);
        assert_eq!(summary.succeeded, 1);

        assert_eq!(
            dag.get_task(idx_b).unwrap().status,
            crate::swarm::SubtaskStatus::Completed
        );
    }

    #[tokio::test]
    async fn test_timeout_marks_failed() {
        let mut dag = SubtaskDAG::new("test");
        let _idx_a = dag.add_task(SwarmSubtask::new("A", "Task A"));

        let executor: SubtaskExecutorFn = Arc::new(move |_idx, _task| {
            Box::pin(async move {
                sleep(Duration::from_millis(100)).await;
                Ok("done".into())
            })
        });

        let mut config = SwarmSchedulerConfig::default();
        config.task_timeout = Duration::from_millis(1);
        let scheduler = SequentialScheduler::with_config(config);

        let summary = scheduler.execute(&mut dag, executor).await.unwrap();

        assert_eq!(summary.failed, 1);
        let results = summary.results;
        assert!(
            matches!(results[0].outcome, TaskOutcome::Failure(ref s) if s.contains("timed out"))
        );
    }

    #[tokio::test]
    async fn test_parallel_diamond_waves() {
        let mut dag = SubtaskDAG::new("test");
        let idx_a = dag.add_task(SwarmSubtask::new("A", "Task A"));
        let idx_b = dag.add_task(SwarmSubtask::new("B", "Task B"));
        let idx_c = dag.add_task(SwarmSubtask::new("C", "Task C"));
        let idx_d = dag.add_task(SwarmSubtask::new("D", "Task D"));

        dag.add_dependency(idx_a, idx_b).unwrap();
        dag.add_dependency(idx_a, idx_c).unwrap();
        dag.add_dependency(idx_b, idx_d).unwrap();
        dag.add_dependency(idx_c, idx_d).unwrap();

        let execution_log = Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let execution_log_clone = execution_log.clone();

        let executor: SubtaskExecutorFn = Arc::new(move |_idx, task| {
            let log = execution_log_clone.clone();
            let task_id = task.id.clone();
            Box::pin(async move {
                sleep(Duration::from_millis(50)).await;
                log.lock().await.push(task_id.clone());
                Ok(format!("{} done", task_id))
            })
        });

        let scheduler = ParallelScheduler::new();
        let summary = scheduler.execute(&mut dag, executor).await.unwrap();

        assert!(summary.is_fully_successful());
        assert_eq!(summary.succeeded, 4);

        let log = execution_log.lock().await;
        assert_eq!(log[0], "A");
        assert!(log[1] == "B" || log[1] == "C");
        assert!(log[2] == "B" || log[2] == "C");
        assert_ne!(log[1], log[2]);
        assert_eq!(log[3], "D");
    }

    #[tokio::test]
    async fn test_parallel_concurrency_limit() {
        let mut dag = SubtaskDAG::new("test");
        for i in 0..10 {
            dag.add_task(SwarmSubtask::new(format!("T{}", i), format!("Task {}", i)));
        }

        let active_tasks = Arc::new(AtomicUsize::new(0));
        let peak_tasks = Arc::new(AtomicUsize::new(0));

        let active_clone = active_tasks.clone();
        let peak_clone = peak_tasks.clone();

        let executor: SubtaskExecutorFn = Arc::new(move |_idx, _task| {
            let active = active_clone.clone();
            let peak = peak_clone.clone();

            Box::pin(async move {
                let current = active.fetch_add(1, Ordering::SeqCst) + 1;
                let mut current_peak = peak.load(Ordering::SeqCst);
                while current > current_peak {
                    match peak.compare_exchange_weak(
                        current_peak,
                        current,
                        Ordering::SeqCst,
                        Ordering::SeqCst,
                    ) {
                        Ok(_) => break,
                        Err(x) => current_peak = x,
                    }
                }

                sleep(Duration::from_millis(50)).await;

                active.fetch_sub(1, Ordering::SeqCst);
                Ok("done".into())
            })
        });

        let mut config = SwarmSchedulerConfig::default();
        config.concurrency_limit = Some(3);
        let scheduler = ParallelScheduler::with_config(config);

        let summary = scheduler.execute(&mut dag, executor).await.unwrap();

        assert!(summary.is_fully_successful());
        assert_eq!(summary.succeeded, 10);

        let actual_peak = peak_tasks.load(Ordering::SeqCst);
        assert_eq!(
            actual_peak, 3,
            "Peak concurrency should strictly match the limit of 3"
        );
    }

    #[test]
    fn test_into_scheduler() {
        let seq = CoordinationPattern::Sequential.into_scheduler();
        assert_eq!(seq.pattern(), CoordinationPattern::Sequential);

        let par = CoordinationPattern::Parallel.into_scheduler();
        assert_eq!(par.pattern(), CoordinationPattern::Parallel);

        let mr = CoordinationPattern::MapReduce.into_scheduler();
        assert_eq!(mr.pattern(), CoordinationPattern::MapReduce);

        let deb = CoordinationPattern::Debate.into_scheduler();
        assert_eq!(deb.pattern(), CoordinationPattern::Debate);

        let con = CoordinationPattern::Consensus.into_scheduler();
        assert_eq!(con.pattern(), CoordinationPattern::Consensus);

        let rou = CoordinationPattern::Routing.into_scheduler();
        assert_eq!(rou.pattern(), CoordinationPattern::Routing);

        let sup = CoordinationPattern::Supervision.into_scheduler();
        assert_eq!(sup.pattern(), CoordinationPattern::Supervision);
    }

    #[tokio::test]
    async fn test_sequential_fail_fast_cascades_skip_to_hard_dependents() {
        let mut dag = SubtaskDAG::new("test");
        let idx_a = dag.add_task(SwarmSubtask::new("A", "Task A"));
        let idx_b = dag.add_task(SwarmSubtask::new("B", "Task B"));
        let idx_c = dag.add_task(SwarmSubtask::new("C", "Task C"));

        // A -> B -> C
        dag.add_dependency(idx_a, idx_b).unwrap();
        dag.add_dependency(idx_b, idx_c).unwrap();

        let executor: SubtaskExecutorFn = Arc::new(move |_idx, task| {
            Box::pin(async move {
                if task.id == "A" {
                    Err(GlobalError::runtime("boom"))
                } else {
                    Ok("should not run".into())
                }
            })
        });

        let mut config = SwarmSchedulerConfig::default();
        config.failure_policy = FailurePolicy::FailFastCascade;
        let scheduler = SequentialScheduler::with_config(config);

        let summary = scheduler.execute(&mut dag, executor).await.unwrap();
        assert_eq!(summary.failed, 1);
        assert_eq!(summary.skipped, 2);
        assert_eq!(summary.succeeded, 0);
        assert_eq!(
            dag.get_task(idx_b).unwrap().status,
            crate::swarm::SubtaskStatus::Skipped
        );
        assert_eq!(
            dag.get_task(idx_c).unwrap().status,
            crate::swarm::SubtaskStatus::Skipped
        );
    }

    #[tokio::test]
    async fn test_soft_dependency_does_not_block_execution() {
        let mut dag = SubtaskDAG::new("test");
        let idx_a = dag.add_task(SwarmSubtask::new("A", "Optional"));
        let idx_b = dag.add_task(SwarmSubtask::new("B", "Main"));

        dag.add_dependency_with_kind(idx_a, idx_b, DependencyKind::Soft)
            .unwrap();

        // Executor fails A, but B must still run because it's only a soft dependency.
        let executor: SubtaskExecutorFn = Arc::new(move |_idx, task| {
            Box::pin(async move {
                if task.id == "A" {
                    Err(GlobalError::runtime("A failed"))
                } else {
                    Ok("B ok".into())
                }
            })
        });

        let scheduler = ParallelScheduler::new();
        let summary = scheduler.execute(&mut dag, executor).await.unwrap();

        assert_eq!(summary.failed, 1);
        assert_eq!(summary.succeeded, 1);
        assert_eq!(summary.skipped, 0);
        assert_eq!(
            dag.get_task(idx_b).unwrap().status,
            crate::swarm::SubtaskStatus::Completed
        );
    }

    #[tokio::test]
    async fn test_outputs_are_persisted_on_dag_nodes() {
        let mut dag = SubtaskDAG::new("test");
        let idx_a = dag.add_task(SwarmSubtask::new("A", "Task A"));

        let executor: SubtaskExecutorFn =
            Arc::new(move |_idx, task| Box::pin(async move { Ok(format!("out:{}", task.id)) }));

        let scheduler = SequentialScheduler::new();
        let summary = scheduler.execute(&mut dag, executor).await.unwrap();
        assert!(summary.is_fully_successful());

        let task = dag.get_task(idx_a).unwrap();
        assert_eq!(task.status, crate::swarm::SubtaskStatus::Completed);
        assert_eq!(task.output.as_deref(), Some("out:A"));
    }

    #[tokio::test]
    async fn test_empty_dag_is_noop_and_does_not_panic() {
        let mut dag = SubtaskDAG::new("empty");

        let executor: SubtaskExecutorFn =
            Arc::new(move |_idx, _task| Box::pin(async move { Ok("unused".into()) }));

        let scheduler = SequentialScheduler::new();
        let summary = scheduler.execute(&mut dag, executor).await.unwrap();
        assert_eq!(summary.total_tasks, 0);
        assert_eq!(summary.succeeded, 0);
        assert_eq!(summary.failed, 0);
        assert_eq!(summary.skipped, 0);
    }

    #[tokio::test]
    async fn test_parallel_continue_after_failure_matches_ready_tasks_semantics() {
        let mut dag = SubtaskDAG::new("test");
        let idx_a = dag.add_task(SwarmSubtask::new("A", "A"));
        let idx_b = dag.add_task(SwarmSubtask::new("B", "B"));
        dag.add_dependency(idx_a, idx_b).unwrap();

        let executor: SubtaskExecutorFn = Arc::new(move |_idx, task| {
            Box::pin(async move {
                if task.id == "A" {
                    Err(GlobalError::runtime("A failed"))
                } else {
                    Ok("B ran".into())
                }
            })
        });

        let scheduler = ParallelScheduler::new(); // default FailurePolicy::Continue
        let summary = scheduler.execute(&mut dag, executor).await.unwrap();

        assert_eq!(summary.failed, 1);
        assert_eq!(summary.succeeded, 1);
        assert_eq!(summary.skipped, 0);
        assert_eq!(
            dag.get_task(idx_b).unwrap().status,
            crate::swarm::SubtaskStatus::Completed
        );
    }

    #[tokio::test]
    async fn test_concurrency_limit_one_never_exceeds_one_active_task() {
        let mut dag = SubtaskDAG::new("test");
        for i in 0..5 {
            dag.add_task(SwarmSubtask::new(format!("T{}", i), "t"));
        }

        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let a = active.clone();
        let p = peak.clone();

        let executor: SubtaskExecutorFn = Arc::new(move |_idx, _task| {
            let a = a.clone();
            let p = p.clone();
            Box::pin(async move {
                let current = a.fetch_add(1, Ordering::SeqCst) + 1;
                p.fetch_max(current, Ordering::SeqCst);
                sleep(Duration::from_millis(10)).await;
                a.fetch_sub(1, Ordering::SeqCst);
                Ok("ok".into())
            })
        });

        let mut config = SwarmSchedulerConfig::default();
        config.concurrency_limit = Some(1);
        let scheduler = ParallelScheduler::with_config(config);

        let summary = scheduler.execute(&mut dag, executor).await.unwrap();
        assert!(summary.is_fully_successful());
        assert_eq!(peak.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_mapreduce_outputs_injected_into_reducer() {
        let mut dag = SubtaskDAG::new("test");
        let idx_m1 = dag.add_task(SwarmSubtask::new("mapper-1", "Map chunk 1"));
        let idx_m2 = dag.add_task(SwarmSubtask::new("mapper-2", "Map chunk 2"));
        let idx_r = dag.add_task(SwarmSubtask::new("reducer", "Reduce all"));
        dag.add_dependency(idx_m1, idx_r).unwrap();
        dag.add_dependency(idx_m2, idx_r).unwrap();

        let seen_desc = Arc::new(tokio::sync::Mutex::new(String::new()));
        let seen_clone = seen_desc.clone();

        let executor: SubtaskExecutorFn = Arc::new(move |_idx, task| {
            let seen = seen_clone.clone();
            let id = task.id.clone();
            let desc = task.description.clone();
            Box::pin(async move {
                if id == "reducer" {
                    *seen.lock().await = desc;
                }
                Ok(format!("{}-out", id))
            })
        });

        let scheduler = MapReduceScheduler::new();
        let summary = scheduler.execute(&mut dag, executor).await.unwrap();

        assert!(summary.is_fully_successful());
        assert_eq!(summary.succeeded, 3);

        let desc = seen_desc.lock().await;
        assert!(desc.contains("mapper-1-out"), "reducer must see mapper-1 output");
        assert!(desc.contains("mapper-2-out"), "reducer must see mapper-2 output");
    }

    #[tokio::test]
    async fn test_mapreduce_partial_failure_reducer_still_runs() {
        let mut dag = SubtaskDAG::new("test");
        let idx_m1 = dag.add_task(SwarmSubtask::new("mapper-ok", "Map A"));
        let idx_m2 = dag.add_task(SwarmSubtask::new("mapper-fail", "Map B"));
        let idx_r = dag.add_task(SwarmSubtask::new("reducer", "Reduce"));
        dag.add_dependency(idx_m1, idx_r).unwrap();
        dag.add_dependency(idx_m2, idx_r).unwrap();

        let reducer_ran = Arc::new(AtomicUsize::new(0));
        let ran_clone = reducer_ran.clone();

        let executor: SubtaskExecutorFn = Arc::new(move |_idx, task| {
            let ran = ran_clone.clone();
            let id = task.id.clone();
            Box::pin(async move {
                if id == "mapper-fail" {
                    Err(GlobalError::runtime("mapper error"))
                } else if id == "reducer" {
                    ran.fetch_add(1, Ordering::SeqCst);
                    Ok("reduced".into())
                } else {
                    Ok("ok".into())
                }
            })
        });

        let scheduler = MapReduceScheduler::new();
        let summary = scheduler.execute(&mut dag, executor).await.unwrap();

        assert_eq!(reducer_ran.load(Ordering::SeqCst), 1, "reducer must run even when a mapper fails");
        assert_eq!(summary.failed, 1);
        assert_eq!(summary.succeeded, 2);
    }

    #[tokio::test]
    async fn test_debate_judge_receives_both_arguments() {
        let mut dag = SubtaskDAG::new("test");
        let idx_pro = dag.add_task(SwarmSubtask::new("pro", "Argue for"));
        let idx_con = dag.add_task(SwarmSubtask::new("con", "Argue against"));
        let idx_j = dag.add_task(SwarmSubtask::new("judge", "Decide"));
        dag.add_dependency(idx_pro, idx_j).unwrap();
        dag.add_dependency(idx_con, idx_j).unwrap();

        let judge_desc = Arc::new(tokio::sync::Mutex::new(String::new()));
        let jd_clone = judge_desc.clone();

        let executor: SubtaskExecutorFn = Arc::new(move |_idx, task| {
            let jd = jd_clone.clone();
            let id = task.id.clone();
            let desc = task.description.clone();
            Box::pin(async move {
                if id == "judge" {
                    *jd.lock().await = desc;
                }
                Ok(format!("{}-verdict", id))
            })
        });

        let scheduler = DebateScheduler::new();
        let summary = scheduler.execute(&mut dag, executor).await.unwrap();

        assert!(summary.is_fully_successful());
        let desc = judge_desc.lock().await;
        assert!(desc.contains("**pro:**"), "judge must see pro argument");
        assert!(desc.contains("**con:**"), "judge must see con argument");
    }

    #[tokio::test]
    async fn test_debate_debaters_run_in_parallel() {
        let mut dag = SubtaskDAG::new("test");
        let idx_a = dag.add_task(SwarmSubtask::new("debater-a", "Position A"));
        let idx_b = dag.add_task(SwarmSubtask::new("debater-b", "Position B"));
        let idx_j = dag.add_task(SwarmSubtask::new("judge", "Decide"));
        dag.add_dependency(idx_a, idx_j).unwrap();
        dag.add_dependency(idx_b, idx_j).unwrap();

        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let a = active.clone();
        let p = peak.clone();

        let executor: SubtaskExecutorFn = Arc::new(move |_idx, task| {
            let a = a.clone();
            let p = p.clone();
            let id = task.id.clone();
            Box::pin(async move {
                if id != "judge" {
                    let current = a.fetch_add(1, Ordering::SeqCst) + 1;
                    p.fetch_max(current, Ordering::SeqCst);
                    sleep(Duration::from_millis(30)).await;
                    a.fetch_sub(1, Ordering::SeqCst);
                }
                Ok(format!("{}-done", id))
            })
        });

        let scheduler = DebateScheduler::new();
        let summary = scheduler.execute(&mut dag, executor).await.unwrap();

        assert!(summary.is_fully_successful());
        assert!(
            peak.load(Ordering::SeqCst) >= 2,
            "debaters must run in parallel"
        );
    }

    #[tokio::test]
    async fn test_consensus_aggregator_receives_voter_outputs() {
        let mut dag = SubtaskDAG::new("test");
        let idx_v1 = dag.add_task(SwarmSubtask::new("voter-1", "Vote A"));
        let idx_v2 = dag.add_task(SwarmSubtask::new("voter-2", "Vote B"));
        let idx_agg = dag.add_task(SwarmSubtask::new("aggregator", "Aggregate"));
        dag.add_dependency(idx_v1, idx_agg).unwrap();
        dag.add_dependency(idx_v2, idx_agg).unwrap();

        let agg_desc = Arc::new(tokio::sync::Mutex::new(String::new()));
        let agg_clone = agg_desc.clone();

        let executor: SubtaskExecutorFn = Arc::new(move |_idx, task| {
            let agg = agg_clone.clone();
            let id = task.id.clone();
            let desc = task.description.clone();
            Box::pin(async move {
                if id == "aggregator" {
                    *agg.lock().await = desc;
                }
                Ok(format!("{}-result", id))
            })
        });

        let scheduler = ConsensusScheduler::new();
        let summary = scheduler.execute(&mut dag, executor).await.unwrap();

        assert!(summary.is_fully_successful());
        let desc = agg_desc.lock().await;
        assert!(desc.contains("voter-1-result"), "aggregator must see voter-1");
        assert!(desc.contains("voter-2-result"), "aggregator must see voter-2");
    }

    #[tokio::test]
    async fn test_consensus_majority_candidate_prepended() {
        let mut dag = SubtaskDAG::new("test");
        let idx_v1 = dag.add_task(SwarmSubtask::new("voter-1", "Vote"));
        let idx_v2 = dag.add_task(SwarmSubtask::new("voter-2", "Vote"));
        let idx_v3 = dag.add_task(SwarmSubtask::new("voter-3", "Vote"));
        let idx_agg = dag.add_task(SwarmSubtask::new("aggregator", "Aggregate"));
        dag.add_dependency(idx_v1, idx_agg).unwrap();
        dag.add_dependency(idx_v2, idx_agg).unwrap();
        dag.add_dependency(idx_v3, idx_agg).unwrap();

        let agg_desc = Arc::new(tokio::sync::Mutex::new(String::new()));
        let agg_clone = agg_desc.clone();

        let executor: SubtaskExecutorFn = Arc::new(move |_idx, task| {
            let agg = agg_clone.clone();
            let id = task.id.clone();
            let desc = task.description.clone();
            Box::pin(async move {
                if id == "aggregator" {
                    *agg.lock().await = desc;
                    Ok("final".into())
                } else {
                    Ok("positive".into())
                }
            })
        });

        let scheduler = ConsensusScheduler::new();
        let summary = scheduler.execute(&mut dag, executor).await.unwrap();

        assert!(summary.is_fully_successful());
        let desc = agg_desc.lock().await;
        assert!(
            desc.contains("Majority Candidate"),
            "aggregator must see majority candidate section"
        );
        assert!(desc.contains("positive"), "majority candidate must be the repeated vote");
    }

    #[tokio::test]
    async fn test_routing_matches_specialist_by_capability() {
        let mut dag = SubtaskDAG::new("test");
        let idx_router = dag.add_task(SwarmSubtask::new("router", "Route task"));

        let mut db_spec = SwarmSubtask::new("db-specialist", "Handle DB");
        db_spec.required_capabilities = vec!["database".into()];
        let idx_db = dag.add_task(db_spec);

        let mut ml_spec = SwarmSubtask::new("ml-specialist", "Handle ML");
        ml_spec.required_capabilities = vec!["machine_learning".into()];
        let idx_ml = dag.add_task(ml_spec);

        dag.add_dependency(idx_router, idx_db).unwrap();
        dag.add_dependency(idx_router, idx_ml).unwrap();

        let executed = Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let exec_clone = executed.clone();

        let executor: SubtaskExecutorFn = Arc::new(move |_idx, task| {
            let exec = exec_clone.clone();
            let id = task.id.clone();
            Box::pin(async move {
                exec.lock().await.push(id.clone());
                if id == "router" {
                    Ok("needs database access".into())
                } else {
                    Ok(format!("{}-done", id))
                }
            })
        });

        let scheduler = RoutingScheduler::new();
        let summary = scheduler.execute(&mut dag, executor).await.unwrap();

        assert_eq!(summary.succeeded, 2);
        assert_eq!(summary.skipped, 1);

        let ran = executed.lock().await;
        assert!(ran.contains(&"db-specialist".to_string()), "db specialist must run");
        assert!(!ran.contains(&"ml-specialist".to_string()), "ml specialist must be skipped");
    }

    #[tokio::test]
    async fn test_routing_fallback_when_no_match() {
        let mut dag = SubtaskDAG::new("test");
        let idx_router = dag.add_task(SwarmSubtask::new("router", "Route task"));

        let mut s1 = SwarmSubtask::new("specialist-1", "Handle X");
        s1.required_capabilities = vec!["very_specific_cap".into()];
        let idx_s1 = dag.add_task(s1);

        let mut s2 = SwarmSubtask::new("specialist-2", "Handle Y");
        s2.required_capabilities = vec!["another_cap".into()];
        let idx_s2 = dag.add_task(s2);

        dag.add_dependency(idx_router, idx_s1).unwrap();
        dag.add_dependency(idx_router, idx_s2).unwrap();

        let executed = Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let exec_clone = executed.clone();

        let executor: SubtaskExecutorFn = Arc::new(move |_idx, task| {
            let exec = exec_clone.clone();
            let id = task.id.clone();
            Box::pin(async move {
                exec.lock().await.push(id.clone());
                Ok(format!("{}-done", id))
            })
        });

        let scheduler = RoutingScheduler::new();
        let summary = scheduler.execute(&mut dag, executor).await.unwrap();

        assert_eq!(summary.succeeded, 2);
        assert_eq!(summary.skipped, 1);

        let ran = executed.lock().await;
        assert_eq!(ran.len(), 2, "router + one fallback specialist");
    }

    #[tokio::test]
    async fn test_supervision_supervisor_always_runs_after_workers() {
        let mut dag = SubtaskDAG::new("test");
        let idx_w1 = dag.add_task(SwarmSubtask::new("worker-ok", "Do work"));
        let idx_w2 = dag.add_task(SwarmSubtask::new("worker-fail", "Do work 2"));
        let idx_sup = dag.add_task(SwarmSubtask::new("supervisor", "Oversee"));
        dag.add_dependency(idx_w1, idx_sup).unwrap();
        dag.add_dependency(idx_w2, idx_sup).unwrap();

        let supervisor_ran = Arc::new(AtomicUsize::new(0));
        let ran_clone = supervisor_ran.clone();

        let executor: SubtaskExecutorFn = Arc::new(move |_idx, task| {
            let ran = ran_clone.clone();
            let id = task.id.clone();
            Box::pin(async move {
                if id == "worker-fail" {
                    Err(GlobalError::runtime("worker crashed"))
                } else if id == "supervisor" {
                    ran.fetch_add(1, Ordering::SeqCst);
                    Ok("supervised".into())
                } else {
                    Ok("worker-ok-out".into())
                }
            })
        });

        let scheduler = SupervisionScheduler::new();
        let summary = scheduler.execute(&mut dag, executor).await.unwrap();

        assert_eq!(
            supervisor_ran.load(Ordering::SeqCst),
            1,
            "supervisor must always run"
        );
        assert_eq!(summary.failed, 1);
        assert_eq!(summary.succeeded, 2);
    }

    #[tokio::test]
    async fn test_supervision_supervisor_receives_failure_context() {
        let mut dag = SubtaskDAG::new("test");
        let idx_w1 = dag.add_task(SwarmSubtask::new("worker-a", "Task A"));
        let idx_w2 = dag.add_task(SwarmSubtask::new("worker-b", "Task B"));
        let idx_sup = dag.add_task(SwarmSubtask::new("supervisor", "Oversee"));
        dag.add_dependency(idx_w1, idx_sup).unwrap();
        dag.add_dependency(idx_w2, idx_sup).unwrap();

        let sup_desc = Arc::new(tokio::sync::Mutex::new(String::new()));
        let desc_clone = sup_desc.clone();

        let executor: SubtaskExecutorFn = Arc::new(move |_idx, task| {
            let desc = desc_clone.clone();
            let id = task.id.clone();
            let task_desc = task.description.clone();
            Box::pin(async move {
                if id == "worker-b" {
                    Err(GlobalError::runtime("critical failure"))
                } else if id == "supervisor" {
                    *desc.lock().await = task_desc;
                    Ok("recovery plan issued".into())
                } else {
                    Ok("done".into())
                }
            })
        });

        let scheduler = SupervisionScheduler::new();
        let summary = scheduler.execute(&mut dag, executor).await.unwrap();

        assert_eq!(summary.succeeded, 2);
        assert_eq!(summary.failed, 1);

        let desc = sup_desc.lock().await;
        assert!(desc.contains("FAILED"), "supervisor must see FAILED label for failed worker");
        assert!(desc.contains("worker-b"), "supervisor must see failed worker id");
    }

    #[tokio::test]
    async fn test_mapreduce_mappers_run_in_parallel() {
        let mut dag = SubtaskDAG::new("test");
        let m1 = dag.add_task(SwarmSubtask::new("mapper-a", "Map A"));
        let m2 = dag.add_task(SwarmSubtask::new("mapper-b", "Map B"));
        let m3 = dag.add_task(SwarmSubtask::new("mapper-c", "Map C"));
        let r = dag.add_task(SwarmSubtask::new("reducer", "Reduce"));
        dag.add_dependency(m1, r).unwrap();
        dag.add_dependency(m2, r).unwrap();
        dag.add_dependency(m3, r).unwrap();

        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let a = active.clone();
        let p = peak.clone();

        let executor: SubtaskExecutorFn = Arc::new(move |_idx, task| {
            let a = a.clone();
            let p = p.clone();
            let id = task.id.clone();
            Box::pin(async move {
                if id != "reducer" {
                    let current = a.fetch_add(1, Ordering::SeqCst) + 1;
                    p.fetch_max(current, Ordering::SeqCst);
                    sleep(Duration::from_millis(30)).await;
                    a.fetch_sub(1, Ordering::SeqCst);
                }
                Ok(format!("{}-out", id))
            })
        });

        let scheduler = MapReduceScheduler::new();
        let summary = scheduler.execute(&mut dag, executor).await.unwrap();

        assert!(summary.is_fully_successful());
        assert!(
            peak.load(Ordering::SeqCst) >= 3,
            "all 3 mappers must run in parallel"
        );
    }

    #[tokio::test]
    async fn test_dry_run_skips_all_without_executing() {
        let mut dag = SubtaskDAG::new("test");
        dag.add_task(SwarmSubtask::new("a", "Task A"));
        dag.add_task(SwarmSubtask::new("b", "Task B"));
        dag.add_task(SwarmSubtask::new("c", "Task C"));

        let called = Arc::new(AtomicUsize::new(0));
        let c = called.clone();
        let executor: SubtaskExecutorFn = Arc::new(move |_idx, _task| {
            c.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { Ok("should not run".into()) })
        });

        let config = SwarmSchedulerConfig { dry_run: true, ..Default::default() };
        let summary = ParallelScheduler::with_config(config)
            .execute(&mut dag, executor)
            .await
            .unwrap();

        assert_eq!(called.load(Ordering::SeqCst), 0, "executor must never be called in dry_run");
        assert_eq!(summary.skipped, 3);
        assert_eq!(summary.succeeded, 0);
        assert_eq!(summary.total_wall_time, Duration::ZERO);
    }

    #[tokio::test]
    async fn test_timeline_sequential_tasks_have_increasing_offsets() {
        let mut dag = SubtaskDAG::new("test");
        let a = dag.add_task(SwarmSubtask::new("a", "Task A"));
        let b = dag.add_task(SwarmSubtask::new("b", "Task B"));
        let c = dag.add_task(SwarmSubtask::new("c", "Task C"));
        dag.add_dependency(a, b).unwrap();
        dag.add_dependency(b, c).unwrap();

        let executor: SubtaskExecutorFn = Arc::new(move |_idx, task| {
            let id = task.id.clone();
            Box::pin(async move {
                sleep(Duration::from_millis(20)).await;
                Ok(format!("{}-done", id))
            })
        });

        let summary = SequentialScheduler::new().execute(&mut dag, executor).await.unwrap();

        assert!(summary.is_fully_successful());
        let tl = summary.timeline();
        assert_eq!(tl.len(), 3);
        assert!(
            tl[0].start_offset_us <= tl[1].start_offset_us,
            "a must start before b"
        );
        assert!(
            tl[1].start_offset_us <= tl[2].start_offset_us,
            "b must start before c"
        );
        assert!(
            summary.peak_concurrency() <= 1,
            "sequential never exceeds 1 concurrent task"
        );
    }

    #[tokio::test]
    async fn test_peak_concurrency_parallel_tasks() {
        let mut dag = SubtaskDAG::new("test");
        dag.add_task(SwarmSubtask::new("p1", "Task 1"));
        dag.add_task(SwarmSubtask::new("p2", "Task 2"));
        dag.add_task(SwarmSubtask::new("p3", "Task 3"));

        let executor: SubtaskExecutorFn = Arc::new(move |_idx, task| {
            let id = task.id.clone();
            Box::pin(async move {
                sleep(Duration::from_millis(30)).await;
                Ok(format!("{}-done", id))
            })
        });

        let summary = ParallelScheduler::new().execute(&mut dag, executor).await.unwrap();

        assert!(summary.is_fully_successful());
        assert_eq!(
            summary.peak_concurrency(),
            3,
            "all 3 independent tasks run concurrently"
        );
    }
}
