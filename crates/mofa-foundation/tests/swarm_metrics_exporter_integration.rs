use std::sync::Arc;
use std::time::Duration;

use mofa_foundation::swarm::{
    CoordinationPattern, SchedulerSummary, SwarmMetrics, SwarmMetricsExporter,
};

fn make_summary(
    pattern: CoordinationPattern,
    total: usize,
    succeeded: usize,
    failed: usize,
    skipped: usize,
    wall_ms: u64,
) -> SchedulerSummary {
    SchedulerSummary {
        pattern,
        total_tasks: total,
        succeeded,
        failed,
        skipped,
        total_wall_time: Duration::from_millis(wall_ms),
        results: vec![],
    }
}

// --- counter tests ---

#[test]
fn empty_exporter_renders_empty_string() {
    let exporter = SwarmMetricsExporter::new();
    assert_eq!(exporter.render(), "");
}

#[test]
fn record_single_run_increments_runs_counter() {
    let exporter = SwarmMetricsExporter::new();
    let s = make_summary(CoordinationPattern::Sequential, 5, 4, 1, 0, 200);
    exporter.record_scheduler_run(&s);
    let out = exporter.render();
    assert!(out.contains("mofa_swarm_scheduler_runs_total{pattern=\"Sequential\"} 1"), "{out}");
}

#[test]
fn record_multiple_runs_accumulate_counters() {
    let exporter = SwarmMetricsExporter::new();
    for _ in 0..3 {
        let s = make_summary(CoordinationPattern::Parallel, 4, 4, 0, 0, 100);
        exporter.record_scheduler_run(&s);
    }
    let out = exporter.render();
    assert!(out.contains("mofa_swarm_scheduler_runs_total{pattern=\"Parallel\"} 3"), "{out}");
    assert!(out.contains("mofa_swarm_tasks_total{pattern=\"Parallel\",status=\"succeeded\"} 12"), "{out}");
}

#[test]
fn tasks_total_succeeded_failed_skipped_labels_correct() {
    let exporter = SwarmMetricsExporter::new();
    let s = make_summary(CoordinationPattern::Debate, 10, 7, 2, 1, 500);
    exporter.record_scheduler_run(&s);
    let out = exporter.render();
    assert!(out.contains("mofa_swarm_tasks_total{pattern=\"Debate\",status=\"succeeded\"} 7"), "{out}");
    assert!(out.contains("mofa_swarm_tasks_total{pattern=\"Debate\",status=\"failed\"} 2"), "{out}");
    assert!(out.contains("mofa_swarm_tasks_total{pattern=\"Debate\",status=\"skipped\"} 1"), "{out}");
}

#[test]
fn two_patterns_do_not_bleed_into_each_other() {
    let exporter = SwarmMetricsExporter::new();
    exporter.record_scheduler_run(&make_summary(CoordinationPattern::Sequential, 3, 3, 0, 0, 100));
    exporter.record_scheduler_run(&make_summary(CoordinationPattern::Parallel, 5, 2, 3, 0, 300));
    let out = exporter.render();
    assert!(out.contains("mofa_swarm_scheduler_runs_total{pattern=\"Sequential\"} 1"), "{out}");
    assert!(out.contains("mofa_swarm_scheduler_runs_total{pattern=\"Parallel\"} 1"), "{out}");
    assert!(out.contains("mofa_swarm_tasks_total{pattern=\"Sequential\",status=\"succeeded\"} 3"), "{out}");
    assert!(out.contains("mofa_swarm_tasks_total{pattern=\"Parallel\",status=\"failed\"} 3"), "{out}");
}

// --- histogram tests ---

#[test]
fn fast_run_lands_in_sub_100ms_bucket() {
    let exporter = SwarmMetricsExporter::new();
    // 50 ms -> should land in 0.1s bucket
    exporter.record_scheduler_run(&make_summary(CoordinationPattern::MapReduce, 1, 1, 0, 0, 50));
    let out = exporter.render();
    assert!(
        out.contains("mofa_swarm_scheduler_duration_seconds_bucket{pattern=\"MapReduce\",le=\"0.1\"} 1"),
        "{out}"
    );
}

#[test]
fn slow_run_does_not_land_in_sub_100ms_bucket() {
    let exporter = SwarmMetricsExporter::new();
    // 5000 ms -> only +Inf bucket
    exporter.record_scheduler_run(&make_summary(CoordinationPattern::Consensus, 1, 1, 0, 0, 5000));
    let out = exporter.render();
    assert!(
        out.contains("mofa_swarm_scheduler_duration_seconds_bucket{pattern=\"Consensus\",le=\"0.1\"} 0"),
        "{out}"
    );
    assert!(
        out.contains("mofa_swarm_scheduler_duration_seconds_bucket{pattern=\"Consensus\",le=\"+Inf\"} 1"),
        "{out}"
    );
}

#[test]
fn histogram_sum_and_count_correct() {
    let exporter = SwarmMetricsExporter::new();
    exporter.record_scheduler_run(&make_summary(CoordinationPattern::Routing, 2, 2, 0, 0, 200));
    exporter.record_scheduler_run(&make_summary(CoordinationPattern::Routing, 2, 2, 0, 0, 800));
    let out = exporter.render();
    assert!(out.contains("mofa_swarm_scheduler_duration_seconds_count{pattern=\"Routing\"} 2"), "{out}");
    assert!(out.contains("mofa_swarm_scheduler_duration_seconds_sum{pattern=\"Routing\"} 1.000000"), "{out}");
}

#[test]
fn histogram_buckets_are_cumulative() {
    let exporter = SwarmMetricsExporter::new();
    // 50 ms -> hits le=0.1, le=0.5, le=1.0 ... all higher buckets too
    exporter.record_scheduler_run(&make_summary(CoordinationPattern::Supervision, 1, 1, 0, 0, 50));
    let out = exporter.render();
    assert!(out.contains("le=\"0.1\"} 1"), "{out}");
    assert!(out.contains("le=\"0.5\"} 1"), "{out}");
    assert!(out.contains("le=\"120\"} 1"), "{out}");
    assert!(out.contains("le=\"+Inf\"} 1"), "{out}");
}

// --- HITL / token tests ---

#[test]
fn hitl_interventions_accumulate() {
    let exporter = SwarmMetricsExporter::new();
    let mut m = SwarmMetrics::default();
    m.record_hitl_intervention();
    m.record_hitl_intervention();
    exporter.record_swarm_result(&m);
    exporter.record_swarm_result(&m);
    let out = exporter.render();
    assert!(out.contains("mofa_swarm_hitl_interventions_total 4"), "{out}");
}

#[test]
fn tokens_total_accumulates() {
    let exporter = SwarmMetricsExporter::new();
    let mut m = SwarmMetrics::default();
    m.add_tokens(1000);
    exporter.record_swarm_result(&m);
    exporter.record_swarm_result(&m);
    let out = exporter.render();
    assert!(out.contains("mofa_swarm_tokens_total 2000"), "{out}");
}

// --- render format tests ---

#[test]
fn render_contains_help_and_type_lines() {
    let exporter = SwarmMetricsExporter::new();
    exporter.record_scheduler_run(&make_summary(CoordinationPattern::Sequential, 1, 1, 0, 0, 10));
    let out = exporter.render();
    assert!(out.contains("# HELP mofa_swarm_scheduler_runs_total"), "{out}");
    assert!(out.contains("# TYPE mofa_swarm_scheduler_runs_total counter"), "{out}");
    assert!(out.contains("# HELP mofa_swarm_tasks_total"), "{out}");
    assert!(out.contains("# HELP mofa_swarm_scheduler_duration_seconds"), "{out}");
    assert!(out.contains("# TYPE mofa_swarm_scheduler_duration_seconds histogram"), "{out}");
}

#[test]
fn reset_clears_all_state() {
    let exporter = SwarmMetricsExporter::new();
    exporter.record_scheduler_run(&make_summary(CoordinationPattern::Parallel, 4, 4, 0, 0, 100));
    exporter.reset();
    assert_eq!(exporter.render(), "");
}

// --- concurrency test ---

#[test]
fn concurrent_record_no_data_race() {
    let exporter = Arc::new(SwarmMetricsExporter::new());
    let handles: Vec<_> = (0..8)
        .map(|_| {
            let e = Arc::clone(&exporter);
            std::thread::spawn(move || {
                for _ in 0..10 {
                    e.record_scheduler_run(&make_summary(
                        CoordinationPattern::Parallel,
                        5,
                        5,
                        0,
                        0,
                        100,
                    ));
                }
            })
        })
        .collect();
    for h in handles {
        h.join().unwrap();
    }
    let out = exporter.render();
    assert!(out.contains("mofa_swarm_scheduler_runs_total{pattern=\"Parallel\"} 80"), "{out}");
    assert!(out.contains("mofa_swarm_tasks_total{pattern=\"Parallel\",status=\"succeeded\"} 400"), "{out}");
}
