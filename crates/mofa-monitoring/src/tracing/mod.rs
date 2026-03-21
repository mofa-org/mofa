//! Distributed tracing module
//!
//! Provides distributed tracing functionality, supporting:
//! - Trace Context propagation
//! - Span management
//! - Multiple exporters (Console, Jaeger, OTLP)
//! - Automatic tracing for Agents and Workflows

mod context;
mod exporter;
mod instrumentation;
#[cfg(feature = "otlp-metrics")]
mod metrics_exporter;
mod propagator;
mod span;
mod tracer;

pub use context::{SpanContext, SpanId, TraceContext, TraceFlags, TraceId, TraceState};
pub use exporter::{
    ConsoleExporter, ExporterConfig, JaegerExporter, OtlpExporter, TracingExporter,
};
pub use instrumentation::{
    AgentTracer, MessageTracer, TracedAgent, TracedWorkflow, WorkflowTracer, trace_agent_operation,
    trace_workflow_execution,
};
#[cfg(feature = "otlp-metrics")]
pub use metrics_exporter::{
    CardinalityLimits, OtlpExporterHandles, OtlpMetricsExporter, OtlpMetricsExporterConfig,
    OtlpMetricsExporterError,
};
pub use propagator::{B3Propagator, HeaderCarrier, TracePropagator, W3CTraceContextPropagator};
pub use span::{Span, SpanAttribute, SpanBuilder, SpanEvent, SpanKind, SpanLink, SpanStatus};
pub use tracer::{
    BatchSpanProcessor, GlobalTracer, SamplingStrategy, SimpleSpanProcessor, SpanProcessor, Tracer,
    TracerConfig, TracerProvider,
};
