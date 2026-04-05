//! AgentState transition benchmarks
//!
//! Measures state machine transition speed, permission checks,
//! and full lifecycle traversals.

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use mofa_kernel::agent::AgentState;

fn bench_state_transitions(c: &mut Criterion) {
    let mut group = c.benchmark_group("agent_state_transitions");

    // Single valid transition
    group.bench_function("created_to_initializing", |b| {
        b.iter(|| {
            let state = AgentState::Created;
            let _ = black_box(state.transition_to(black_box(AgentState::Initializing)));
        });
    });

    group.bench_function("ready_to_running", |b| {
        b.iter(|| {
            let state = AgentState::Ready;
            let _ = black_box(state.transition_to(black_box(AgentState::Running)));
        });
    });

    // Invalid transition (should return error)
    group.bench_function("created_to_running_invalid", |b| {
        b.iter(|| {
            let state = AgentState::Created;
            let _ = black_box(state.transition_to(black_box(AgentState::Running)));
        });
    });

    // Full lifecycle: Created → Initializing → Ready → Running → ShuttingDown → Shutdown
    group.bench_function("full_lifecycle", |b| {
        b.iter(|| {
            let state = AgentState::Created;
            let state = state
                .transition_to(AgentState::Initializing)
                .unwrap_or(AgentState::Created);
            let state = state
                .transition_to(AgentState::Ready)
                .unwrap_or(AgentState::Initializing);
            let state = state
                .transition_to(AgentState::Running)
                .unwrap_or(AgentState::Ready);
            let state = state
                .transition_to(AgentState::ShuttingDown)
                .unwrap_or(AgentState::Running);
            let state = state
                .transition_to(AgentState::Shutdown)
                .unwrap_or(AgentState::ShuttingDown);
            black_box(state);
        });
    });

    group.finish();
}

fn bench_can_transition(c: &mut Criterion) {
    let mut group = c.benchmark_group("agent_state_can_transition");

    let states = vec![
        AgentState::Created,
        AgentState::Initializing,
        AgentState::Ready,
        AgentState::Running,
        AgentState::Paused,
        AgentState::Failed,
    ];

    let targets = vec![
        AgentState::Initializing,
        AgentState::Ready,
        AgentState::Running,
        AgentState::ShuttingDown,
        AgentState::Failed,
    ];

    // Benchmark checking all state × target combinations
    group.bench_function("all_combinations", |b| {
        b.iter(|| {
            for state in &states {
                for target in &targets {
                    let can = state.can_transition_to(black_box(target));
                    black_box(can);
                }
            }
        });
    });

    group.finish();
}

fn bench_state_queries(c: &mut Criterion) {
    let mut group = c.benchmark_group("agent_state_queries");

    let states = vec![
        AgentState::Created,
        AgentState::Initializing,
        AgentState::Ready,
        AgentState::Running,
        AgentState::Paused,
        AgentState::Failed,
        AgentState::Shutdown,
        AgentState::Destroyed,
        AgentState::Error("test error".to_string()),
    ];

    group.bench_function("is_active_all_states", |b| {
        b.iter(|| {
            for state in &states {
                let active = black_box(state).is_active();
                black_box(active);
            }
        });
    });

    group.bench_function("is_terminal_all_states", |b| {
        b.iter(|| {
            for state in &states {
                let terminal = black_box(state).is_terminal();
                black_box(terminal);
            }
        });
    });

    group.bench_function("display_all_states", |b| {
        b.iter(|| {
            for state in &states {
                let display = format!("{}", black_box(state));
                black_box(display);
            }
        });
    });

    group.bench_function("clone_all_states", |b| {
        b.iter(|| {
            for state in &states {
                let cloned = black_box(state).clone();
                black_box(cloned);
            }
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_state_transitions,
    bench_can_transition,
    bench_state_queries,
);
criterion_main!(benches);
