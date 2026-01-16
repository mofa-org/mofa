//! Span 定义和管理
//!
//! 实现分布式追踪的 Span 概念

use super::context::{SpanContext, SpanId, TraceId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Span 类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SpanKind {
    /// 内部操作
    #[default]
    Internal,
    /// 服务器端（处理请求）
    Server,
    /// 客户端（发起请求）
    Client,
    /// 消息生产者
    Producer,
    /// 消息消费者
    Consumer,
}

impl std::fmt::Display for SpanKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SpanKind::Internal => write!(f, "INTERNAL"),
            SpanKind::Server => write!(f, "SERVER"),
            SpanKind::Client => write!(f, "CLIENT"),
            SpanKind::Producer => write!(f, "PRODUCER"),
            SpanKind::Consumer => write!(f, "CONSUMER"),
        }
    }
}

/// Span 状态
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum SpanStatus {
    /// 未设置
    #[default]
    Unset,
    /// 成功
    Ok,
    /// 错误
    Error { message: String },
}

/// Span 属性值
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SpanAttribute {
    String(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    StringArray(Vec<String>),
    IntArray(Vec<i64>),
    FloatArray(Vec<f64>),
    BoolArray(Vec<bool>),
}

impl From<&str> for SpanAttribute {
    fn from(v: &str) -> Self {
        SpanAttribute::String(v.to_string())
    }
}

impl From<String> for SpanAttribute {
    fn from(v: String) -> Self {
        SpanAttribute::String(v)
    }
}

impl From<i64> for SpanAttribute {
    fn from(v: i64) -> Self {
        SpanAttribute::Int(v)
    }
}

impl From<i32> for SpanAttribute {
    fn from(v: i32) -> Self {
        SpanAttribute::Int(v as i64)
    }
}

impl From<f64> for SpanAttribute {
    fn from(v: f64) -> Self {
        SpanAttribute::Float(v)
    }
}

impl From<bool> for SpanAttribute {
    fn from(v: bool) -> Self {
        SpanAttribute::Bool(v)
    }
}

impl From<Vec<String>> for SpanAttribute {
    fn from(v: Vec<String>) -> Self {
        SpanAttribute::StringArray(v)
    }
}

/// Span 事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanEvent {
    /// 事件名称
    pub name: String,
    /// 事件时间
    pub timestamp: DateTime<Utc>,
    /// 事件属性
    pub attributes: HashMap<String, SpanAttribute>,
}

impl SpanEvent {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            timestamp: Utc::now(),
            attributes: HashMap::new(),
        }
    }

    pub fn with_attribute(
        mut self,
        key: impl Into<String>,
        value: impl Into<SpanAttribute>,
    ) -> Self {
        self.attributes.insert(key.into(), value.into());
        self
    }
}

/// Span 链接 - 关联到其他 Span
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanLink {
    /// 链接的 SpanContext
    pub span_context: SpanContext,
    /// 链接属性
    pub attributes: HashMap<String, SpanAttribute>,
}

impl SpanLink {
    pub fn new(span_context: SpanContext) -> Self {
        Self {
            span_context,
            attributes: HashMap::new(),
        }
    }

    pub fn with_attribute(
        mut self,
        key: impl Into<String>,
        value: impl Into<SpanAttribute>,
    ) -> Self {
        self.attributes.insert(key.into(), value.into());
        self
    }
}

/// Span 数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanData {
    /// Span 上下文
    pub span_context: SpanContext,
    /// 父 Span 上下文
    pub parent_span_context: Option<SpanContext>,
    /// Span 名称
    pub name: String,
    /// Span 类型
    pub kind: SpanKind,
    /// 开始时间
    pub start_time: DateTime<Utc>,
    /// 结束时间
    pub end_time: Option<DateTime<Utc>>,
    /// 状态
    pub status: SpanStatus,
    /// 属性
    pub attributes: HashMap<String, SpanAttribute>,
    /// 事件
    pub events: Vec<SpanEvent>,
    /// 链接
    pub links: Vec<SpanLink>,
    /// 服务名称
    pub service_name: String,
}

/// Span 内部状态
struct SpanInner {
    data: SpanData,
    is_recording: bool,
    is_ended: bool,
}

/// Span - 追踪的基本单元
pub struct Span {
    inner: Arc<RwLock<SpanInner>>,
}

impl Span {
    /// 创建新的 Span
    pub(crate) fn new(
        name: impl Into<String>,
        span_context: SpanContext,
        parent_span_context: Option<SpanContext>,
        kind: SpanKind,
        service_name: impl Into<String>,
    ) -> Self {
        let inner = SpanInner {
            data: SpanData {
                span_context,
                parent_span_context,
                name: name.into(),
                kind,
                start_time: Utc::now(),
                end_time: None,
                status: SpanStatus::Unset,
                attributes: HashMap::new(),
                events: Vec::new(),
                links: Vec::new(),
                service_name: service_name.into(),
            },
            is_recording: true,
            is_ended: false,
        };
        Self {
            inner: Arc::new(RwLock::new(inner)),
        }
    }

    /// 创建非记录的 Span（用于未采样的情况）
    pub fn non_recording(span_context: SpanContext) -> Self {
        let inner = SpanInner {
            data: SpanData {
                span_context,
                parent_span_context: None,
                name: String::new(),
                kind: SpanKind::Internal,
                start_time: Utc::now(),
                end_time: None,
                status: SpanStatus::Unset,
                attributes: HashMap::new(),
                events: Vec::new(),
                links: Vec::new(),
                service_name: String::new(),
            },
            is_recording: false,
            is_ended: false,
        };
        Self {
            inner: Arc::new(RwLock::new(inner)),
        }
    }

    /// 获取 SpanContext
    pub async fn span_context(&self) -> SpanContext {
        self.inner.read().await.data.span_context.clone()
    }

    /// 获取 Trace ID
    pub async fn trace_id(&self) -> TraceId {
        self.inner.read().await.data.span_context.trace_id
    }

    /// 获取 Span ID
    pub async fn span_id(&self) -> SpanId {
        self.inner.read().await.data.span_context.span_id
    }

    /// 是否正在记录
    pub async fn is_recording(&self) -> bool {
        self.inner.read().await.is_recording
    }

    /// 是否已结束
    pub async fn is_ended(&self) -> bool {
        self.inner.read().await.is_ended
    }

    /// 设置属性
    pub async fn set_attribute(&self, key: impl Into<String>, value: impl Into<SpanAttribute>) {
        let mut inner = self.inner.write().await;
        if inner.is_recording && !inner.is_ended {
            inner.data.attributes.insert(key.into(), value.into());
        }
    }

    /// 批量设置属性
    pub async fn set_attributes(
        &self,
        attributes: impl IntoIterator<Item = (String, SpanAttribute)>,
    ) {
        let mut inner = self.inner.write().await;
        if inner.is_recording && !inner.is_ended {
            for (key, value) in attributes {
                inner.data.attributes.insert(key, value);
            }
        }
    }

    /// 添加事件
    pub async fn add_event(&self, event: SpanEvent) {
        let mut inner = self.inner.write().await;
        if inner.is_recording && !inner.is_ended {
            inner.data.events.push(event);
        }
    }

    /// 添加简单事件
    pub async fn add_event_with_name(&self, name: impl Into<String>) {
        self.add_event(SpanEvent::new(name)).await;
    }

    /// 添加链接
    pub async fn add_link(&self, link: SpanLink) {
        let mut inner = self.inner.write().await;
        if inner.is_recording && !inner.is_ended {
            inner.data.links.push(link);
        }
    }

    /// 设置状态
    pub async fn set_status(&self, status: SpanStatus) {
        let mut inner = self.inner.write().await;
        if inner.is_recording && !inner.is_ended {
            inner.data.status = status;
        }
    }

    /// 设置为成功状态
    pub async fn set_ok(&self) {
        self.set_status(SpanStatus::Ok).await;
    }

    /// 设置为错误状态
    pub async fn set_error(&self, message: impl Into<String>) {
        self.set_status(SpanStatus::Error {
            message: message.into(),
        })
        .await;
    }

    /// 记录异常
    pub async fn record_exception(&self, error: &dyn std::error::Error) {
        let event = SpanEvent::new("exception")
            .with_attribute("exception.type", std::any::type_name_of_val(error))
            .with_attribute("exception.message", error.to_string());
        self.add_event(event).await;
        self.set_error(error.to_string()).await;
    }

    /// 更新名称
    pub async fn update_name(&self, name: impl Into<String>) {
        let mut inner = self.inner.write().await;
        if inner.is_recording && !inner.is_ended {
            inner.data.name = name.into();
        }
    }

    /// 结束 Span
    pub async fn end(&self) {
        let mut inner = self.inner.write().await;
        if !inner.is_ended {
            inner.is_ended = true;
            inner.data.end_time = Some(Utc::now());
        }
    }

    /// 结束 Span 并指定时间
    pub async fn end_with_timestamp(&self, timestamp: DateTime<Utc>) {
        let mut inner = self.inner.write().await;
        if !inner.is_ended {
            inner.is_ended = true;
            inner.data.end_time = Some(timestamp);
        }
    }

    /// 获取 Span 数据（用于导出）
    pub async fn get_data(&self) -> SpanData {
        self.inner.read().await.data.clone()
    }

    /// 获取持续时间（毫秒）
    pub async fn duration_ms(&self) -> Option<i64> {
        let inner = self.inner.read().await;
        inner
            .data
            .end_time
            .map(|end| (end - inner.data.start_time).num_milliseconds())
    }
}

impl Clone for Span {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

/// Span 构建器
pub struct SpanBuilder {
    name: String,
    kind: SpanKind,
    parent_context: Option<SpanContext>,
    attributes: HashMap<String, SpanAttribute>,
    links: Vec<SpanLink>,
    start_time: Option<DateTime<Utc>>,
    service_name: String,
}

impl SpanBuilder {
    /// 创建新的 SpanBuilder
    pub fn new(name: impl Into<String>, service_name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            kind: SpanKind::Internal,
            parent_context: None,
            attributes: HashMap::new(),
            links: Vec::new(),
            start_time: None,
            service_name: service_name.into(),
        }
    }

    /// 设置 Span 类型
    pub fn with_kind(mut self, kind: SpanKind) -> Self {
        self.kind = kind;
        self
    }

    /// 设置父上下文
    pub fn with_parent(mut self, parent: SpanContext) -> Self {
        self.parent_context = Some(parent);
        self
    }

    /// 添加属性
    pub fn with_attribute(
        mut self,
        key: impl Into<String>,
        value: impl Into<SpanAttribute>,
    ) -> Self {
        self.attributes.insert(key.into(), value.into());
        self
    }

    /// 添加多个属性
    pub fn with_attributes(
        mut self,
        attributes: impl IntoIterator<Item = (String, SpanAttribute)>,
    ) -> Self {
        for (key, value) in attributes {
            self.attributes.insert(key, value);
        }
        self
    }

    /// 添加链接
    pub fn with_link(mut self, link: SpanLink) -> Self {
        self.links.push(link);
        self
    }

    /// 设置开始时间
    pub fn with_start_time(mut self, start_time: DateTime<Utc>) -> Self {
        self.start_time = Some(start_time);
        self
    }

    /// 构建 Span
    pub fn start(self) -> Span {
        use super::context::{TraceFlags, TraceId};

        let (trace_id, _parent_span_id) = match &self.parent_context {
            Some(parent) => (parent.trace_id, Some(parent.span_id)),
            None => (TraceId::new(), None),
        };

        let span_context = SpanContext::new(trace_id, SpanId::new(), TraceFlags::SAMPLED, false);

        let span = Span::new(
            self.name,
            span_context,
            self.parent_context,
            self.kind,
            self.service_name,
        );

        // 设置属性
        let span_clone = span.clone();
        let attributes = self.attributes;
        let links = self.links;

        tokio::spawn(async move {
            for (key, value) in attributes {
                span_clone.set_attribute(key, value).await;
            }
            for link in links {
                span_clone.add_link(link).await;
            }
        });

        span
    }
}

#[cfg(test)]
mod tests {
    use super::super::context::TraceFlags;
    use super::*;

    #[tokio::test]
    async fn test_span_creation() {
        let span_context =
            SpanContext::new(TraceId::new(), SpanId::new(), TraceFlags::SAMPLED, false);

        let span = Span::new(
            "test-span",
            span_context.clone(),
            None,
            SpanKind::Internal,
            "test-service",
        );

        assert!(span.is_recording().await);
        assert!(!span.is_ended().await);
        assert_eq!(span.trace_id().await, span_context.trace_id);
    }

    #[tokio::test]
    async fn test_span_attributes() {
        let span_context =
            SpanContext::new(TraceId::new(), SpanId::new(), TraceFlags::SAMPLED, false);

        let span = Span::new(
            "test-span",
            span_context,
            None,
            SpanKind::Internal,
            "test-service",
        );

        span.set_attribute("key1", "value1").await;
        span.set_attribute("key2", 42i64).await;
        span.set_attribute("key3", true).await;

        let data = span.get_data().await;
        assert_eq!(data.attributes.len(), 3);
    }

    #[tokio::test]
    async fn test_span_events() {
        let span_context =
            SpanContext::new(TraceId::new(), SpanId::new(), TraceFlags::SAMPLED, false);

        let span = Span::new(
            "test-span",
            span_context,
            None,
            SpanKind::Internal,
            "test-service",
        );

        span.add_event_with_name("event1").await;
        span.add_event(SpanEvent::new("event2").with_attribute("attr", "value"))
            .await;

        let data = span.get_data().await;
        assert_eq!(data.events.len(), 2);
    }

    #[tokio::test]
    async fn test_span_end() {
        let span_context =
            SpanContext::new(TraceId::new(), SpanId::new(), TraceFlags::SAMPLED, false);

        let span = Span::new(
            "test-span",
            span_context,
            None,
            SpanKind::Internal,
            "test-service",
        );

        assert!(!span.is_ended().await);
        span.end().await;
        assert!(span.is_ended().await);

        let data = span.get_data().await;
        assert!(data.end_time.is_some());
    }

    #[tokio::test]
    async fn test_span_builder() {
        let span = SpanBuilder::new("test-span", "test-service")
            .with_kind(SpanKind::Server)
            .with_attribute("http.method", "GET")
            .start();

        let data = span.get_data().await;
        assert_eq!(data.name, "test-span");
        assert_eq!(data.kind, SpanKind::Server);
    }
}
