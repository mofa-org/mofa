//! # Production Observability Example
//!
//! This example demonstrates the two observability pillars added to MoFA:
//!
//! ## Part 1 — Prometheus Metrics
//!
//! Shows the **new** LLM counters introduced in Task 26:
//! - `mofa_llm_input_tokens_total`  — cumulative prompt tokens per model
//! - `mofa_llm_output_tokens_total` — cumulative completion tokens per model
//! - `mofa_llm_time_to_first_token_seconds` — TTFT gauge for streaming calls
//!
//! The example populates these counters with realistic simulated values and
//! then scrapes the `/metrics` endpoint, printing the relevant lines to
//! stdout so you can verify the output without needing a running Prometheus
//! server.
//!
//! ## Part 2 — OpenTelemetry Distributed Tracing
//!
//! Shows the `gen_ai.*` and `llm.*` OTel spans produced by `mofa-foundation`
//! when the `otel-tracing` feature is enabled.  A **console exporter** is
//! used so that you can see the spans printed to stdout without running
//! Jaeger or any other backend.
//!
//! Run with:
//!
//! ```bash
//! # From the workspace root
//! cd examples/production_observability
//! cargo run
//! ```
//!
//! To forward spans to Jaeger instead:
//!
//! ```bash
//! # Start Jaeger all-in-one
//! docker run -d --name jaeger \
//!   -p 16686:16686 \
//!   -p 4317:4317 \
//!   jaegertracing/all-in-one:latest
//!
//! # Then configure OTLP exporter before running:
//! export OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317
//! cargo run
//!
//! # Open http://localhost:16686 and select service "mofa-foundation"
//! ```

use std::sync::Arc;
use std::time::Duration;

use mofa_sdk::dashboard::{
    LLMMetrics, MetricsCollector, MetricsConfig, PrometheusExportConfig, PrometheusExporter,
};
use opentelemetry::global;
use opentelemetry_sdk::trace::TracerProvider;
use tracing::info;

// ---------------------------------------------------------------------------
// Entrypoint
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Structured logging — shows INFO and above from mofa crates.
    tracing_subscriber::fmt()
        .with_env_filter("info,mofa=debug")
        .init();

    print_banner("MoFA Production Observability Demo");

    // -----------------------------------------------------------------------
    // Part 1 — Prometheus metrics
    // -----------------------------------------------------------------------
    demo_prometheus().await?;

    println!();

    // -----------------------------------------------------------------------
    // Part 2 — OpenTelemetry distributed tracing (console exporter)
    // -----------------------------------------------------------------------
    demo_otel_tracing().await?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Part 1: Prometheus
// ---------------------------------------------------------------------------

async fn demo_prometheus() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("\n════════════════════════════════════════════════════════════");
    println!("  PART 1 — Prometheus Metrics");
    println!("════════════════════════════════════════════════════════════\n");

    // Build a metrics collector and populate it with simulated LLM data.
    let collector = Arc::new(MetricsCollector::new(MetricsConfig::default()));

    // Simulate three rounds of LLM calls, growing the counters each time.
    let scenarios: &[(&str, &str, &str, u64, u64, Option<f64>)] = &[
        // (plugin_id, provider, model, prompt_tok, completion_tok, ttft_ms)
        ("openai-gpt4o", "openai", "gpt-4o", 1_200, 380, Some(310.0)),
        ("openai-gpt4o", "openai", "gpt-4o", 2_450, 720, Some(295.0)),
        ("openai-gpt4o", "openai", "gpt-4o", 3_800, 1_056, Some(320.0)),
        ("anthropic-claude3", "anthropic", "claude-3-opus", 900, 240, None),
        ("anthropic-claude3", "anthropic", "claude-3-opus", 1_700, 490, None),
    ];

    for (plugin_id, provider, model, prompt_tok, completion_tok, ttft_ms) in scenarios {
        let metrics = LLMMetrics {
            plugin_id: plugin_id.to_string(),
            provider_name: provider.to_string(),
            model_name: model.to_string(),
            state: "running".to_string(),
            total_requests: *prompt_tok / 400 + 1,
            successful_requests: *prompt_tok / 400,
            failed_requests: 0,
            total_tokens: prompt_tok + completion_tok,
            prompt_tokens: *prompt_tok,
            completion_tokens: *completion_tok,
            avg_latency_ms: 1_240.0,
            tokens_per_second: Some(85.0),
            time_to_first_token_ms: *ttft_ms,
            requests_per_minute: 12.0,
            error_rate: 0.0,
            last_request_timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        };
        collector.update_llm(metrics).await;
    }

    info!("✅  LLM metrics populated with simulated data");

    // Build the Prometheus exporter and snapshot the output.
    let exporter = Arc::new(PrometheusExporter::new(
        collector.clone(),
        PrometheusExportConfig::default()
            .with_refresh_interval(Duration::from_millis(100)),
    ));

    // Refresh once so the cached body is populated.
    exporter.refresh_once().await?;

    let payload = exporter.render_cached().await;
    let text = String::from_utf8_lossy(&payload);

    // -----------------------------------------------------------------------
    // Print only the LLM-related metric families for readability.
    // -----------------------------------------------------------------------
    println!("── Scraping /metrics (LLM families only) ──────────────────\n");

    let mut in_target_family = false;
    for line in text.lines() {
        // Start a new metric family on HELP lines.
        if line.starts_with("# HELP") {
            in_target_family = line.contains("mofa_llm");
        }
        if in_target_family {
            println!("{line}");
        }
    }

    // -----------------------------------------------------------------------
    // Assertion: verify the three new counter/gauge families are present.
    // -----------------------------------------------------------------------
    let missing: Vec<&str> = [
        "mofa_llm_input_tokens_total",
        "mofa_llm_output_tokens_total",
        "mofa_llm_time_to_first_token_seconds",
    ]
    .iter()
    .copied()
    .filter(|name| !text.contains(name))
    .collect();

    if missing.is_empty() {
        println!(
            "\n✅  All three new metric families are present in /metrics:\n   \
             mofa_llm_input_tokens_total, \
             mofa_llm_output_tokens_total, \
             mofa_llm_time_to_first_token_seconds"
        );
    } else {
        eprintln!("\n❌  Missing metric families: {missing:?}");
        std::process::exit(1);
    }

    // -----------------------------------------------------------------------
    // Prometheus scrape config reminder
    // -----------------------------------------------------------------------
    println!("\n── How to wire this into Prometheus ────────────────────────");
    println!(
        "\n  Start the monitoring dashboard:\n    cargo run -p mofa-monitoring\n\n  \
         Add to prometheus.yml:\n    scrape_configs:\n      - job_name: mofa\n        \
         static_configs:\n          - targets: [\"localhost:9090\"]\n        \
         scrape_interval: 15s\n\n  Useful PromQL queries:\n\n    \
         # Token throughput (tokens/min):\n    \
         rate(mofa_llm_input_tokens_total[1m]) + rate(mofa_llm_output_tokens_total[1m])\n\n    \
         # I/O token ratio (response verbosity):\n    \
         rate(mofa_llm_output_tokens_total[5m]) / rate(mofa_llm_input_tokens_total[5m])\n\n    \
         # P95 latency:\n    \
         histogram_quantile(0.95, rate(mofa_llm_request_duration_seconds_bucket[5m]))\n\n    \
         # Streaming TTFT by provider:\n    \
         mofa_llm_time_to_first_token_seconds"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// Part 2: OpenTelemetry tracing
// ---------------------------------------------------------------------------

async fn demo_otel_tracing() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("════════════════════════════════════════════════════════════");
    println!("  PART 2 — OpenTelemetry Distributed Tracing");
    println!("════════════════════════════════════════════════════════════\n");

    println!("Initialising OpenTelemetry console exporter …");
    println!(
        "(In production swap this for OTLP/Jaeger — see the Jaeger section below.)\n"
    );

    // Install a console-based tracer provider.  Every span produced by
    // mofa-foundation (feature = "otel-tracing") will be printed to stdout.
    let exporter = opentelemetry_stdout::SpanExporter::default();
    let provider = TracerProvider::builder()
        .with_simple_exporter(exporter)
        .build();
    global::set_tracer_provider(provider);

    // -----------------------------------------------------------------------
    // Simulate the spans that mofa-foundation emits.
    //
    // When you have a real LLM provider configured (OPENAI_API_KEY etc.) you
    // can replace this block with an actual LLMAgentBuilder call and the spans
    // will appear automatically.  For this self-contained demo we emit the
    // same spans manually so the example compiles without any API key.
    // -----------------------------------------------------------------------
    println!(
        "─────────────────────────────────────────────────────────\n\
         Simulated span output from mofa-foundation (otel-tracing feature)\n\
         ─────────────────────────────────────────────────────────\n"
    );

    emit_demo_spans();

    println!(
        "\n─────────────────────────────────────────────────────────\n\
         Expected span pairs on every chat_with_session() call:\n\n  \
         1. llm.agent.chat  (Internal)\n     \
            attributes: agent.id, session.id\n\n  \
         2. gen_ai.chat_completion  (Client)  — child of above\n     \
            attributes: gen_ai.system, gen_ai.request.model,\n     \
                        gen_ai.usage.input_tokens, gen_ai.usage.output_tokens,\n     \
                        gen_ai.response.model, session.id\n"
    );

    // -----------------------------------------------------------------------
    // Jaeger setup reference
    // -----------------------------------------------------------------------
    println!("── Jaeger / OTLP setup ─────────────────────────────────────\n");
    println!(
        "  # 1. Start Jaeger all-in-one\n  \
         docker run -d --name jaeger \\\n    \
           -p 16686:16686 \\\n    \
           -p 4317:4317 \\\n    \
           jaegertracing/all-in-one:latest\n\n  \
         # 2. Configure the OTLP exporter (no code changes needed):\n  \
         export OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317\n  \
         export OTEL_SERVICE_NAME=my-mofa-app\n\n  \
         # 3. In your Rust application:\n  \
         use opentelemetry_otlp::WithExportConfig;\n  \
         use opentelemetry_sdk::trace::TracerProvider;\n  \
         use opentelemetry::global;\n\n  \
         fn init_tracing() -> anyhow::Result<()> {{\n      \
             let exporter = opentelemetry_otlp::SpanExporter::builder()\n          \
                 .with_tonic()\n          \
                 .with_endpoint(\"http://localhost:4317\")\n          \
                 .build()?;\n      \
             let provider = TracerProvider::builder()\n          \
                 .with_batch_exporter(exporter, opentelemetry_sdk::runtime::Tokio)\n          \
                 .build();\n      \
             global::set_tracer_provider(provider);\n      \
             Ok(())\n  \
         }}\n\n  \
         // Build any LLMAgent with otel-tracing feature enabled — all calls\n  \
         // to chat_with_session() automatically emit the two spans above.\n  \
         let agent = LLMAgentBuilder::from_env()?\n      \
             .with_id(\"my-agent\")\n      \
             .with_system_prompt(\"You are a helpful assistant.\")\n      \
             .build();\n\n  \
         // Open http://localhost:16686 → select service 'mofa-foundation'\n"
    );

    // Flush all pending spans before exit.
    global::shutdown_tracer_provider();

    println!("✅  OTel spans flushed — demo complete.");

    Ok(())
}

// ---------------------------------------------------------------------------
// Emit representative OTel spans that mirror what mofa-foundation produces
// ---------------------------------------------------------------------------

fn emit_demo_spans() {
    use opentelemetry::trace::{Span, SpanKind, Tracer};
    use opentelemetry::KeyValue;

    let tracer = global::tracer("mofa-foundation");

    // Outer agent span (Internal kind) — wraps the full chat_with_session call.
    let mut agent_span = tracer
        .span_builder("llm.agent.chat")
        .with_kind(SpanKind::Internal)
        .start(&tracer);
    agent_span.set_attribute(KeyValue::new("agent.id", "demo-agent"));
    agent_span.set_attribute(KeyValue::new(
        "session.id",
        "01936b2f-1234-7abc-8def-000000000001",
    ));

    // Inner provider span (Client kind) — represents the HTTP call to the LLM API.
    let mut provider_span = tracer
        .span_builder("gen_ai.chat_completion")
        .with_kind(SpanKind::Client)
        .start(&tracer);
    provider_span.set_attribute(KeyValue::new("gen_ai.system", "openai"));
    provider_span.set_attribute(KeyValue::new("gen_ai.request.model", "gpt-4o"));
    provider_span.set_attribute(KeyValue::new(
        "session.id",
        "01936b2f-1234-7abc-8def-000000000001",
    ));
    // These are set once the response arrives.
    provider_span.set_attribute(KeyValue::new("gen_ai.response.model", "gpt-4o"));
    provider_span.set_attribute(KeyValue::new("gen_ai.usage.input_tokens", 312_i64));
    provider_span.set_attribute(KeyValue::new("gen_ai.usage.output_tokens", 78_i64));
    provider_span.set_status(opentelemetry::trace::Status::Ok);
    provider_span.end();

    agent_span.set_status(opentelemetry::trace::Status::Ok);
    agent_span.end();

    // Second turn — streaming variant.
    let mut stream_span = tracer
        .span_builder("llm.agent.chat_stream")
        .with_kind(SpanKind::Internal)
        .start(&tracer);
    stream_span.set_attribute(KeyValue::new("agent.id", "demo-agent"));
    stream_span.set_attribute(KeyValue::new(
        "session.id",
        "01936b2f-1234-7abc-8def-000000000001",
    ));
    stream_span.set_attribute(KeyValue::new("llm.streaming", true));
    stream_span.set_status(opentelemetry::trace::Status::Ok);
    stream_span.end();
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn print_banner(title: &str) {
    let width = 62;
    let padding = (width - title.len()).saturating_sub(2) / 2;
    let left = " ".repeat(padding);
    let right = " ".repeat(width - title.len() - 2 - padding);
    println!("╔{}╗", "═".repeat(width));
    println!("║{left}{title}{right}║");
    println!("╚{}╝", "═".repeat(width));
}
