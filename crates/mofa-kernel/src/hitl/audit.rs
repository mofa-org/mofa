//! HITL Audit Trail
//!
//! Immutable audit log for review operations

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Audit event type for review operations
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ReviewAuditEventType {
    /// Review request created
    Created,
    /// Review resolved (approved/rejected/changes requested)
    Resolved,
    /// Review expired
    Expired,
    /// Review cancelled
    Cancelled,
    /// Review updated (metadata, priority, etc.)
    Updated,
    /// Review assigned to reviewer
    Assigned,
    /// Review unassigned
    Unassigned,
    /// Review escalated
    Escalated,
    /// Review delegated
    Delegated,
}

/// Review audit event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewAuditEvent {
    /// Event ID (unique)
    pub event_id: String,
    /// Review request ID
    pub review_id: String,
    /// Event type
    pub event_type: ReviewAuditEventType,
    /// Execution ID (if applicable)
    pub execution_id: Option<String>,
    /// Node ID (if applicable)
    pub node_id: Option<String>,
    /// Tenant ID (if applicable)
    pub tenant_id: Option<Uuid>,
    /// Actor (who performed the action)
    pub actor: Option<String>,
    /// Timestamp (milliseconds since epoch)
    pub timestamp_ms: u64,
    /// Event data (context-specific)
    pub data: HashMap<String, serde_json::Value>,
    /// IP address (if available)
    pub ip_address: Option<String>,
    /// User agent (if available)
    pub user_agent: Option<String>,
}

impl ReviewAuditEvent {
    /// Create a new audit event
    pub fn new(
        review_id: impl Into<String>,
        event_type: ReviewAuditEventType,
        actor: Option<String>,
    ) -> Self {
        Self {
            event_id: Uuid::new_v4().to_string(),
            review_id: review_id.into(),
            event_type,
            execution_id: None,
            node_id: None,
            tenant_id: None,
            actor,
            timestamp_ms: chrono::Utc::now().timestamp_millis() as u64,
            data: HashMap::new(),
            ip_address: None,
            user_agent: None,
        }
    }

    /// Set execution ID
    pub fn with_execution_id(mut self, execution_id: impl Into<String>) -> Self {
        self.execution_id = Some(execution_id.into());
        self
    }

    /// Set node ID
    pub fn with_node_id(mut self, node_id: impl Into<String>) -> Self {
        self.node_id = Some(node_id.into());
        self
    }

    /// Set tenant ID
    pub fn with_tenant_id(mut self, tenant_id: Uuid) -> Self {
        self.tenant_id = Some(tenant_id);
        self
    }

    /// Set event data
    pub fn with_data(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.data.insert(key.into(), value);
        self
    }

    /// Set IP address
    pub fn with_ip_address(mut self, ip: impl Into<String>) -> Self {
        self.ip_address = Some(ip.into());
        self
    }

    /// Set user agent
    pub fn with_user_agent(mut self, ua: impl Into<String>) -> Self {
        self.user_agent = Some(ua.into());
        self
    }
}

/// Audit log query filter
#[derive(Debug, Clone, Default)]
pub struct AuditLogQuery {
    /// Filter by review ID
    pub review_id: Option<String>,
    /// Filter by execution ID
    pub execution_id: Option<String>,
    /// Filter by tenant ID
    pub tenant_id: Option<Uuid>,
    /// Filter by event type
    pub event_type: Option<ReviewAuditEventType>,
    /// Filter by actor
    pub actor: Option<String>,
    /// Start timestamp (inclusive)
    pub start_time_ms: Option<u64>,
    /// End timestamp (exclusive)
    pub end_time_ms: Option<u64>,
    /// Maximum number of results
    pub limit: Option<u64>,
    /// Offset for pagination
    pub offset: Option<u64>,
}
