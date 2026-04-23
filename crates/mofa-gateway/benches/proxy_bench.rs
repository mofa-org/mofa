//! Performance benchmarks for mofa-local-llm proxy.
//!
//! Measures:
//! - Proxy overhead (latency added by gateway)
//! - Throughput (requests per second)
//! - Concurrent request handling

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use std::time::Duration;

/// Benchmark direct requests to mofa-local-llm (baseline).
fn bench_direct_request(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("direct_models_list", |b| {
        b.to_async(&runtime).iter(|| async {
            let client = reqwest::Client::new();
            let response = client.get("http://localhost:8000/v1/models").send().await;
            black_box(response)
        });
    });
}

/// Benchmark proxied requests through gateway.
fn bench_proxied_request(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("proxied_models_list", |b| {
        b.to_async(&runtime).iter(|| async {
            let client = reqwest::Client::new();
            let response = client.get("http://localhost:8080/v1/models").send().await;
            black_box(response)
        });
    });
}

/// Benchmark proxy overhead by comparing direct vs proxied.
fn bench_proxy_overhead(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let client = reqwest::Client::new();

    let mut group = c.benchmark_group("proxy_overhead");

    // Direct request
    group.bench_function("direct", |b| {
        b.to_async(&runtime).iter(|| async {
            let response = client.get("http://localhost:8000/v1/models").send().await;
            black_box(response)
        });
    });

    // Proxied request
    group.bench_function("proxied", |b| {
        b.to_async(&runtime).iter(|| async {
            let response = client.get("http://localhost:8080/v1/models").send().await;
            black_box(response)
        });
    });

    group.finish();
}

/// Benchmark concurrent request handling.
fn bench_concurrent_requests(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();

    let mut group = c.benchmark_group("concurrent_requests");

    for concurrency in [1, 5, 10, 20].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(concurrency),
            concurrency,
            |b, &concurrency| {
                b.to_async(&runtime).iter(|| async move {
                    let client = reqwest::Client::new();
                    let mut handles = vec![];

                    for _ in 0..concurrency {
                        let client = client.clone();
                        let handle = tokio::spawn(async move {
                            client.get("http://localhost:8080/v1/models").send().await
                        });
                        handles.push(handle);
                    }

                    for handle in handles {
                        let _ = handle.await;
                    }
                });
            },
        );
    }

    group.finish();
}

/// Benchmark different endpoint types.
fn bench_endpoint_types(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let client = reqwest::Client::new();

    let mut group = c.benchmark_group("endpoint_types");

    // Models list
    group.bench_function("models_list", |b| {
        b.to_async(&runtime).iter(|| async {
            let response = client.get("http://localhost:8080/v1/models").send().await;
            black_box(response)
        });
    });

    // Model info
    group.bench_function("model_info", |b| {
        b.to_async(&runtime).iter(|| async {
            let response = client
                .get("http://localhost:8080/v1/models/test-model")
                .send()
                .await;
            black_box(response)
        });
    });

    // Chat completions (POST)
    group.bench_function("chat_completions", |b| {
        b.to_async(&runtime).iter(|| async {
            let request_body = serde_json::json!({
                "model": "test-model",
                "messages": [{"role": "user", "content": "test"}],
                "max_tokens": 10
            });

            let response = client
                .post("http://localhost:8080/v1/chat/completions")
                .json(&request_body)
                .send()
                .await;
            black_box(response)
        });
    });

    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .measurement_time(Duration::from_secs(10))
        .sample_size(100);
    targets =
        bench_direct_request,
        bench_proxied_request,
        bench_proxy_overhead,
        bench_concurrent_requests,
        bench_endpoint_types
}

criterion_main!(benches);
