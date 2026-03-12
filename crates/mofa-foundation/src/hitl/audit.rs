//! HITL Audit Trail Implementation
//!
//! Immutable audit log storage and querying

use async_trait::async_trait;
use mofa_kernel::hitl::{AuditLogQuery, ReviewAuditEvent};
use std::sync::Arc;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum AuditStoreError {
    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Query error: {0}")]
    Query(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Constraint violation: {0}")]
    Constraint(String),
}

/// Audit log store trait
#[async_trait]
pub trait AuditStore: Send + Sync {
    /// Record an audit event (immutable append-only)
    async fn record_event(&self, event: &ReviewAuditEvent) -> Result<(), AuditStoreError>;

    /// Query audit events
    async fn query_events(
        &self,
        query: &AuditLogQuery,
    ) -> Result<Vec<ReviewAuditEvent>, AuditStoreError>;

    /// Get events for a specific review
    async fn get_review_events(
        &self,
        review_id: &str,
    ) -> Result<Vec<ReviewAuditEvent>, AuditStoreError>;

    /// Get events for a specific execution
    async fn get_execution_events(
        &self,
        execution_id: &str,
    ) -> Result<Vec<ReviewAuditEvent>, AuditStoreError>;

    /// Get events for a tenant
    async fn get_tenant_events(
        &self,
        tenant_id: Uuid,
        limit: Option<u64>,
    ) -> Result<Vec<ReviewAuditEvent>, AuditStoreError>;

    /// Cleanup old events (optional, for compliance retention policies)
    async fn cleanup_old_events(&self, before_timestamp_ms: u64) -> Result<u64, AuditStoreError>;
}

/// In-memory audit store (for testing)
use parking_lot::RwLock;
use std::collections::HashMap;

pub struct InMemoryAuditStore {
    events: Arc<RwLock<Vec<ReviewAuditEvent>>>,
    by_review: Arc<RwLock<HashMap<String, Vec<String>>>>, // review_id -> event_ids
    by_execution: Arc<RwLock<HashMap<String, Vec<String>>>>, // execution_id -> event_ids
    by_tenant: Arc<RwLock<HashMap<Uuid, Vec<String>>>>,   // tenant_id -> event_ids
}

impl InMemoryAuditStore {
    pub fn new() -> Self {
        Self {
            events: Arc::new(RwLock::new(Vec::new())),
            by_review: Arc::new(RwLock::new(HashMap::new())),
            by_execution: Arc::new(RwLock::new(HashMap::new())),
            by_tenant: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for InMemoryAuditStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AuditStore for InMemoryAuditStore {
    async fn record_event(&self, event: &ReviewAuditEvent) -> Result<(), AuditStoreError> {
        let mut events = self.events.write();
        events.push(event.clone());

        // Index by review ID
        if let Some(review_id) = events.last().map(|e| e.review_id.clone()) {
            self.by_review
                .write()
                .entry(review_id)
                .or_insert_with(Vec::new)
                .push(event.event_id.clone());
        }

        // Index by execution ID
        if let Some(execution_id) = event.execution_id.clone() {
            self.by_execution
                .write()
                .entry(execution_id)
                .or_insert_with(Vec::new)
                .push(event.event_id.clone());
        }

        // Index by tenant ID
        if let Some(tenant_id) = event.tenant_id {
            self.by_tenant
                .write()
                .entry(tenant_id)
                .or_insert_with(Vec::new)
                .push(event.event_id.clone());
        }

        Ok(())
    }

    async fn query_events(
        &self,
        query: &AuditLogQuery,
    ) -> Result<Vec<ReviewAuditEvent>, AuditStoreError> {
        let events = self.events.read();
        let mut results: Vec<ReviewAuditEvent> = events
            .iter()
            .filter(|event| {
                // Filter by review ID
                if let Some(ref review_id) = query.review_id {
                    if event.review_id != *review_id {
                        return false;
                    }
                }

                // Filter by execution ID
                if let Some(ref execution_id) = query.execution_id {
                    if event.execution_id.as_ref() != Some(execution_id) {
                        return false;
                    }
                }

                // Filter by tenant ID
                if let Some(tenant_id) = query.tenant_id {
                    if event.tenant_id != Some(tenant_id) {
                        return false;
                    }
                }

                // Filter by event type
                if let Some(ref event_type) = query.event_type {
                    if &event.event_type != event_type {
                        return false;
                    }
                }

                // Filter by actor
                if let Some(ref actor) = query.actor {
                    if event.actor.as_ref() != Some(actor) {
                        return false;
                    }
                }

                // Filter by time range
                if let Some(start_time) = query.start_time_ms {
                    if event.timestamp_ms < start_time {
                        return false;
                    }
                }

                if let Some(end_time) = query.end_time_ms {
                    if event.timestamp_ms >= end_time {
                        return false;
                    }
                }

                true
            })
            .cloned()
            .collect();

        // Sort by timestamp (newest first)
        results.sort_by(|a, b| b.timestamp_ms.cmp(&a.timestamp_ms));

        // Apply pagination
        let offset = query.offset.unwrap_or(0) as usize;
        let limit = query.limit.unwrap_or(1000) as usize;

        Ok(results.into_iter().skip(offset).take(limit).collect())
    }

    async fn get_review_events(
        &self,
        review_id: &str,
    ) -> Result<Vec<ReviewAuditEvent>, AuditStoreError> {
        let events = self.events.read();
        let event_ids = self.by_review.read();

        if let Some(ids) = event_ids.get(review_id) {
            let mut review_events: Vec<ReviewAuditEvent> = events
                .iter()
                .filter(|e| ids.contains(&e.event_id))
                .cloned()
                .collect();

            review_events.sort_by(|a, b| a.timestamp_ms.cmp(&b.timestamp_ms));
            Ok(review_events)
        } else {
            Ok(Vec::new())
        }
    }

    async fn get_execution_events(
        &self,
        execution_id: &str,
    ) -> Result<Vec<ReviewAuditEvent>, AuditStoreError> {
        let events = self.events.read();
        let event_ids = self.by_execution.read();

        if let Some(ids) = event_ids.get(execution_id) {
            let mut execution_events: Vec<ReviewAuditEvent> = events
                .iter()
                .filter(|e| ids.contains(&e.event_id))
                .cloned()
                .collect();

            execution_events.sort_by(|a, b| a.timestamp_ms.cmp(&b.timestamp_ms));
            Ok(execution_events)
        } else {
            Ok(Vec::new())
        }
    }

    async fn get_tenant_events(
        &self,
        tenant_id: Uuid,
        limit: Option<u64>,
    ) -> Result<Vec<ReviewAuditEvent>, AuditStoreError> {
        let events = self.events.read();
        let event_ids = self.by_tenant.read();

        if let Some(ids) = event_ids.get(&tenant_id) {
            let mut tenant_events: Vec<ReviewAuditEvent> = events
                .iter()
                .filter(|e| ids.contains(&e.event_id))
                .cloned()
                .collect();

            tenant_events.sort_by(|a, b| b.timestamp_ms.cmp(&a.timestamp_ms));

            if let Some(limit) = limit {
                tenant_events.truncate(limit as usize);
            }

            Ok(tenant_events)
        } else {
            Ok(Vec::new())
        }
    }

    async fn cleanup_old_events(&self, before_timestamp_ms: u64) -> Result<u64, AuditStoreError> {
        let mut events = self.events.write();
        let initial_count = events.len();

        events.retain(|e| e.timestamp_ms >= before_timestamp_ms);

        let removed = initial_count - events.len();

        // Rebuild indexes
        let mut by_review = self.by_review.write();
        let mut by_execution = self.by_execution.write();
        let mut by_tenant = self.by_tenant.write();

        by_review.clear();
        by_execution.clear();
        by_tenant.clear();

        for event in events.iter() {
            by_review
                .entry(event.review_id.clone())
                .or_insert_with(Vec::new)
                .push(event.event_id.clone());

            if let Some(ref execution_id) = event.execution_id {
                by_execution
                    .entry(execution_id.clone())
                    .or_insert_with(Vec::new)
                    .push(event.event_id.clone());
            }

            if let Some(tenant_id) = event.tenant_id {
                by_tenant
                    .entry(tenant_id)
                    .or_insert_with(Vec::new)
                    .push(event.event_id.clone());
            }
        }

        Ok(removed as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mofa_kernel::hitl::ReviewAuditEventType;

    #[tokio::test]
    async fn test_record_and_query_events() {
        let store = InMemoryAuditStore::new();

        let event1 = ReviewAuditEvent::new(
            "review-1",
            ReviewAuditEventType::Created,
            Some("user1".to_string()),
        )
        .with_execution_id("exec-1");
        let event2 = ReviewAuditEvent::new(
            "review-1",
            ReviewAuditEventType::Resolved,
            Some("user2".to_string()),
        )
        .with_execution_id("exec-1");

        store.record_event(&event1).await.unwrap();
        store.record_event(&event2).await.unwrap();

        let query = AuditLogQuery {
            review_id: Some("review-1".to_string()),
            ..Default::default()
        };

        let events = store.query_events(&query).await.unwrap();
        assert_eq!(events.len(), 2);

        let review_events = store.get_review_events("review-1").await.unwrap();
        assert_eq!(review_events.len(), 2);
    }
}
