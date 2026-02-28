//! AgentMessage serialization benchmarks
//!
//! Measures serde performance for all AgentMessage variants
//! using both JSON and bincode formats.

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use mofa_bench::utils;
use mofa_kernel::message::AgentMessage;

fn bench_message_json_serde(c: &mut Criterion) {
    let mut group = c.benchmark_group("agent_message_json");

    let messages: Vec<(&str, AgentMessage)> = vec![
        ("task_request", utils::task_request_message()),
        ("task_response", utils::task_response_message()),
        ("state_sync", utils::state_sync_message()),
        ("stream_message", utils::stream_message()),
    ];

    for (name, msg) in &messages {
        group.bench_with_input(BenchmarkId::new("serialize", name), msg, |b, msg| {
            b.iter(|| {
                let json = serde_json::to_string(black_box(msg)).unwrap();
                black_box(json);
            });
        });
    }

    for (name, msg) in &messages {
        let json = serde_json::to_string(msg).unwrap();
        group.bench_with_input(BenchmarkId::new("deserialize", name), &json, |b, data| {
            b.iter(|| {
                let decoded: AgentMessage = serde_json::from_str(black_box(data)).unwrap();
                black_box(decoded);
            });
        });
    }

    // Batch serialization (serialize 100 messages)
    group.bench_function("batch_serialize_100", |b| {
        let msgs: Vec<_> = (0..100)
            .map(|i| AgentMessage::TaskRequest {
                task_id: format!("task-{i:04}"),
                content: format!("Process item {i} from the queue"),
            })
            .collect();

        b.iter(|| {
            let results: Vec<String> = msgs
                .iter()
                .map(|m| serde_json::to_string(black_box(m)).unwrap())
                .collect();
            black_box(results);
        });
    });

    group.finish();
}

fn bench_message_bincode_serde(c: &mut Criterion) {
    let mut group = c.benchmark_group("agent_message_bincode");

    let messages: Vec<(&str, AgentMessage)> = vec![
        ("task_request", utils::task_request_message()),
        ("task_response", utils::task_response_message()),
        ("state_sync", utils::state_sync_message()),
        ("stream_message", utils::stream_message()),
    ];

    for (name, msg) in &messages {
        group.bench_with_input(BenchmarkId::new("serialize", name), msg, |b, msg| {
            b.iter(|| {
                let encoded = bincode::serialize(black_box(msg)).unwrap();
                black_box(encoded);
            });
        });
    }

    for (name, msg) in &messages {
        let encoded = bincode::serialize(msg).unwrap();
        group.bench_with_input(
            BenchmarkId::new("deserialize", name),
            &encoded,
            |b, data| {
                b.iter(|| {
                    let decoded: AgentMessage = bincode::deserialize(black_box(data)).unwrap();
                    black_box(decoded);
                });
            },
        );
    }

    group.finish();
}

fn bench_message_construction(c: &mut Criterion) {
    let mut group = c.benchmark_group("agent_message_construction");

    group.bench_function("task_request", |b| {
        b.iter(|| {
            let msg = AgentMessage::TaskRequest {
                task_id: black_box("task-001".to_string()),
                content: black_box("Analyze the data".to_string()),
            };
            black_box(msg);
        });
    });

    group.bench_function("task_response", |b| {
        b.iter(|| {
            let msg = AgentMessage::TaskResponse {
                task_id: black_box("task-001".to_string()),
                result: black_box("Analysis complete".to_string()),
                status: mofa_kernel::message::TaskStatus::Success,
            };
            black_box(msg);
        });
    });

    group.bench_function("stream_message_1kb", |b| {
        let payload = vec![0u8; 1024];
        b.iter(|| {
            let msg = AgentMessage::StreamMessage {
                stream_id: black_box("stream-001".to_string()),
                message: black_box(payload.clone()),
                sequence: 1,
            };
            black_box(msg);
        });
    });

    group.bench_function("clone_task_request", |b| {
        let msg = utils::task_request_message();
        b.iter(|| {
            let cloned = black_box(&msg).clone();
            black_box(cloned);
        });
    });

    group.finish();
}

fn bench_task_request_struct(c: &mut Criterion) {
    let mut group = c.benchmark_group("task_request_struct");

    group.bench_function("construction", |b| {
        b.iter(|| {
            let req = utils::sample_task_request();
            black_box(req);
        });
    });

    let req = utils::sample_task_request();
    group.bench_function("json_serialize", |b| {
        b.iter(|| {
            let json = serde_json::to_string(black_box(&req)).unwrap();
            black_box(json);
        });
    });

    let json = serde_json::to_string(&req).unwrap();
    group.bench_function("json_deserialize", |b| {
        b.iter(|| {
            let decoded: mofa_kernel::message::TaskRequest =
                serde_json::from_str(black_box(&json)).unwrap();
            black_box(decoded);
        });
    });

    group.bench_function("priority_comparison", |b| {
        use mofa_kernel::message::TaskPriority;
        let priorities = [
            TaskPriority::Critical,
            TaskPriority::Highest,
            TaskPriority::High,
            TaskPriority::Medium,
            TaskPriority::Normal,
            TaskPriority::Low,
        ];
        b.iter(|| {
            for i in 0..priorities.len() {
                for j in 0..priorities.len() {
                    let cmp = black_box(&priorities[i]).cmp(black_box(&priorities[j]));
                    black_box(cmp);
                }
            }
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_message_json_serde,
    bench_message_bincode_serde,
    bench_message_construction,
    bench_task_request_struct,
);
criterion_main!(benches);
