//! AgentBus message throughput benchmarks
//!
//! Measures bus creation, channel registration, and message send/receive performance.

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use mofa_bench::utils;
use mofa_kernel::bus::{AgentBus, CommunicationMode};

fn bench_bus_creation(c: &mut Criterion) {
    c.bench_function("agent_bus_new", |b| {
        b.iter(|| {
            let bus = AgentBus::new();
            black_box(bus);
        });
    });
}

fn bench_channel_registration(c: &mut Criterion) {
    let mut group = c.benchmark_group("channel_registration");
    let rt = tokio::runtime::Runtime::new().unwrap();

    for count in [1, 10, 50, 100] {
        group.bench_with_input(
            BenchmarkId::new("register_p2p", count),
            &count,
            |b, &count| {
                b.iter(|| {
                    let bus = AgentBus::new();
                    rt.block_on(async {
                        for i in 0..count {
                            let meta =
                                utils::sample_agent_metadata(&format!("agent-{i}"));
                            let _ = bus
                                .register_channel(
                                    &meta,
                                    CommunicationMode::PointToPoint(format!("peer-{i}")),
                                )
                                .await;
                        }
                    });
                    black_box(&bus);
                });
            },
        );
    }

    for count in [1, 10, 50] {
        group.bench_with_input(
            BenchmarkId::new("register_broadcast", count),
            &count,
            |b, &count| {
                b.iter(|| {
                    let bus = AgentBus::new();
                    rt.block_on(async {
                        for i in 0..count {
                            let meta =
                                utils::sample_agent_metadata(&format!("agent-{i}"));
                            let _ = bus
                                .register_channel(&meta, CommunicationMode::Broadcast)
                                .await;
                        }
                    });
                    black_box(&bus);
                });
            },
        );
    }

    group.finish();
}

fn bench_broadcast_send(c: &mut Criterion) {
    let mut group = c.benchmark_group("message_send");
    let rt = tokio::runtime::Runtime::new().unwrap();

    // Broadcast send
    group.bench_function("broadcast", |b| {
        let bus = AgentBus::new();
        let message = utils::task_request_message();

        b.iter(|| {
            rt.block_on(async {
                // Broadcast doesn't require channel registration
                let _ = bus
                    .send_message("sender-1", CommunicationMode::Broadcast, black_box(&message))
                    .await;
            });
        });
    });

    // Point-to-point send (requires registered channel)
    group.bench_function("p2p", |b| {
        let bus = AgentBus::new();
        let receiver_meta = utils::sample_agent_metadata("receiver-1");
        rt.block_on(async {
            bus.register_channel(
                &receiver_meta,
                CommunicationMode::PointToPoint("sender-1".to_string()),
            )
            .await
            .unwrap();
        });

        let message = utils::task_request_message();

        b.iter(|| {
            rt.block_on(async {
                let _ = bus
                    .send_message(
                        "sender-1",
                        CommunicationMode::PointToPoint("receiver-1".to_string()),
                        black_box(&message),
                    )
                    .await;
            });
        });
    });

    group.finish();
}

fn bench_message_serialization_for_bus(c: &mut Criterion) {
    let mut group = c.benchmark_group("message_bus_serialization");

    let messages: Vec<(&str, mofa_kernel::message::AgentMessage)> = vec![
        ("task_request", utils::task_request_message()),
        ("task_response", utils::task_response_message()),
        ("state_sync", utils::state_sync_message()),
        ("stream_message", utils::stream_message()),
    ];

    // The bus uses bincode internally for serialization
    for (name, msg) in &messages {
        group.bench_with_input(BenchmarkId::new("bincode_encode", name), msg, |b, msg| {
            b.iter(|| {
                let encoded = bincode::serialize(black_box(msg)).unwrap();
                black_box(encoded);
            });
        });
    }

    for (name, msg) in &messages {
        let encoded = bincode::serialize(msg).unwrap();
        group.bench_with_input(
            BenchmarkId::new("bincode_decode", name),
            &encoded,
            |b, data| {
                b.iter(|| {
                    let decoded: mofa_kernel::message::AgentMessage =
                        bincode::deserialize(black_box(data)).unwrap();
                    black_box(decoded);
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_bus_creation,
    bench_channel_registration,
    bench_broadcast_send,
    bench_message_serialization_for_bus,
);
criterion_main!(benches);
