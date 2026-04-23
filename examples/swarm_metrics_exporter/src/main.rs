//! SwarmMetricsExporter demo.
//!
//! Simulates four scheduler runs across three coordination patterns,
//! then prints the Prometheus text-format output to stdout.
//!
//! Run: cargo run -p swarm_metrics_exporter

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
        hitl_stats: None,
    }
}

fn main() {
    let exporter = SwarmMetricsExporter::new();

    exporter.record_scheduler_run(&make_summary(CoordinationPattern::Sequential, 5, 5, 0, 0, 80));
    exporter.record_scheduler_run(&make_summary(CoordinationPattern::Parallel, 8, 7, 1, 0, 420));
    exporter.record_scheduler_run(&make_summary(CoordinationPattern::Parallel, 6, 5, 0, 1, 310));
    exporter.record_scheduler_run(&make_summary(CoordinationPattern::MapReduce, 12, 10, 2, 0, 3800));

    let mut m1 = SwarmMetrics::default();
    m1.record_hitl_intervention();
    m1.add_tokens(4200);
    exporter.record_swarm_result(&m1);

    let mut m2 = SwarmMetrics::default();
    m2.add_tokens(7800);
    exporter.record_swarm_result(&m2);

    println!("{}", exporter.render());
}
