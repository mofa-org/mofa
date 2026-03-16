//! Review Policy
//! Trait for defining review policies

use crate::hitl::{HitlResult, ReviewContext, ReviewRequest, ReviewType};
use async_trait::async_trait;

/// Review policy trait
///
/// Policies determine when reviews should be requested and whether
/// they can be auto-approved.
#[async_trait]
pub trait ReviewPolicy: Send + Sync {
    /// Determine if a review should be requested for the given context
    async fn should_request_review(
        &self,
        context: &ReviewContext,
    ) -> HitlResult<Option<ReviewRequest>>;

    /// Check if a review request can be auto-approved
    async fn can_auto_approve(&self, request: &ReviewRequest) -> HitlResult<bool>;

    /// Get policy name (for logging/debugging)
    fn name(&self) -> &str;
}

/// Always request review policy
pub struct AlwaysReviewPolicy;

#[async_trait]
impl ReviewPolicy for AlwaysReviewPolicy {
    async fn should_request_review(
        &self,
        context: &ReviewContext,
    ) -> HitlResult<Option<ReviewRequest>> {
        let request = ReviewRequest::new(
            "unknown", // execution_id will be set by caller
            ReviewType::Approval,
            context.clone(),
        );
        Ok(Some(request))
    }

    async fn can_auto_approve(&self, _request: &ReviewRequest) -> HitlResult<bool> {
        Ok(false)
    }

    fn name(&self) -> &str {
        "AlwaysReviewPolicy"
    }
}

/// Never request review policy
pub struct NeverReviewPolicy;

#[async_trait]
impl ReviewPolicy for NeverReviewPolicy {
    async fn should_request_review(
        &self,
        _context: &ReviewContext,
    ) -> HitlResult<Option<ReviewRequest>> {
        Ok(None)
    }

    async fn can_auto_approve(&self, _request: &ReviewRequest) -> HitlResult<bool> {
        Ok(true)
    }

    fn name(&self) -> &str {
        "NeverReviewPolicy"
    }
}
