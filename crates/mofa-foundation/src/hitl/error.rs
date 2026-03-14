//! HITL Error Types for Foundation Layer
//!
//! Error types specific to the foundation layer HITL implementation

use crate::hitl::store::ReviewStoreError;
use mofa_kernel::hitl::HitlError;
use thiserror::Error;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum FoundationHitlError {
    #[error("Review store error: {0}")]
    Store(#[from] ReviewStoreError),

    #[error("Rate limit exceeded: {0}")]
    RateLimit(String),

    #[error("Webhook delivery failed: {0}")]
    WebhookDelivery(String),

    #[error("Notification failed: {0}")]
    Notification(String),

    #[error("Policy evaluation failed: {0}")]
    PolicyEvaluation(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("Audit error: {0}")]
    Audit(String),
}

impl From<HitlError> for FoundationHitlError {
    fn from(err: HitlError) -> Self {
        match err {
            HitlError::StoreError(store_err) => {
                FoundationHitlError::Store(ReviewStoreError::from(store_err))
            }
            HitlError::RateLimitExceeded {
                tenant_id,
                retry_after_secs,
            } => FoundationHitlError::RateLimit(format!(
                "Tenant {} exceeded rate limit, retry after {}s",
                tenant_id, retry_after_secs
            )),
            HitlError::PolicyError { reason } => FoundationHitlError::PolicyEvaluation(reason),
            HitlError::ReviewNotFound { id } => {
                FoundationHitlError::InvalidConfig(format!("Review not found: {}", id))
            }
            HitlError::ReviewExpired { id } => {
                FoundationHitlError::InvalidConfig(format!("Review expired: {}", id))
            }
            HitlError::ReviewAlreadyResolved { id } => {
                FoundationHitlError::InvalidConfig(format!("Review already resolved: {}", id))
            }
            HitlError::InvalidResponse { reason } => {
                FoundationHitlError::InvalidConfig(format!("Invalid response: {}", reason))
            }
            HitlError::NotificationError { reason } => FoundationHitlError::Notification(reason),
            HitlError::SerializationError(msg) => FoundationHitlError::Serialization(msg),
            HitlError::ReviewTimeout { id } => {
                FoundationHitlError::InvalidConfig(format!("Review timeout: {}", id))
            }
            HitlError::WebhookError { reason } => FoundationHitlError::WebhookDelivery(reason),
            HitlError::TenantAccessDenied { tenant_id } => {
                FoundationHitlError::InvalidConfig(format!("Tenant access denied: {}", tenant_id))
            }
            // Handle future variants added to non_exhaustive enum
            _ => FoundationHitlError::InvalidConfig(err.to_string()),
        }
    }
}

impl From<mofa_kernel::hitl::StoreError> for ReviewStoreError {
    fn from(err: mofa_kernel::hitl::StoreError) -> Self {
        match err {
            mofa_kernel::hitl::StoreError::Connection(msg) => ReviewStoreError::Connection(msg),
            mofa_kernel::hitl::StoreError::Query(msg) => ReviewStoreError::Query(msg),
            mofa_kernel::hitl::StoreError::NotFound(msg) => ReviewStoreError::NotFound(msg),
            // Handle future variants added to non_exhaustive enum
            _ => ReviewStoreError::Query(format!("Unknown store error: {}", err)),
        }
    }
}

pub type HitlResult<T> = Result<T, FoundationHitlError>;
