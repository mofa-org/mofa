//! HITL Analytics and Metrics
//!
//! Analytics and metrics collection for review operations

use crate::hitl::audit::AuditStore;
use mofa_kernel::hitl::{AuditLogQuery, ReviewAuditEventType, ReviewStatus};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use uuid::Uuid;

/// Review analytics metrics
#[derive(Debug, Clone, serde::Serialize)]
pub struct ReviewMetrics {
    /// Total reviews created
    pub total_reviews: u64,
    /// Pending reviews
    pub pending_reviews: u64,
    /// Approved reviews
    pub approved_reviews: u64,
    /// Rejected reviews
    pub rejected_reviews: u64,
    /// Expired reviews
    pub expired_reviews: u64,
    /// Average review time (milliseconds)
    pub average_review_time_ms: Option<u64>,
    /// Median review time (milliseconds)
    pub median_review_time_ms: Option<u64>,
    /// Review approval rate (0.0 to 1.0)
    pub approval_rate: f64,
    /// Reviews by type
    pub reviews_by_type: std::collections::HashMap<String, u64>,
    /// Reviews by status
    pub reviews_by_status: std::collections::HashMap<String, u64>,
}

/// Reviewer activity metrics
#[derive(Debug, Clone, serde::Serialize)]
pub struct ReviewerMetrics {
    /// Reviewer identifier
    pub reviewer: String,
    /// Total reviews resolved
    pub total_resolved: u64,
    /// Approved count
    pub approved: u64,
    /// Rejected count
    pub rejected: u64,
    /// Average review time (milliseconds)
    pub average_review_time_ms: Option<u64>,
    /// Last review timestamp
    pub last_review_at: Option<u64>,
}

/// Analytics engine for review metrics
pub struct ReviewAnalytics {
    audit_store: Arc<dyn AuditStore>,
}

impl ReviewAnalytics {
    /// Create new analytics engine
    pub fn new(audit_store: Arc<dyn AuditStore>) -> Self {
        Self { audit_store }
    }

    /// Calculate review metrics for a tenant
    pub async fn calculate_metrics(
        &self,
        tenant_id: Option<Uuid>,
        start_time_ms: Option<u64>,
        end_time_ms: Option<u64>,
    ) -> Result<ReviewMetrics, crate::hitl::audit::AuditStoreError> {
        let query = AuditLogQuery {
            tenant_id,
            start_time_ms,
            end_time_ms,
            limit: Some(10000), // Large limit for analytics
            ..Default::default()
        };

        let events = self.audit_store.query_events(&query).await?;

        let mut total_reviews = 0u64;
        let mut pending_reviews = 0u64;
        let mut approved_reviews = 0u64;
        let mut rejected_reviews = 0u64;
        let mut expired_reviews = 0u64;
        let mut reviews_by_type = std::collections::HashMap::new();
        let mut reviews_by_status = std::collections::HashMap::new();
        let mut review_times: Vec<u64> = Vec::new();

        // Track review creation and resolution times
        let mut review_created: std::collections::HashMap<String, u64> =
            std::collections::HashMap::new();

        for event in &events {
            match event.event_type {
                ReviewAuditEventType::Created => {
                    total_reviews += 1;
                    pending_reviews += 1;
                    review_created.insert(event.review_id.clone(), event.timestamp_ms);

                    // Extract review type from event data
                    if let Some(review_type) = event.data.get("review_type") {
                        let type_str = review_type.as_str().unwrap_or("unknown").to_string();
                        *reviews_by_type.entry(type_str).or_insert(0) += 1;
                    }
                }
                ReviewAuditEventType::Resolved => {
                    if let Some(created_at) = review_created.get(&event.review_id) {
                        let review_time = event.timestamp_ms.saturating_sub(*created_at);
                        review_times.push(review_time);
                    }

                    pending_reviews = pending_reviews.saturating_sub(1);

                    // Extract status from event data
                    if let Some(status_val) = event.data.get("status")
                        && let Some(status_str) = status_val.as_str()
                    {
                        let status = status_str.to_string();
                        *reviews_by_status.entry(status.clone()).or_insert(0) += 1;

                        if status.contains("Approved") {
                            approved_reviews += 1;
                        } else if status.contains("Rejected") {
                            rejected_reviews += 1;
                        }
                    }
                }
                ReviewAuditEventType::Expired => {
                    expired_reviews += 1;
                    pending_reviews = pending_reviews.saturating_sub(1);
                    *reviews_by_status.entry("Expired".to_string()).or_insert(0) += 1;
                }
                ReviewAuditEventType::Cancelled => {
                    pending_reviews = pending_reviews.saturating_sub(1);
                    *reviews_by_status
                        .entry("Cancelled".to_string())
                        .or_insert(0) += 1;
                }
                _ => {}
            }
        }

        // Calculate average and median review times
        let average_review_time_ms = if !review_times.is_empty() {
            let sum: u64 = review_times.iter().sum();
            Some(sum / review_times.len() as u64)
        } else {
            None
        };

        let median_review_time_ms = if !review_times.is_empty() {
            review_times.sort();
            let mid = review_times.len() / 2;
            Some(review_times[mid])
        } else {
            None
        };

        // Calculate approval rate
        let total_resolved = approved_reviews + rejected_reviews;
        let approval_rate = if total_resolved > 0 {
            approved_reviews as f64 / total_resolved as f64
        } else {
            0.0
        };

        Ok(ReviewMetrics {
            total_reviews,
            pending_reviews,
            approved_reviews,
            rejected_reviews,
            expired_reviews,
            average_review_time_ms,
            median_review_time_ms,
            approval_rate,
            reviews_by_type,
            reviews_by_status,
        })
    }

    /// Get reviewer activity metrics
    pub async fn get_reviewer_metrics(
        &self,
        tenant_id: Option<Uuid>,
        start_time_ms: Option<u64>,
        end_time_ms: Option<u64>,
    ) -> Result<Vec<ReviewerMetrics>, crate::hitl::audit::AuditStoreError> {
        let query = AuditLogQuery {
            tenant_id,
            start_time_ms,
            end_time_ms,
            event_type: Some(ReviewAuditEventType::Resolved),
            limit: Some(10000),
            ..Default::default()
        };

        let events = self.audit_store.query_events(&query).await?;

        let mut reviewer_stats: std::collections::HashMap<String, ReviewerStats> =
            std::collections::HashMap::new();
        let mut review_created: std::collections::HashMap<String, u64> =
            std::collections::HashMap::new();

        // First pass: collect creation times
        let creation_query = AuditLogQuery {
            tenant_id,
            start_time_ms,
            end_time_ms,
            event_type: Some(ReviewAuditEventType::Created),
            limit: Some(10000),
            ..Default::default()
        };

        let creation_events = self.audit_store.query_events(&creation_query).await?;
        for event in creation_events {
            review_created.insert(event.review_id, event.timestamp_ms);
        }

        // Second pass: process resolutions
        for event in events {
            if let Some(actor) = &event.actor {
                let stats = reviewer_stats
                    .entry(actor.clone())
                    .or_insert_with(|| ReviewerStats {
                        reviewer: actor.clone(),
                        total_resolved: 0,
                        approved: 0,
                        rejected: 0,
                        review_times: Vec::new(),
                        last_review_at: None,
                    });

                stats.total_resolved += 1;

                if let Some(created_at) = review_created.get(&event.review_id) {
                    let review_time = event.timestamp_ms.saturating_sub(*created_at);
                    stats.review_times.push(review_time);
                }

                if event.timestamp_ms > stats.last_review_at.unwrap_or(0) {
                    stats.last_review_at = Some(event.timestamp_ms);
                }

                // Check status
                if let Some(status_val) = event.data.get("status")
                    && let Some(status_str) = status_val.as_str()
                {
                    if status_str.contains("Approved") {
                        stats.approved += 1;
                    } else if status_str.contains("Rejected") {
                        stats.rejected += 1;
                    }
                }
            }
        }

        // Convert to ReviewerMetrics
        let mut metrics: Vec<ReviewerMetrics> = reviewer_stats
            .into_values()
            .map(|stats| {
                let average_review_time_ms = if !stats.review_times.is_empty() {
                    let sum: u64 = stats.review_times.iter().sum();
                    Some(sum / stats.review_times.len() as u64)
                } else {
                    None
                };

                ReviewerMetrics {
                    reviewer: stats.reviewer,
                    total_resolved: stats.total_resolved,
                    approved: stats.approved,
                    rejected: stats.rejected,
                    average_review_time_ms,
                    last_review_at: stats.last_review_at,
                }
            })
            .collect();

        // Sort by total resolved (descending)
        metrics.sort_by(|a, b| b.total_resolved.cmp(&a.total_resolved));

        Ok(metrics)
    }

    /// Get review volume over time (time series data)
    pub async fn get_review_volume(
        &self,
        tenant_id: Option<Uuid>,
        start_time_ms: u64,
        end_time_ms: u64,
        interval_ms: u64,
    ) -> Result<Vec<TimeSeriesPoint>, crate::hitl::audit::AuditStoreError> {
        let query = AuditLogQuery {
            tenant_id,
            start_time_ms: Some(start_time_ms),
            end_time_ms: Some(end_time_ms),
            event_type: Some(ReviewAuditEventType::Created),
            limit: Some(100000),
            ..Default::default()
        };

        let events = self.audit_store.query_events(&query).await?;

        // Group events by time interval
        let mut buckets: std::collections::BTreeMap<u64, u64> = std::collections::BTreeMap::new();

        for event in events {
            let bucket = (event.timestamp_ms / interval_ms) * interval_ms;
            *buckets.entry(bucket).or_insert(0) += 1;
        }

        // Convert to time series points
        let points: Vec<TimeSeriesPoint> = buckets
            .into_iter()
            .map(|(timestamp, count)| TimeSeriesPoint {
                timestamp,
                value: count,
            })
            .collect();

        Ok(points)
    }
}

/// Internal reviewer statistics
struct ReviewerStats {
    reviewer: String,
    total_resolved: u64,
    approved: u64,
    rejected: u64,
    review_times: Vec<u64>,
    last_review_at: Option<u64>,
}

/// Time series data point
#[derive(Debug, Clone, serde::Serialize)]
pub struct TimeSeriesPoint {
    pub timestamp: u64,
    pub value: u64,
}
