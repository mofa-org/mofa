//! Human-in-the-Loop (HITL) System
//!
//! Production-ready HITL system for MoFA with persistent review queue,
//! review policies, notifications, rate limiting, and multi-tenancy support.

pub mod analytics;
pub mod audit;
pub mod error;
pub mod manager;
pub mod notifier;
pub mod policy_engine;
pub mod rate_limiter;
pub mod store;
pub mod webhook;

#[cfg(feature = "http-api")]
pub mod api;

pub mod handlers;

pub use analytics::{ReviewAnalytics, ReviewMetrics, ReviewerMetrics, TimeSeriesPoint};
pub use audit::{AuditStore, AuditStoreError, InMemoryAuditStore};
pub use error::FoundationHitlError;
pub use manager::{ReviewManager, ReviewManagerConfig};
pub use notifier::{NotificationChannel, ReviewNotifier};
pub use policy_engine::ReviewPolicyEngine;
pub use rate_limiter::RateLimiter;
pub use store::{InMemoryReviewStore, ReviewStore, ReviewStoreError};
pub use webhook::{WebhookConfig, WebhookDelivery};

#[cfg(feature = "http-api")]
pub use api::{
    ResolveReviewRequest, ReviewApiState, ReviewDetailResponse, ReviewListResponse,
    create_review_api_router,
};

pub use handlers::*;
