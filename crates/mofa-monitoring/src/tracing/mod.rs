//! 分布式追踪模块
//! Distributed tracing module
//!
//! 提供分布式追踪功能，支持:
//! Provides distributed tracing functionality, supporting:
//! - Trace Context 传播
//! - Trace Context propagation
//! - Span 管理
//! - Span management
//! - 多种导出器 (Console, Jaeger, OTLP)
//! - Multiple exporters (Console, Jaeger, OTLP)
//! - Agent 和 Workflow 的自动追踪
//! - Automatic tracing for Agents and Workflows

mod context;
mod exporter;
mod instrumentation;
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
pub use propagator::{B3Propagator, HeaderCarrier, TracePropagator, W3CTraceContextPropagator};
pub use span::{Span, SpanAttribute, SpanBuilder, SpanEvent, SpanKind, SpanLink, SpanStatus};
pub use tracer::{
    BatchSpanProcessor, GlobalTracer, SamplingStrategy, SimpleSpanProcessor, SpanProcessor, Tracer,
    TracerConfig, TracerProvider,
};
