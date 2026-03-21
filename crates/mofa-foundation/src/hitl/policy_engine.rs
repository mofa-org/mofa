//! Review Policy Engine
//!
//! Evaluates review policies to determine when reviews are needed

use crate::hitl::error::FoundationHitlError;
use mofa_kernel::hitl::{ReviewContext, ReviewPolicy, ReviewRequest};
use std::sync::Arc;

/// Review policy engine
pub struct ReviewPolicyEngine {
    policies: Vec<Arc<dyn ReviewPolicy>>,
}

impl ReviewPolicyEngine {
    /// Create a new policy engine
    pub fn new(policies: Vec<Arc<dyn ReviewPolicy>>) -> Self {
        Self { policies }
    }

    /// Evaluate policies to determine if a review is needed
    pub async fn should_request_review(
        &self,
        context: &ReviewContext,
    ) -> Result<Option<ReviewRequest>, FoundationHitlError> {
        for policy in &self.policies {
            match policy.should_request_review(context).await {
                Ok(Some(request)) => return Ok(Some(request)),
                Ok(None) => continue,
                Err(e) => {
                    tracing::warn!("Policy {} evaluation failed: {}", policy.name(), e);
                    continue;
                }
            }
        }
        Ok(None)
    }

    /// Check if a review can be auto-approved
    pub async fn can_auto_approve(
        &self,
        request: &ReviewRequest,
    ) -> Result<bool, FoundationHitlError> {
        for policy in &self.policies {
            match policy.can_auto_approve(request).await {
                Ok(true) => return Ok(true),
                Ok(false) => continue,
                Err(e) => {
                    tracing::warn!("Policy {} auto-approval check failed: {}", policy.name(), e);
                    continue;
                }
            }
        }
        Ok(false)
    }
}

impl Default for ReviewPolicyEngine {
    fn default() -> Self {
        Self::new(vec![])
    }
}
