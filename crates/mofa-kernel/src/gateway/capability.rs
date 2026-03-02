//! Backend capability registry — kernel contract.
//!
//! The [`CapabilityRegistry`] trait is the single kernel-level abstraction for
//! discovering and managing the backend targets that the gateway can forward
//! requests to.  Concrete implementations (in-memory, service-mesh, Consul …)
//! live in `mofa-gateway` or plugin crates.

use super::error::GatewayError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ─────────────────────────────────────────────────────────────────────────────
// Backend kind
// ─────────────────────────────────────────────────────────────────────────────

/// Classifies what *type* of service a backend represents.
///
/// This drives capability-matching logic: an LLM route must not be forwarded
/// to an IoT backend, for example.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum BackendKind {
    /// OpenAI-compatible completion / embedding endpoint.
    LlmOpenAI,
    /// Anthropic Claude API endpoint.
    LlmAnthropic,
    /// Generic OpenAI-compatible endpoint (e.g. local LLM, Azure OpenAI).
    LlmCompatible,
    /// MCP (Model Context Protocol) tool server.
    McpTool,
    /// Agent-to-Agent (A2A) communication target.
    A2AAgent,
    /// IoT hub / device endpoint.
    IoT,
    /// Arbitrary HTTP service.
    Http,
}

// ─────────────────────────────────────────────────────────────────────────────
// Health status
// ─────────────────────────────────────────────────────────────────────────────

/// Last-known health state of a backend, updated by health-check polling.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[non_exhaustive]
pub enum BackendHealth {
    /// Backend is responding normally.
    Healthy,
    /// Backend is responding but with elevated latency or partial errors.
    Degraded(String),
    /// Backend is not responding or returning errors.
    Unhealthy(String),
    /// Health has not yet been checked since registration.
    #[default]
    Unknown,
}

// ─────────────────────────────────────────────────────────────────────────────
// CapabilityDescriptor
// ─────────────────────────────────────────────────────────────────────────────

/// Full description of a backend registered in the capability registry.
///
/// All registered backends have a unique `id`.  The `kind` field drives
/// routing rules; `endpoint` is the URL the gateway will forward to.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapabilityDescriptor {
    /// Unique stable identifier (must not be empty).
    pub id: String,
    /// Classification of this backend.
    pub kind: BackendKind,
    /// Base URL for forwarding (e.g. `https://api.openai.com`).
    pub endpoint: String,
    /// Optional health-check path appended to `endpoint`
    /// (e.g. `/health` → `GET {endpoint}/health`).
    pub health_check_path: Option<String>,
    /// Arbitrary key-value metadata (model names, regions, labels, …).
    pub metadata: HashMap<String, serde_json::Value>,
    /// Last-known health state (updated by the health-check loop).
    #[serde(default)]
    pub health: BackendHealth,
}

impl CapabilityDescriptor {
    /// Construct a minimal descriptor.
    pub fn new(
        id: impl Into<String>,
        kind: BackendKind,
        endpoint: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            kind,
            endpoint: endpoint.into(),
            health_check_path: None,
            metadata: HashMap::new(),
            health: BackendHealth::Unknown,
        }
    }

    /// Builder: set the health-check path.
    pub fn with_health_check(mut self, path: impl Into<String>) -> Self {
        self.health_check_path = Some(path.into());
        self
    }

    /// Builder: attach arbitrary metadata.
    pub fn with_metadata(
        mut self,
        key: impl Into<String>,
        value: serde_json::Value,
    ) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Basic sanity checks run during [`GatewayConfig::validate()`].
    pub(crate) fn validate(&self) -> Result<(), GatewayError> {
        if self.id.trim().is_empty() {
            return Err(GatewayError::EmptyBackendId);
        }
        if self.endpoint.trim().is_empty() {
            return Err(GatewayError::InvalidEndpoint(
                self.id.clone(),
                "endpoint URI cannot be empty".to_string(),
            ));
        }
        if !self.endpoint.starts_with("http://") && !self.endpoint.starts_with("https://") {
            return Err(GatewayError::InvalidEndpoint(
                self.id.clone(),
                format!(
                    "endpoint '{}' must start with http:// or https://",
                    self.endpoint
                ),
            ));
        }
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// CapabilityRegistry trait
// ─────────────────────────────────────────────────────────────────────────────

/// Kernel contract for the backend capability registry.
///
/// Implementations store [`CapabilityDescriptor`]s and expose lookup and
/// filtering operations used by the router and health-check system.
#[async_trait]
pub trait CapabilityRegistry: Send + Sync {
    /// Register a new backend.
    ///
    /// Returns [`GatewayError::DuplicateBackend`] if a descriptor with the
    /// same `id` already exists.
    fn register(&mut self, descriptor: CapabilityDescriptor) -> Result<(), GatewayError>;

    /// Look up a backend by its unique id.  Returns `None` if not found.
    fn lookup(&self, id: &str) -> Option<&CapabilityDescriptor>;

    /// Return all backends of a specific [`BackendKind`].
    fn list_by_kind(&self, kind: &BackendKind) -> Vec<&CapabilityDescriptor>;

    /// Return all registered backends.
    fn list_all(&self) -> Vec<&CapabilityDescriptor>;

    /// Remove a backend by id.
    ///
    /// Returns [`GatewayError::DuplicateBackend`] (used as "not found") if
    /// the id is absent.
    fn deregister(&mut self, id: &str) -> Result<(), GatewayError>;

    /// Update the health state of a registered backend.
    ///
    /// Returns [`GatewayError::DuplicateBackend`] (used as "not found") if
    /// the id is absent.
    fn update_health(
        &mut self,
        id: &str,
        health: BackendHealth,
    ) -> Result<(), GatewayError>;
}
