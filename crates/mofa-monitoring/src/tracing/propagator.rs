//! Trace Context 传播器
//! Trace Context Propagator
//!
//! 实现 W3C Trace Context 和 B3 传播格式
//! Implements W3C Trace Context and B3 propagation formats

use super::context::{SpanContext, SpanId, TraceFlags, TraceId, TraceState};
use std::collections::HashMap;

/// Header 载体 - 用于传播追踪上下文
/// Header Carrier - Used for propagating trace context
pub trait HeaderCarrier {
    /// 获取 header 值
    /// Get header value
    fn get(&self, key: &str) -> Option<&str>;
    /// 设置 header 值
    /// Set header value
    fn set(&mut self, key: &str, value: String);
    /// 获取所有 keys
    /// Get all keys
    fn keys(&self) -> Vec<&str>;
}

/// HashMap 实现 HeaderCarrier
/// HashMap implementation of HeaderCarrier
impl HeaderCarrier for HashMap<String, String> {
    fn get(&self, key: &str) -> Option<&str> {
        self.get(key).map(|s| s.as_str())
    }

    fn set(&mut self, key: &str, value: String) {
        self.insert(key.to_string(), value);
    }

    fn keys(&self) -> Vec<&str> {
        self.keys().map(|s| s.as_str()).collect()
    }
}

/// Trace 传播器 trait
/// Trace Propagator trait
pub trait TracePropagator: Send + Sync {
    /// 从载体中提取 SpanContext
    /// Extract SpanContext from the carrier
    fn extract(&self, carrier: &dyn HeaderCarrier) -> Option<SpanContext>;

    /// 将 SpanContext 注入到载体中
    /// Inject SpanContext into the carrier
    fn inject(&self, span_context: &SpanContext, carrier: &mut dyn HeaderCarrier);

    /// 获取传播器使用的 header 名称
    /// Get header names used by the propagator
    fn fields(&self) -> &[&str];
}

/// W3C Trace Context 传播器
/// W3C Trace Context Propagator
///
/// 实现 W3C Trace Context 规范
/// Implements W3C Trace Context specification
/// - traceparent: 包含 trace-id, span-id, trace-flags
/// - traceparent: contains trace-id, span-id, trace-flags
/// - tracestate: 供应商特定的追踪数据
/// - tracestate: vendor-specific tracing data
pub struct W3CTraceContextPropagator;

impl W3CTraceContextPropagator {
    /// traceparent header 名称
    /// traceparent header name
    pub const TRACEPARENT: &'static str = "traceparent";
    /// tracestate header 名称
    /// tracestate header name
    pub const TRACESTATE: &'static str = "tracestate";
    /// 版本号
    /// Version number
    pub const VERSION: &'static str = "00";

    pub fn new() -> Self {
        Self
    }

    /// 解析 traceparent header
    /// Parse traceparent header
    fn parse_traceparent(value: &str) -> Option<(TraceId, SpanId, TraceFlags)> {
        let parts: Vec<&str> = value.split('-').collect();
        if parts.len() != 4 {
            return None;
        }

        // 检查版本
        // Check version
        if parts[0] != Self::VERSION {
            // 允许更高版本，但只解析已知字段
            // Allow higher versions, but only parse known fields
        }

        let trace_id = TraceId::from_hex(parts[1]).ok()?;
        let span_id = SpanId::from_hex(parts[2]).ok()?;
        let flags = u8::from_str_radix(parts[3], 16).ok()?;

        Some((trace_id, span_id, TraceFlags::new(flags)))
    }

    /// 格式化 traceparent header
    /// Format traceparent header
    fn format_traceparent(trace_id: &TraceId, span_id: &SpanId, flags: &TraceFlags) -> String {
        format!(
            "{}-{}-{}-{:02x}",
            Self::VERSION,
            trace_id.to_hex(),
            span_id.to_hex(),
            flags.as_u8()
        )
    }
}

impl Default for W3CTraceContextPropagator {
    fn default() -> Self {
        Self::new()
    }
}

impl TracePropagator for W3CTraceContextPropagator {
    fn extract(&self, carrier: &dyn HeaderCarrier) -> Option<SpanContext> {
        let traceparent = carrier.get(Self::TRACEPARENT)?;
        let (trace_id, span_id, trace_flags) = Self::parse_traceparent(traceparent)?;

        if !trace_id.is_valid() || !span_id.is_valid() {
            return None;
        }

        let trace_state = carrier
            .get(Self::TRACESTATE)
            .map(TraceState::from_header)
            .unwrap_or_default();

        Some(SpanContext::new(trace_id, span_id, trace_flags, true).with_trace_state(trace_state))
    }

    fn inject(&self, span_context: &SpanContext, carrier: &mut dyn HeaderCarrier) {
        if !span_context.is_valid() {
            return;
        }

        let traceparent = Self::format_traceparent(
            &span_context.trace_id,
            &span_context.span_id,
            &span_context.trace_flags,
        );
        carrier.set(Self::TRACEPARENT, traceparent);

        if !span_context.trace_state.is_empty() {
            carrier.set(Self::TRACESTATE, span_context.trace_state.to_header());
        }
    }

    fn fields(&self) -> &[&str] {
        &[Self::TRACEPARENT, Self::TRACESTATE]
    }
}

/// B3 传播器
/// B3 Propagator
///
/// 支持 Zipkin B3 格式（单 header 和多 header）
/// Supports Zipkin B3 format (single and multi header)
pub struct B3Propagator {
    /// 是否使用单 header 格式
    /// Whether to use single header format
    single_header: bool,
}

impl B3Propagator {
    /// B3 单 header 名称
    /// B3 single header name
    pub const B3: &'static str = "b3";
    /// X-B3-TraceId header
    /// X-B3-TraceId header
    pub const X_B3_TRACE_ID: &'static str = "x-b3-traceid";
    /// X-B3-SpanId header
    /// X-B3-SpanId header
    pub const X_B3_SPAN_ID: &'static str = "x-b3-spanid";
    /// X-B3-ParentSpanId header
    /// X-B3-ParentSpanId header
    pub const X_B3_PARENT_SPAN_ID: &'static str = "x-b3-parentspanid";
    /// X-B3-Sampled header
    /// X-B3-Sampled header
    pub const X_B3_SAMPLED: &'static str = "x-b3-sampled";
    /// X-B3-Flags header
    /// X-B3-Flags header
    pub const X_B3_FLAGS: &'static str = "x-b3-flags";

    /// 创建多 header 格式的传播器
    /// Create propagator with multi-header format
    pub fn new() -> Self {
        Self {
            single_header: false,
        }
    }

    /// 创建单 header 格式的传播器
    /// Create propagator with single-header format
    pub fn single_header() -> Self {
        Self {
            single_header: true,
        }
    }

    /// 解析单 header 格式
    /// Parse single header format
    fn parse_single_header(value: &str) -> Option<(TraceId, SpanId, TraceFlags)> {
        // 格式: {TraceId}-{SpanId}-{SamplingState}-{ParentSpanId}
        // Format: {TraceId}-{SpanId}-{SamplingState}-{ParentSpanId}
        // 或: {TraceId}-{SpanId}-{SamplingState}
        // or: {TraceId}-{SpanId}-{SamplingState}
        // 或: {TraceId}-{SpanId}
        // or: {TraceId}-{SpanId}
        // 或: 0 (deny)
        // or: 0 (deny)
        // 或: 1 (accept)
        // or: 1 (accept)
        // 或: d (debug)
        // or: d (debug)

        if value == "0" {
            return Some((TraceId::INVALID, SpanId::INVALID, TraceFlags::NONE));
        }

        let parts: Vec<&str> = value.split('-').collect();
        if parts.len() < 2 {
            return None;
        }

        let trace_id = TraceId::from_hex(parts[0]).ok()?;
        let span_id = SpanId::from_hex(parts[1]).ok()?;

        let flags = if parts.len() > 2 {
            match parts[2] {
                "1" | "d" => TraceFlags::SAMPLED,
                _ => TraceFlags::NONE,
            }
        } else {
            TraceFlags::SAMPLED
        };

        Some((trace_id, span_id, flags))
    }

    /// 格式化单 header
    /// Format single header
    fn format_single_header(trace_id: &TraceId, span_id: &SpanId, flags: &TraceFlags) -> String {
        let sampled = if flags.is_sampled() { "1" } else { "0" };
        format!("{}-{}-{}", trace_id.to_hex(), span_id.to_hex(), sampled)
    }
}

impl Default for B3Propagator {
    fn default() -> Self {
        Self::new()
    }
}

impl TracePropagator for B3Propagator {
    fn extract(&self, carrier: &dyn HeaderCarrier) -> Option<SpanContext> {
        // 首先尝试单 header 格式
        // First try single header format
        if let Some(b3) = carrier.get(Self::B3)
            && let Some((trace_id, span_id, flags)) = Self::parse_single_header(b3)
            && trace_id.is_valid()
            && span_id.is_valid()
        {
            return Some(SpanContext::new(trace_id, span_id, flags, true));
        }

        // 尝试多 header 格式
        // Try multi-header format
        let trace_id = carrier
            .get(Self::X_B3_TRACE_ID)
            .and_then(|v| TraceId::from_hex(v).ok())?;

        let span_id = carrier
            .get(Self::X_B3_SPAN_ID)
            .and_then(|v| SpanId::from_hex(v).ok())?;

        if !trace_id.is_valid() || !span_id.is_valid() {
            return None;
        }

        // 检查 flags 或 sampled
        // Check flags or sampled
        let flags = if carrier.get(Self::X_B3_FLAGS) == Some("1") {
            TraceFlags::SAMPLED
        } else {
            match carrier.get(Self::X_B3_SAMPLED) {
                Some("1") | Some("true") => TraceFlags::SAMPLED,
                Some("0") | Some("false") => TraceFlags::NONE,
                _ => TraceFlags::SAMPLED, // 默认采样
                                          // Default sampled
            }
        };

        Some(SpanContext::new(trace_id, span_id, flags, true))
    }

    fn inject(&self, span_context: &SpanContext, carrier: &mut dyn HeaderCarrier) {
        if !span_context.is_valid() {
            return;
        }

        if self.single_header {
            let b3 = Self::format_single_header(
                &span_context.trace_id,
                &span_context.span_id,
                &span_context.trace_flags,
            );
            carrier.set(Self::B3, b3);
        } else {
            carrier.set(Self::X_B3_TRACE_ID, span_context.trace_id.to_hex());
            carrier.set(Self::X_B3_SPAN_ID, span_context.span_id.to_hex());
            carrier.set(
                Self::X_B3_SAMPLED,
                if span_context.trace_flags.is_sampled() {
                    "1".to_string()
                } else {
                    "0".to_string()
                },
            );
        }
    }

    fn fields(&self) -> &[&str] {
        if self.single_header {
            &[Self::B3]
        } else {
            &[
                Self::X_B3_TRACE_ID,
                Self::X_B3_SPAN_ID,
                Self::X_B3_PARENT_SPAN_ID,
                Self::X_B3_SAMPLED,
                Self::X_B3_FLAGS,
            ]
        }
    }
}

/// 复合传播器 - 支持多种格式
/// Composite Propagator - Supports multiple formats
pub struct CompositePropagator {
    propagators: Vec<Box<dyn TracePropagator>>,
}

impl CompositePropagator {
    pub fn new(propagators: Vec<Box<dyn TracePropagator>>) -> Self {
        Self { propagators }
    }

    /// 创建默认的复合传播器（W3C + B3）
    /// Create default composite propagator (W3C + B3)
    pub fn default_propagators() -> Self {
        Self::new(vec![
            Box::new(W3CTraceContextPropagator::new()),
            Box::new(B3Propagator::new()),
        ])
    }
}

impl TracePropagator for CompositePropagator {
    fn extract(&self, carrier: &dyn HeaderCarrier) -> Option<SpanContext> {
        for propagator in &self.propagators {
            if let Some(context) = propagator.extract(carrier) {
                return Some(context);
            }
        }
        None
    }

    fn inject(&self, span_context: &SpanContext, carrier: &mut dyn HeaderCarrier) {
        for propagator in &self.propagators {
            propagator.inject(span_context, carrier);
        }
    }

    fn fields(&self) -> &[&str] {
        // 返回所有传播器的字段
        // Return fields for all propagators
        // 注意：这里简化处理，返回空切片
        // Note: Simplified here, returning an empty slice
        &[]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_w3c_propagator_inject_extract() {
        let propagator = W3CTraceContextPropagator::new();

        let span_context = SpanContext::new(
            TraceId::from_hex("0af7651916cd43dd8448eb211c80319c").unwrap(),
            SpanId::from_hex("b7ad6b7169203331").unwrap(),
            TraceFlags::SAMPLED,
            false,
        );

        let mut carrier = HashMap::new();
        propagator.inject(&span_context, &mut carrier);

        assert!(carrier.contains_key(W3CTraceContextPropagator::TRACEPARENT));

        let extracted = propagator.extract(&carrier).unwrap();
        assert_eq!(extracted.trace_id, span_context.trace_id);
        assert_eq!(extracted.span_id, span_context.span_id);
        assert!(extracted.is_sampled());
        assert!(extracted.is_remote);
    }

    #[test]
    fn test_w3c_traceparent_format() {
        let traceparent = "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01";

        let mut carrier = HashMap::new();
        carrier.insert(
            W3CTraceContextPropagator::TRACEPARENT.to_string(),
            traceparent.to_string(),
        );

        let propagator = W3CTraceContextPropagator::new();
        let context = propagator.extract(&carrier).unwrap();

        assert_eq!(
            context.trace_id.to_hex(),
            "0af7651916cd43dd8448eb211c80319c"
        );
        assert_eq!(context.span_id.to_hex(), "b7ad6b7169203331");
        assert!(context.is_sampled());
    }

    #[test]
    fn test_b3_single_header() {
        let propagator = B3Propagator::single_header();

        let span_context = SpanContext::new(
            TraceId::from_hex("463ac35c9f6413ad48485a3953bb6124").unwrap(),
            SpanId::from_hex("0020000000000001").unwrap(),
            TraceFlags::SAMPLED,
            false,
        );

        let mut carrier = HashMap::new();
        propagator.inject(&span_context, &mut carrier);

        assert!(carrier.contains_key(B3Propagator::B3));

        let extracted = propagator.extract(&carrier).unwrap();
        assert_eq!(extracted.trace_id, span_context.trace_id);
        assert_eq!(extracted.span_id, span_context.span_id);
    }

    #[test]
    fn test_b3_multi_header() {
        let propagator = B3Propagator::new();

        let mut carrier = HashMap::new();
        carrier.insert(
            B3Propagator::X_B3_TRACE_ID.to_string(),
            "463ac35c9f6413ad48485a3953bb6124".to_string(),
        );
        carrier.insert(
            B3Propagator::X_B3_SPAN_ID.to_string(),
            "0020000000000001".to_string(),
        );
        carrier.insert(B3Propagator::X_B3_SAMPLED.to_string(), "1".to_string());

        let context = propagator.extract(&carrier).unwrap();
        assert!(context.is_valid());
        assert!(context.is_sampled());
    }

    #[test]
    fn test_composite_propagator() {
        let propagator = CompositePropagator::default_propagators();

        // W3C 格式
        // W3C format
        let mut carrier = HashMap::new();
        carrier.insert(
            W3CTraceContextPropagator::TRACEPARENT.to_string(),
            "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01".to_string(),
        );

        let context = propagator.extract(&carrier).unwrap();
        assert!(context.is_valid());

        // B3 格式
        // B3 format
        let mut carrier = HashMap::new();
        carrier.insert(
            B3Propagator::B3.to_string(),
            "463ac35c9f6413ad48485a3953bb6124-0020000000000001-1".to_string(),
        );

        let context = propagator.extract(&carrier).unwrap();
        assert!(context.is_valid());
    }
}
