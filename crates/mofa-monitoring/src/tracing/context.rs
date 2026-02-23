//! Trace Context 定义
//! Trace Context Definition
//!
//! 实现 W3C Trace Context 标准
//! Implementation of the W3C Trace Context standard

use rand::Rng;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Trace ID - 16字节 (128位)
/// Trace ID - 16 bytes (128 bits)
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TraceId([u8; 16]);

impl TraceId {
    /// 创建新的随机 Trace ID
    /// Create a new random Trace ID
    pub fn new() -> Self {
        let mut rng = rand::thread_rng();
        let mut bytes = [0u8; 16];
        rng.fill(&mut bytes);
        Self(bytes)
    }

    /// 从字节数组创建
    /// Create from a byte array
    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    /// 从十六进制字符串创建
    /// Create from a hexadecimal string
    pub fn from_hex(hex: &str) -> Result<Self, String> {
        if hex.len() != 32 {
            return Err("TraceId hex string must be 32 characters".to_string());
        }
        let bytes = hex::decode(hex).map_err(|e| e.to_string())?;
        let mut arr = [0u8; 16];
        arr.copy_from_slice(&bytes);
        Ok(Self(arr))
    }

    /// 转换为十六进制字符串
    /// Convert to a hexadecimal string
    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }

    /// 获取字节
    /// Get the bytes
    pub fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }

    /// 是否有效（非全零）
    /// Whether it is valid (not all zeros)
    pub fn is_valid(&self) -> bool {
        self.0.iter().any(|&b| b != 0)
    }

    /// 无效的 Trace ID
    /// Invalid Trace ID
    pub const INVALID: TraceId = TraceId([0u8; 16]);
}

impl Default for TraceId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for TraceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TraceId({})", self.to_hex())
    }
}

impl fmt::Display for TraceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

impl FromStr for TraceId {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_hex(s)
    }
}

/// Span ID - 8字节 (64位)
/// Span ID - 8 bytes (64 bits)
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SpanId([u8; 8]);

impl SpanId {
    /// 创建新的随机 Span ID
    /// Create a new random Span ID
    pub fn new() -> Self {
        let mut rng = rand::thread_rng();
        let mut bytes = [0u8; 8];
        rng.fill(&mut bytes);
        Self(bytes)
    }

    /// 从字节数组创建
    /// Create from a byte array
    pub fn from_bytes(bytes: [u8; 8]) -> Self {
        Self(bytes)
    }

    /// 从十六进制字符串创建
    /// Create from a hexadecimal string
    pub fn from_hex(hex: &str) -> Result<Self, String> {
        if hex.len() != 16 {
            return Err("SpanId hex string must be 16 characters".to_string());
        }
        let bytes = hex::decode(hex).map_err(|e| e.to_string())?;
        let mut arr = [0u8; 8];
        arr.copy_from_slice(&bytes);
        Ok(Self(arr))
    }

    /// 转换为十六进制字符串
    /// Convert to a hexadecimal string
    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }

    /// 获取字节
    /// Get the bytes
    pub fn as_bytes(&self) -> &[u8; 8] {
        &self.0
    }

    /// 是否有效（非全零）
    /// Whether it is valid (not all zeros)
    pub fn is_valid(&self) -> bool {
        self.0.iter().any(|&b| b != 0)
    }

    /// 无效的 Span ID
    /// Invalid Span ID
    pub const INVALID: SpanId = SpanId([0u8; 8]);
}

impl Default for SpanId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for SpanId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SpanId({})", self.to_hex())
    }
}

impl fmt::Display for SpanId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

impl FromStr for SpanId {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_hex(s)
    }
}

/// Trace Flags - 采样标志
/// Trace Flags - Sampling flags
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceFlags(u8);

impl TraceFlags {
    /// 已采样标志
    /// Sampled flag
    pub const SAMPLED: TraceFlags = TraceFlags(0x01);
    /// 无标志
    /// No flags
    pub const NONE: TraceFlags = TraceFlags(0x00);

    /// 创建新的 TraceFlags
    /// Create new TraceFlags
    pub fn new(flags: u8) -> Self {
        Self(flags)
    }

    /// 是否已采样
    /// Whether it is sampled
    pub fn is_sampled(&self) -> bool {
        self.0 & 0x01 != 0
    }

    /// 设置采样
    /// Set sampling
    pub fn with_sampled(mut self, sampled: bool) -> Self {
        if sampled {
            self.0 |= 0x01;
        } else {
            self.0 &= !0x01;
        }
        self
    }

    /// 获取原始值
    /// Get the raw value
    pub fn as_u8(&self) -> u8 {
        self.0
    }
}

impl Default for TraceFlags {
    fn default() -> Self {
        Self::SAMPLED
    }
}

impl fmt::Display for TraceFlags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:02x}", self.0)
    }
}

/// Trace State - 供应商特定的追踪数据
/// Trace State - Vendor-specific tracing data
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TraceState {
    entries: Vec<(String, String)>,
}

impl TraceState {
    /// 创建空的 TraceState
    /// Create an empty TraceState
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// 从键值对创建
    /// Create from key-value pairs
    pub fn from_entries(entries: Vec<(String, String)>) -> Self {
        Self { entries }
    }

    /// 添加条目
    /// Add an entry
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let key = key.into();
        // 移除已存在的同名条目
        // Remove existing entry with the same name
        self.entries.retain(|(k, _)| k != &key);
        self.entries.push((key, value.into()));
    }

    /// 获取条目
    /// Get an entry
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// 移除条目
    /// Remove an entry
    pub fn remove(&mut self, key: &str) {
        self.entries.retain(|(k, _)| k != key);
    }

    /// 获取所有条目
    /// Get all entries
    pub fn entries(&self) -> &[(String, String)] {
        &self.entries
    }

    /// 是否为空
    /// Whether it is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// 转换为 header 格式字符串
    /// Convert to header format string
    pub fn to_header(&self) -> String {
        self.entries
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join(",")
    }

    /// 从 header 格式字符串解析
    /// Parse from header format string
    pub fn from_header(header: &str) -> Self {
        let entries = header
            .split(',')
            .filter_map(|part| {
                let mut iter = part.splitn(2, '=');
                let key = iter.next()?.trim().to_string();
                let value = iter.next()?.trim().to_string();
                if key.is_empty() {
                    None
                } else {
                    Some((key, value))
                }
            })
            .collect();
        Self { entries }
    }
}

/// Span Context - Span 的不可变上下文信息
/// Span Context - Immutable context information of a Span
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanContext {
    /// Trace ID
    /// Trace ID
    pub trace_id: TraceId,
    /// Span ID
    /// Span ID
    pub span_id: SpanId,
    /// Trace Flags
    /// Trace Flags
    pub trace_flags: TraceFlags,
    /// Trace State
    /// Trace State
    pub trace_state: TraceState,
    /// 是否远程（从其他服务传播过来）
    /// Whether it is remote (propagated from another service)
    pub is_remote: bool,
}

impl SpanContext {
    /// 创建新的 SpanContext
    /// Create a new SpanContext
    pub fn new(
        trace_id: TraceId,
        span_id: SpanId,
        trace_flags: TraceFlags,
        is_remote: bool,
    ) -> Self {
        Self {
            trace_id,
            span_id,
            trace_flags,
            trace_state: TraceState::new(),
            is_remote,
        }
    }

    /// 创建无效的 SpanContext
    /// Create an invalid SpanContext
    pub fn invalid() -> Self {
        Self {
            trace_id: TraceId::INVALID,
            span_id: SpanId::INVALID,
            trace_flags: TraceFlags::NONE,
            trace_state: TraceState::new(),
            is_remote: false,
        }
    }

    /// 是否有效
    /// Whether it is valid
    pub fn is_valid(&self) -> bool {
        self.trace_id.is_valid() && self.span_id.is_valid()
    }

    /// 是否已采样
    /// Whether it is sampled
    pub fn is_sampled(&self) -> bool {
        self.trace_flags.is_sampled()
    }

    /// 设置 TraceState
    /// Set TraceState
    pub fn with_trace_state(mut self, trace_state: TraceState) -> Self {
        self.trace_state = trace_state;
        self
    }
}

impl Default for SpanContext {
    fn default() -> Self {
        Self::invalid()
    }
}

/// Trace Context - 完整的追踪上下文
/// Trace Context - Complete tracing context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceContext {
    /// 当前 Span 上下文
    /// Current Span Context
    pub span_context: SpanContext,
    /// 父 Span 上下文
    /// Parent Span Context
    pub parent_span_context: Option<SpanContext>,
    /// 服务名称
    /// Service name
    pub service_name: String,
    /// 操作名称
    /// Operation name
    pub operation_name: String,
    /// 开始时间
    /// Start time
    pub start_time: chrono::DateTime<chrono::Utc>,
    /// 额外属性
    /// Additional attributes
    pub attributes: std::collections::HashMap<String, String>,
}

impl TraceContext {
    /// 创建新的根追踪上下文
    /// Create a new root trace context
    pub fn new_root(service_name: &str, operation_name: &str) -> Self {
        Self {
            span_context: SpanContext::new(
                TraceId::new(),
                SpanId::new(),
                TraceFlags::SAMPLED,
                false,
            ),
            parent_span_context: None,
            service_name: service_name.to_string(),
            operation_name: operation_name.to_string(),
            start_time: chrono::Utc::now(),
            attributes: std::collections::HashMap::new(),
        }
    }

    /// 创建子追踪上下文
    /// Create a child trace context
    pub fn new_child(&self, operation_name: &str) -> Self {
        Self {
            span_context: SpanContext::new(
                self.span_context.trace_id,
                SpanId::new(),
                self.span_context.trace_flags,
                false,
            ),
            parent_span_context: Some(self.span_context.clone()),
            service_name: self.service_name.clone(),
            operation_name: operation_name.to_string(),
            start_time: chrono::Utc::now(),
            attributes: std::collections::HashMap::new(),
        }
    }

    /// 从远程 SpanContext 创建
    /// Create from a remote SpanContext
    pub fn from_remote(
        span_context: SpanContext,
        service_name: &str,
        operation_name: &str,
    ) -> Self {
        Self {
            span_context: SpanContext::new(
                span_context.trace_id,
                SpanId::new(),
                span_context.trace_flags,
                false,
            ),
            parent_span_context: Some(SpanContext {
                is_remote: true,
                ..span_context
            }),
            service_name: service_name.to_string(),
            operation_name: operation_name.to_string(),
            start_time: chrono::Utc::now(),
            attributes: std::collections::HashMap::new(),
        }
    }

    /// 获取 Trace ID
    /// Get Trace ID
    pub fn trace_id(&self) -> TraceId {
        self.span_context.trace_id
    }

    /// 获取 Span ID
    /// Get Span ID
    pub fn span_id(&self) -> SpanId {
        self.span_context.span_id
    }

    /// 获取父 Span ID
    /// Get Parent Span ID
    pub fn parent_span_id(&self) -> Option<SpanId> {
        self.parent_span_context.as_ref().map(|ctx| ctx.span_id)
    }

    /// 设置属性
    /// Set an attribute
    pub fn set_attribute(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.attributes.insert(key.into(), value.into());
    }

    /// 获取属性
    /// Get an attribute
    pub fn get_attribute(&self, key: &str) -> Option<&str> {
        self.attributes.get(key).map(|s| s.as_str())
    }

    /// 计算持续时间
    /// Calculate duration
    pub fn duration(&self) -> chrono::Duration {
        chrono::Utc::now() - self.start_time
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trace_id() {
        let id = TraceId::new();
        assert!(id.is_valid());
        assert_eq!(id.to_hex().len(), 32);

        let parsed = TraceId::from_hex(&id.to_hex()).unwrap();
        assert_eq!(id, parsed);

        assert!(!TraceId::INVALID.is_valid());
    }

    #[test]
    fn test_span_id() {
        let id = SpanId::new();
        assert!(id.is_valid());
        assert_eq!(id.to_hex().len(), 16);

        let parsed = SpanId::from_hex(&id.to_hex()).unwrap();
        assert_eq!(id, parsed);

        assert!(!SpanId::INVALID.is_valid());
    }

    #[test]
    fn test_trace_flags() {
        let flags = TraceFlags::SAMPLED;
        assert!(flags.is_sampled());

        let flags = TraceFlags::NONE;
        assert!(!flags.is_sampled());

        let flags = TraceFlags::NONE.with_sampled(true);
        assert!(flags.is_sampled());
    }

    #[test]
    fn test_trace_state() {
        let mut state = TraceState::new();
        state.insert("vendor1", "value1");
        state.insert("vendor2", "value2");

        assert_eq!(state.get("vendor1"), Some("value1"));
        assert_eq!(state.get("vendor2"), Some("value2"));
        assert_eq!(state.get("vendor3"), None);

        let header = state.to_header();
        let parsed = TraceState::from_header(&header);
        assert_eq!(parsed.get("vendor1"), Some("value1"));
    }

    #[test]
    fn test_trace_context() {
        let ctx = TraceContext::new_root("test-service", "test-operation");
        assert!(ctx.span_context.is_valid());
        assert!(ctx.parent_span_context.is_none());

        let child = ctx.new_child("child-operation");
        assert_eq!(child.trace_id(), ctx.trace_id());
        assert_ne!(child.span_id(), ctx.span_id());
        assert_eq!(child.parent_span_id(), Some(ctx.span_id()));
    }
}
