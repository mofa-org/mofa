//! AgentInput / AgentOutput serialization benchmarks
//!
//! Measures serde performance for core agent types at varying payload sizes.

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use mofa_bench::utils;

fn bench_agent_input_serde(c: &mut Criterion) {
    let mut group = c.benchmark_group("agent_input_serde");

    // Text inputs at different sizes
    let inputs = vec![
        ("small_text", utils::small_text_input()),
        ("medium_text", utils::medium_text_input()),
        ("large_text", utils::large_text_input()),
        ("json", utils::json_input()),
        ("map", utils::map_input()),
        ("binary", utils::binary_input()),
    ];

    for (name, input) in &inputs {
        group.bench_with_input(BenchmarkId::new("serialize", name), input, |b, input| {
            b.iter(|| {
                let serialized = serde_json::to_string(black_box(input)).unwrap();
                black_box(serialized);
            });
        });
    }

    for (name, input) in &inputs {
        let serialized = serde_json::to_string(input).unwrap();
        group.bench_with_input(
            BenchmarkId::new("deserialize", name),
            &serialized,
            |b, data| {
                b.iter(|| {
                    let deserialized: mofa_kernel::agent::types::AgentInput =
                        serde_json::from_str(black_box(data)).unwrap();
                    black_box(deserialized);
                });
            },
        );
    }

    group.finish();
}

fn bench_agent_output_serde(c: &mut Criterion) {
    let mut group = c.benchmark_group("agent_output_serde");

    let outputs = vec![
        ("simple_text", utils::simple_text_output()),
        ("rich", utils::rich_output()),
        ("large", utils::large_output()),
    ];

    for (name, output) in &outputs {
        group.bench_with_input(BenchmarkId::new("serialize", name), output, |b, output| {
            b.iter(|| {
                let serialized = serde_json::to_string(black_box(output)).unwrap();
                black_box(serialized);
            });
        });
    }

    for (name, output) in &outputs {
        let serialized = serde_json::to_string(output).unwrap();
        group.bench_with_input(
            BenchmarkId::new("deserialize", name),
            &serialized,
            |b, data| {
                b.iter(|| {
                    let deserialized: mofa_kernel::agent::types::AgentOutput =
                        serde_json::from_str(black_box(data)).unwrap();
                    black_box(deserialized);
                });
            },
        );
    }

    group.finish();
}

fn bench_agent_input_construction(c: &mut Criterion) {
    let mut group = c.benchmark_group("agent_input_construction");

    group.bench_function("text_from_str", |b| {
        b.iter(|| {
            let input = mofa_kernel::agent::types::AgentInput::text(black_box("Hello, agent!"));
            black_box(input);
        });
    });

    group.bench_function("text_from_string", |b| {
        b.iter(|| {
            let input =
                mofa_kernel::agent::types::AgentInput::from(black_box("Hello, agent!".to_string()));
            black_box(input);
        });
    });

    group.bench_function("json_construction", |b| {
        b.iter(|| {
            let input = utils::json_input();
            black_box(input);
        });
    });

    group.bench_function("to_text_conversion", |b| {
        let input = utils::json_input();
        b.iter(|| {
            let text = black_box(&input).to_text();
            black_box(text);
        });
    });

    group.bench_function("to_json_conversion", |b| {
        let input = utils::small_text_input();
        b.iter(|| {
            let json = black_box(&input).to_json();
            black_box(json);
        });
    });

    group.finish();
}

fn bench_agent_output_construction(c: &mut Criterion) {
    let mut group = c.benchmark_group("agent_output_construction");

    group.bench_function("simple_text", |b| {
        b.iter(|| {
            let output = mofa_kernel::agent::types::AgentOutput::text(black_box("Result"));
            black_box(output);
        });
    });

    group.bench_function("with_full_metadata", |b| {
        b.iter(|| {
            let output = utils::rich_output();
            black_box(output);
        });
    });

    group.bench_function("clone_rich_output", |b| {
        let output = utils::rich_output();
        b.iter(|| {
            let cloned = black_box(&output).clone();
            black_box(cloned);
        });
    });

    group.finish();
}

fn bench_bincode_serde(c: &mut Criterion) {
    let mut group = c.benchmark_group("agent_types_bincode");

    // Note: bincode serialization works for AgentInput/AgentOutput, but
    // deserialization fails with `DeserializeAnyNotSupported` because these
    // types use serde features (e.g. untagged enums) that bincode does not
    // support. Only AgentMessage (used on the bus) round-trips via bincode.

    let input = utils::json_input();
    group.bench_function("input_serialize", |b| {
        b.iter(|| {
            let encoded = bincode::serialize(black_box(&input)).unwrap();
            black_box(encoded);
        });
    });

    let output = utils::rich_output();
    group.bench_function("output_serialize", |b| {
        b.iter(|| {
            let encoded = bincode::serialize(black_box(&output)).unwrap();
            black_box(encoded);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_agent_input_serde,
    bench_agent_output_serde,
    bench_agent_input_construction,
    bench_agent_output_construction,
    bench_bincode_serde,
);
criterion_main!(benches);
