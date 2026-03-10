use opentelemetry::{
    trace::{Tracer, Span, SpanBuilder, Status, TraceContextExt},
    Context, Key, KeyValue, Value,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct TracingConfig {
    pub service_name: String,
    pub exporter: TracingExporter,
    pub sampling_rate: f64,
    pub max_attributes: usize,
    pub max_events: usize,
    pub max_links: usize,
}

#[derive(Clone)]
pub enum TracingExporter {
    Stdout,
    Jaeger { endpoint: String },
    Zipkin { endpoint: String },
    Otlp { endpoint: String },
    None,
}

impl Default for TracingConfig {
    fn default() -> Self {
        Self {
            service_name: \"mofa\".to_string(),
            exporter: TracingExporter::Stdout,
            sampling_rate: 1.0,
            max_attributes: 100,
            max_events: 1000,
            max_links: 100,
        }
    }
}

pub trait TracerProvider: Send + Sync {
    fn tracer(&self, name: &str) -> Arc<dyn MofaTracer>;
}

pub trait MofaTracer: Send + Sync {
    fn start_span(&self, name: &str) -> Arc<dyn MofaSpan>;
    
    fn span_builder(&self, name: &str) -> SpanBuilder;
    
    fn with_span(&self, span: Arc<dyn MofaSpan>, f: impl FnOnce());
    
    fn current_context(&self) -> Option<TraceContext>;
}

pub trait MofaSpan: Send + Sync {
    fn add_event(&self, name: &str, attributes: Vec<KeyValue>);
    
    fn set_attribute(&self, key: Key, value: Value);
    
    fn set_status(&self, status: Status);
    
    fn end(&self);
    
    fn context(&self) -> &TraceContext;
}

#[derive(Clone)]
pub struct TraceContext {
    pub trace_id: String,
    pub span_id: String,
    pub trace_flags: u8,
}

pub struct OpenTelemetryTracer {
    config: TracingConfig,
    tracer: opentelemetry::trace::Tracer,
}

impl OpenTelemetryTracer {
    pub fn new(config: TracingConfig) -> Self {
        let provider = opentelemetry::sdk::trace::Provider::builder()
            .with_simple_span_exporter(opentelemetry::sdk::trace::SimpleSpanProcessor::new(
                std::io::stdout(),
                opentelemetry::sdk::trace::SpanExporter::new_noop(),
            ))
            .build();

        let tracer = provider.tracer(&config.service_name);

        Self { config, tracer }
    }
}

impl MofaTracer for OpenTelemetryTracer {
    fn start_span(&self, name: &str) -> Arc<dyn MofaSpan> {
        let span = self.tracer.start(name);
        Arc::new(OpenTelemetrySpan { span })
    }

    fn span_builder(&self, name: &str) -> SpanBuilder {
        self.tracer.span_builder(name)
    }

    fn with_span(&self, _span: Arc<dyn MofaSpan>, _f: impl FnOnce()) {
    }

    fn current_context(&self) -> Option<TraceContext> {
        let context = opentelemetry::Context::current();
        let span = context.span();
        if span.span_context().is_valid() {
            Some(TraceContext {
                trace_id: format!(\"{:x}\", span.span_context().trace_id()),
                span_id: format!(\"{:x}\", span.span_context().span_id()),
                trace_flags: span.span_context().trace_flags(),
            })
        } else {
            None
        }
    }
}

struct OpenTelemetrySpan {
    span: opentelemetry::trace::Span,
}

impl MofaSpan for OpenTelemetrySpan {
    fn add_event(&self, name: &str, attributes: Vec<KeyValue>) {
        self.span.add_event(name, attributes);
    }

    fn set_attribute(&self, key: Key, value: Value) {
        self.span.set_attribute(key, value);
    }

    fn set_status(&self, status: Status) {
        self.span.set_status(status);
    }

    fn end(&self) {
        self.span.end();
    }

    fn context(&self) -> &TraceContext {
        unimplemented!()
    }
}

pub struct AgentTracer {
    tracer: Arc<dyn MofaTracer>,
    agent_id: String,
}

impl AgentTracer {
    pub fn new(tracer: Arc<dyn MofaTracer>, agent_id: String) -> Self {
        Self { tracer, agent_id }
    }

    pub fn trace_agent_loop(&self) -> AgentLoopSpan {
        let span = self.tracer.start_span(\"agent_loop\");
        span.set_attribute(
            Key::new(\"agent.id\"),
            Value::String(self.agent_id.clone().into()),
        );
        AgentLoopSpan {
            tracer: self.tracer.clone(),
            span,
        }
    }

    pub fn trace_tool_call(&self, tool_name: &str) -> ToolCallSpan {
        let span = self.tracer.start_span(\"tool_call\");
        span.set_attribute(
            Key::new(\"tool.name\"),
            Value::String(tool_name.into()),
        );
        ToolCallSpan {
            span,
        }
    }

    pub fn trace_llm_call(&self, model: &str) -> LlmCallSpan {
        let span = self.tracer.start_span(\"llm_call\");
        span.set_attribute(
            Key::new(\"llm.model\"),
            Value::String(model.into()),
        );
        LlmCallSpan {
            span,
        }
    }

    pub fn trace_workflow_execution(&self, workflow_id: &str) -> WorkflowSpan {
        let span = self.tracer.start_span(\"workflow_execution\");
        span.set_attribute(
            Key::new(\"workflow.id\"),
            Value::String(workflow_id.into()),
        );
        WorkflowSpan {
            span,
        }
    }
}

pub struct AgentLoopSpan {
    tracer: Arc<dyn MofaTracer>,
    span: Arc<dyn MofaSpan>,
}

impl AgentLoopSpan {
    pub fn set_thought(&self, thought: &str) {
        self.span.add_event(\"agent.thought\", vec![
            KeyValue::new(\"thought\", thought),
        ]);
    }

    pub fn set_action(&self, action: &str) {
        self.span.add_event(\"agent.action\", vec![
            KeyValue::new(\"action\", action),
        ]);
    }

    pub fn record_token_usage(&self, prompt_tokens: u32, completion_tokens: u32) {
        self.span.set_attribute(
            Key::new(\"llm.prompt_tokens\"),
            Value::U64(prompt_tokens as u64),
        );
        self.span.set_attribute(
            Key::new(\"llm.completion_tokens\"),
            Value::U64(completion_tokens as u64),
        );
    }
}

impl Drop for AgentLoopSpan {
    fn drop(&mut self) {
        self.span.end();
    }
}

pub struct ToolCallSpan {
    span: Arc<dyn MofaSpan>,
}

impl ToolCallSpan {
    pub fn set_arguments(&self, args: &str) {
        self.span.set_attribute(
            Key::new(\"tool.arguments\"),
            Value::String(args.into()),
        );
    }

    pub fn set_result(&self, result: &str, success: bool) {
        self.span.set_attribute(
            Key::new(\"tool.result\"),
            Value::String(result.into()),
        );
        self.span.set_attribute(
            Key::new(\"tool.success\"),
            Value::Bool(success),
        );
    }
}

impl Drop for ToolCallSpan {
    fn drop(&mut self) {
        self.span.end();
    }
}

pub struct LlmCallSpan {
    span: Arc<dyn MofaSpan>,
}

impl LlmCallSpan {
    pub fn set_latency(&self, latency_ms: u64) {
        self.span.set_attribute(
            Key::new(\"llm.latency_ms\"),
            Value::U64(latency_ms),
        );
    }

    pub fn set_token_usage(&self, prompt_tokens: u32, completion_tokens: u32) {
        self.span.set_attribute(
            Key::new(\"llm.prompt_tokens\"),
            Value::U64(prompt_tokens as u64),
        );
        self.span.set_attribute(
            Key::new(\"llm.completion_tokens\"),
            Value::U64(completion_tokens as u64),
        );
    }
}

impl Drop for LlmCallSpan {
    fn drop(&mut self) {
        self.span.end();
    }
}

pub struct WorkflowSpan {
    span: Arc<dyn MofaSpan>,
}

impl WorkflowSpan {
    pub fn set_node(&self, node_id: &str) {
        self.span.set_attribute(
            Key::new(\"workflow.node_id\"),
            Value::String(node_id.into()),
        );
    }

    pub fn record_node_duration(&self, node_id: &str, duration_ms: u64) {
        self.span.add_event(\"workflow.node_complete\", vec![
            KeyValue::new(\"node_id\", node_id),
            KeyValue::new(\"duration_ms\", duration_ms as i64),
        ]);
    }
}

impl Drop for WorkflowSpan {
    fn drop(&mut self) {
        self.span.end();
    }
}

pub struct MetricsCollector {
    counters: Arc<RwLock<std::collections::HashMap<String, u64>>>,
    gauges: Arc<RwLock<std::collections::HashMap<String, f64>>>,
    histograms: Arc<RwLock<std::collections::HashMap<String, Vec<f64>>>>,
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            counters: Arc::new(RwLock::new(std::collections::HashMap::new())),
            gauges: Arc::new(RwLock::new(std::collections::HashMap::new())),
            histograms: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    pub async fn increment_counter(&self, name: &str, value: u64) {
        let mut counters = self.counters.write().await;
        *counters.entry(name.to_string()).or_insert(0) += value;
    }

    pub async fn set_gauge(&self, name: &str, value: f64) {
        let mut gauges = self.gauges.write().await;
        gauges.insert(name.to_string(), value);
    }

    pub async fn record_histogram(&self, name: &str, value: f64) {
        let mut histograms = self.histograms.write().await;
        histograms.entry(name.to_string()).or_insert_with(Vec::new).push(value);
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}
