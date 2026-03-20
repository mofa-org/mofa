//! Workflow Review Handler
//!
//! Integration handler for workflow-level review requests

use crate::hitl::error::HitlResult;
use crate::hitl::manager::ReviewManager;
use mofa_kernel::hitl::{
    ReviewContext, ReviewRequest, ReviewRequestId, ReviewResponse, ReviewType,
};
use std::sync::Arc;

/// Handler for workflow-level review integration
pub struct WorkflowReviewHandler {
    manager: Arc<ReviewManager>,
}

impl WorkflowReviewHandler {
    /// Create a new workflow review handler
    pub fn new(manager: Arc<ReviewManager>) -> Self {
        Self { manager }
    }

    /// Request a review for a workflow node
    pub async fn request_node_review(
        &self,
        execution_id: &str,
        node_id: &str,
        context: ReviewContext,
    ) -> HitlResult<ReviewRequestId> {
        let request =
            ReviewRequest::new(execution_id, ReviewType::Approval, context).with_node_id(node_id);

        self.manager.request_review(request).await
    }

    /// Wait for a review to be resolved
    pub async fn wait_for_review(&self, review_id: &ReviewRequestId) -> HitlResult<ReviewResponse> {
        self.manager.wait_for_review(review_id, None).await
    }

    /// Check if a review is resolved
    pub async fn is_resolved(&self, review_id: &ReviewRequestId) -> HitlResult<bool> {
        if let Some(review) = self.manager.get_review(review_id).await? {
            Ok(review.is_resolved())
        } else {
            Ok(false)
        }
    }

    /// Check if a review is approved
    pub async fn is_approved(&self, review_id: &ReviewRequestId) -> HitlResult<bool> {
        if let Some(review) = self.manager.get_review(review_id).await? {
            Ok(review.is_resolved()
                && matches!(review.status, mofa_kernel::hitl::ReviewStatus::Approved))
        } else {
            Ok(false)
        }
    }

    /// Get review response (for checking approval/rejection)
    pub async fn get_review_response(
        &self,
        review_id: &ReviewRequestId,
    ) -> HitlResult<Option<ReviewResponse>> {
        if let Some(review) = self.manager.get_review(review_id).await? {
            Ok(review.response)
        } else {
            Ok(None)
        }
    }
}
