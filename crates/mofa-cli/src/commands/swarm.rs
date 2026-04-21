//! `mofa swarm run` — five-stage swarm pipeline.
//!
//! Stages:
//!   1. load      — parse YAML into SwarmRunConfig
//!   2. coverage  — per-task capability check against registered agents
//!   3. admission — SLA pre-flight (duration, token budget)
//!   4. execute   — Sequential or Parallel scheduler
//!   5. results   — summary table + audit trail + optional Prometheus output

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use comfy_table::Table;
use mofa_foundation::swarm::{
    AgentSpec, AuditEvent, AuditEventKind, CoordinationPattern, FailurePolicy, ParallelScheduler,
    SLAConfig, SchedulerSummary, SequentialScheduler, SubtaskDAG, SubtaskExecutorFn,
    SwarmScheduler, SwarmSchedulerConfig, SwarmSubtask,
};
use serde::Deserialize;

use crate::CliError;
use crate::cli::PatternArg;

// ── YAML input types ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct TaskSpec {
    id: String,
    description: String,
    #[serde(default)]
    capabilities: Vec<String>,
    #[serde(default = "default_complexity")]
    complexity: f64,
    #[serde(default)]
    depends_on: Vec<String>,
}

fn default_complexity() -> f64 {
    0.5
}

#[derive(Debug, Deserialize)]
struct SwarmRunConfig {
    #[serde(default = "default_name")]
    name: String,
    #[serde(default)]
    pattern: CoordinationPattern,
    #[serde(default)]
    agents: Vec<AgentSpec>,
    #[serde(default)]
    sla: SLAConfig,
    #[serde(default)]
    tasks: Vec<TaskSpec>,
}

fn default_name() -> String {
    "swarm".to_string()
}

// ── Coverage ─────────────────────────────────────────────────────────────────

pub struct CoverageResult {
    pub covered: Vec<String>,
    pub partial: Vec<String>,
    pub uncovered: Vec<String>,
    pub gaps: Vec<String>,
}

impl CoverageResult {
    pub fn is_fully_covered(&self) -> bool {
        self.uncovered.is_empty()
    }

    pub fn has_spof_risk(&self) -> bool {
        !self.partial.is_empty()
    }
}

fn check_coverage(agents: &[AgentSpec], tasks: &[TaskSpec]) -> CoverageResult {
    let mut covered = vec![];
    let mut partial = vec![];
    let mut uncovered = vec![];
    let mut gaps: HashSet<String> = HashSet::new();

    for task in tasks {
        if task.capabilities.is_empty() {
            covered.push(task.id.clone());
            continue;
        }
        let capable_count = agents
            .iter()
            .filter(|a| {
                task.capabilities
                    .iter()
                    .all(|cap| a.capabilities.contains(cap))
            })
            .count();

        match capable_count {
            0 => {
                uncovered.push(task.id.clone());
                for cap in &task.capabilities {
                    if !agents.iter().any(|a| a.capabilities.contains(cap)) {
                        gaps.insert(cap.clone());
                    }
                }
            }
            1 => partial.push(task.id.clone()),
            _ => covered.push(task.id.clone()),
        }
    }

    let mut gaps: Vec<String> = gaps.into_iter().collect();
    gaps.sort_unstable();

    CoverageResult {
        covered,
        partial,
        uncovered,
        gaps,
    }
}

// ── DAG construction ──────────────────────────────────────────────────────────

fn build_dag(name: &str, tasks: &[TaskSpec]) -> Result<SubtaskDAG, CliError> {
    let mut dag = SubtaskDAG::new(name);
    let mut id_map = HashMap::new();

    for spec in tasks {
        let subtask = SwarmSubtask::new(&spec.id, &spec.description)
            .with_capabilities(spec.capabilities.clone())
            .with_complexity(spec.complexity);
        let idx = dag.add_task(subtask);
        id_map.insert(spec.id.as_str(), idx);
    }

    for spec in tasks {
        let to = id_map[spec.id.as_str()];
        for dep in &spec.depends_on {
            let from = id_map
                .get(dep.as_str())
                .copied()
                .ok_or_else(|| CliError::Other(format!("unknown depends_on: {dep}")))?;
            dag.add_dependency(from, to)
                .map_err(|e| CliError::Other(e.to_string()))?;
        }
    }

    Ok(dag)
}

// ── Pattern selection ─────────────────────────────────────────────────────────

fn select_pattern(
    requested: CoordinationPattern,
    tasks: &[TaskSpec],
) -> (CoordinationPattern, Option<&'static str>) {
    let all_independent = tasks.iter().all(|t| t.depends_on.is_empty());
    if requested == CoordinationPattern::Sequential && all_independent && tasks.len() > 1 {
        return (
            CoordinationPattern::Parallel,
            Some("all tasks are independent — switching to Parallel for better throughput"),
        );
    }
    (requested, None)
}

// ── Executor ──────────────────────────────────────────────────────────────────

fn make_executor() -> SubtaskExecutorFn {
    Arc::new(|_idx, task| Box::pin(async move { Ok(format!("completed: {}", task.id)) }))
}

// ── Audit events ──────────────────────────────────────────────────────────────

fn collect_audit_events(config_name: &str, summary: &SchedulerSummary) -> Vec<AuditEvent> {
    let mut events = vec![AuditEvent::new(
        AuditEventKind::SwarmStarted,
        format!("swarm started: {config_name}"),
    )];
    for r in &summary.results {
        if r.outcome.is_success() {
            events.push(AuditEvent::new(
                AuditEventKind::SubtaskCompleted,
                format!(
                    "task {} succeeded ({:.0}ms)",
                    r.task_id,
                    r.wall_time.as_millis()
                ),
            ));
        } else {
            events.push(AuditEvent::new(
                AuditEventKind::SubtaskFailed,
                format!("task {} failed", r.task_id),
            ));
        }
    }
    events.push(AuditEvent::new(
        AuditEventKind::SwarmCompleted,
        format!(
            "{}/{} tasks succeeded",
            summary.succeeded, summary.total_tasks
        ),
    ));
    events
}

// ── Display helpers ───────────────────────────────────────────────────────────

fn print_stage(n: u8, total: u8, label: &str) {
    println!("\n[{n}/{total}] {label}");
}

fn print_coverage_report(cov: &CoverageResult) {
    if !cov.covered.is_empty() {
        println!(
            "      covered  ({} tasks):  {}",
            cov.covered.len(),
            cov.covered.join(", ")
        );
    }
    if !cov.partial.is_empty() {
        println!(
            "      partial  ({} tasks):  {}",
            cov.partial.len(),
            cov.partial.join(", ")
        );
    }
    if !cov.uncovered.is_empty() {
        println!(
            "      uncovered ({} tasks): {}",
            cov.uncovered.len(),
            cov.uncovered.join(", ")
        );
    }
    if !cov.gaps.is_empty() {
        println!("      missing capabilities: {}", cov.gaps.join(", "));
    }
    if cov.has_spof_risk() {
        println!("      warning: partial tasks have single-agent coverage (spof risk)");
    }
}

fn print_summary_table(summary: &SchedulerSummary) {
    let mut table = Table::new();
    table.load_preset(comfy_table::presets::UTF8_FULL);
    table.set_header([
        "pattern",
        "tasks",
        "succeeded",
        "failed",
        "skipped",
        "wall time",
    ]);
    table.add_row([
        summary.pattern.to_string(),
        summary.total_tasks.to_string(),
        summary.succeeded.to_string(),
        summary.failed.to_string(),
        summary.skipped.to_string(),
        format!("{:.2}s", summary.total_wall_time.as_secs_f64()),
    ]);
    println!("{table}");
}

fn print_audit_trail(events: &[AuditEvent]) {
    println!("\naudit trail:");
    for ev in events {
        let kind = format!("{:?}", ev.kind).to_lowercase();
        println!("  [{kind}] {}", ev.description);
    }
}

fn print_prometheus_metrics(summary: &SchedulerSummary) {
    let p = summary.pattern.to_string();
    println!("\n# HELP mofa_swarm_scheduler_runs_total total scheduler executions per pattern");
    println!("# TYPE mofa_swarm_scheduler_runs_total counter");
    println!("mofa_swarm_scheduler_runs_total{{pattern=\"{p}\"}} 1");
    println!("# HELP mofa_swarm_tasks_total total subtasks by pattern and status");
    println!("# TYPE mofa_swarm_tasks_total counter");
    println!(
        "mofa_swarm_tasks_total{{pattern=\"{p}\",status=\"succeeded\"}} {}",
        summary.succeeded
    );
    println!(
        "mofa_swarm_tasks_total{{pattern=\"{p}\",status=\"failed\"}} {}",
        summary.failed
    );
    println!(
        "mofa_swarm_tasks_total{{pattern=\"{p}\",status=\"skipped\"}} {}",
        summary.skipped
    );
    println!("# HELP mofa_swarm_scheduler_duration_seconds wall time per scheduler run in seconds");
    println!("# TYPE mofa_swarm_scheduler_duration_seconds gauge");
    println!(
        "mofa_swarm_scheduler_duration_seconds{{pattern=\"{p}\"}} {:.6}",
        summary.total_wall_time.as_secs_f64()
    );
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use mofa_foundation::swarm::AgentSpec;

    fn agent(id: &str, caps: &[&str]) -> AgentSpec {
        AgentSpec {
            id: id.to_string(),
            capabilities: caps.iter().map(|s| s.to_string()).collect(),
            model: None,
            cost_per_token: None,
            max_concurrency: 1,
        }
    }

    fn task(id: &str, caps: &[&str], deps: &[&str]) -> TaskSpec {
        TaskSpec {
            id: id.to_string(),
            description: id.to_string(),
            capabilities: caps.iter().map(|s| s.to_string()).collect(),
            complexity: 0.5,
            depends_on: deps.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn coverage_all_covered_when_two_or_more_agents() {
        let agents = vec![agent("a", &["extract"]), agent("b", &["extract"])];
        let tasks = vec![task("t1", &["extract"], &[])];
        let cov = check_coverage(&agents, &tasks);
        assert_eq!(cov.covered, vec!["t1"]);
        assert!(cov.partial.is_empty());
        assert!(cov.uncovered.is_empty());
    }

    #[test]
    fn coverage_partial_when_single_agent() {
        let agents = vec![agent("a", &["translate"])];
        let tasks = vec![task("t1", &["translate"], &[])];
        let cov = check_coverage(&agents, &tasks);
        assert_eq!(cov.partial, vec!["t1"]);
        assert!(cov.covered.is_empty());
        assert!(cov.uncovered.is_empty());
    }

    #[test]
    fn coverage_uncovered_and_gaps_when_no_agent() {
        let agents = vec![agent("a", &["search"])];
        let tasks = vec![task("t1", &["write"], &[])];
        let cov = check_coverage(&agents, &tasks);
        assert_eq!(cov.uncovered, vec!["t1"]);
        assert_eq!(cov.gaps, vec!["write"]);
    }

    #[test]
    fn coverage_no_cap_task_is_always_covered() {
        let agents: Vec<AgentSpec> = vec![];
        let tasks = vec![task("t1", &[], &[])];
        let cov = check_coverage(&agents, &tasks);
        assert_eq!(cov.covered, vec!["t1"]);
    }

    #[test]
    fn select_pattern_recommends_parallel_for_independent_tasks() {
        let tasks = vec![task("a", &[], &[]), task("b", &[], &[])];
        let (pattern, note) = select_pattern(CoordinationPattern::Sequential, &tasks);
        assert_eq!(pattern, CoordinationPattern::Parallel);
        assert!(note.is_some());
    }

    #[test]
    fn select_pattern_keeps_sequential_when_deps_exist() {
        let tasks = vec![task("a", &[], &[]), task("b", &[], &["a"])];
        let (pattern, note) = select_pattern(CoordinationPattern::Sequential, &tasks);
        assert_eq!(pattern, CoordinationPattern::Sequential);
        assert!(note.is_none());
    }

    #[test]
    fn build_dag_correct_node_count() {
        let tasks = vec![task("a", &[], &[]), task("b", &[], &[])];
        let dag = build_dag("test", &tasks).unwrap();
        assert_eq!(dag.task_count(), 2);
    }

    #[test]
    fn build_dag_errors_on_unknown_depends_on() {
        let tasks = vec![task("a", &[], &["nonexistent"])];
        assert!(build_dag("test", &tasks).is_err());
    }
}

// ── Entry point ───────────────────────────────────────────────────────────────

pub async fn run(
    file: &Path,
    dry_run: bool,
    metrics: bool,
    pattern_override: Option<PatternArg>,
    timeout_secs: u64,
) -> Result<(), CliError> {
    let total: u8 = if dry_run { 3 } else { 5 };

    // 1. load
    print_stage(1, total, "loading config");
    let raw = std::fs::read_to_string(file)?;
    let cfg: SwarmRunConfig =
        serde_yaml::from_str(&raw).map_err(|e| CliError::Other(e.to_string()))?;
    println!("      name:    {}", cfg.name);
    println!("      pattern: {}", cfg.pattern);
    println!("      agents:  {}", cfg.agents.len());
    println!("      tasks:   {}", cfg.tasks.len());

    // 2. coverage
    print_stage(2, total, "coverage check");
    let coverage = check_coverage(&cfg.agents, &cfg.tasks);
    print_coverage_report(&coverage);
    if !coverage.is_fully_covered() {
        if dry_run {
            println!(
                "\n  blocked: {} task(s) have no capable agent",
                coverage.uncovered.len()
            );
            return Ok(());
        }
        eprintln!(
            "warning: proceeding with {} uncovered task(s)",
            coverage.uncovered.len()
        );
    }

    // 3. admission
    print_stage(3, total, "admission check");
    if cfg.sla.max_duration_secs > 0 {
        let estimated: u64 = cfg.tasks.iter().map(|t| (t.complexity * 30.0) as u64).sum();
        println!(
            "      sla max: {}s   estimated: {}s   tokens budget: {}",
            cfg.sla.max_duration_secs, estimated, cfg.sla.max_cost_tokens
        );
        if estimated > cfg.sla.max_duration_secs {
            if dry_run {
                println!("  blocked: estimated duration ({estimated}s) exceeds sla limit");
                return Ok(());
            }
            eprintln!(
                "warning: estimated duration ({estimated}s) may exceed sla limit ({}s)",
                cfg.sla.max_duration_secs
            );
        } else {
            println!("      ok");
        }
    } else {
        println!("      no sla constraints configured");
    }

    if dry_run {
        println!("\n  dry-run complete — no tasks executed");
        return Ok(());
    }

    // 4. execute
    print_stage(4, total, "executing");
    let mut dag = build_dag(&cfg.name, &cfg.tasks)?;

    let requested = match pattern_override {
        Some(PatternArg::Sequential) => CoordinationPattern::Sequential,
        Some(PatternArg::Parallel) => CoordinationPattern::Parallel,
        None => cfg.pattern,
    };
    let (pattern, note) = select_pattern(requested, &cfg.tasks);
    if let Some(msg) = note {
        println!("      {msg}");
    }
    println!("      pattern: {pattern}");

    let sched_cfg = SwarmSchedulerConfig {
        task_timeout: Duration::from_secs(timeout_secs),
        failure_policy: FailurePolicy::Continue,
        concurrency_limit: None,
    };
    let executor = make_executor();

    let summary: SchedulerSummary = match pattern {
        CoordinationPattern::Parallel => ParallelScheduler::with_config(sched_cfg)
            .execute(&mut dag, executor)
            .await
            .map_err(|e| CliError::Other(e.to_string()))?,
        CoordinationPattern::Sequential | _ => {
            if !matches!(pattern, CoordinationPattern::Sequential) {
                eprintln!(
                    "note: {pattern} scheduler not yet implemented — falling back to Sequential"
                );
            }
            SequentialScheduler::with_config(sched_cfg)
                .execute(&mut dag, executor)
                .await
                .map_err(|e| CliError::Other(e.to_string()))?
        }
    };

    // 5. results
    print_stage(5, total, "results");
    print_summary_table(&summary);

    let events = collect_audit_events(&cfg.name, &summary);
    print_audit_trail(&events);

    if metrics {
        print_prometheus_metrics(&summary);
    }

    Ok(())
}
