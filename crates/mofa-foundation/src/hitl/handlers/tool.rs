//! Tool Review Handler
//!
//! Integration handler for tool execution review requests

use crate::hitl::error::HitlResult;
use crate::hitl::manager::ReviewManager;
use mofa_kernel::hitl::{
    ReviewContext, ReviewRequest, ReviewRequestId, ReviewResponse, ReviewType,
};
use std::sync::Arc;

/// Handler for tool execution review integration
pub struct ToolReviewHandler {
    manager: Arc<ReviewManager>,
}

impl ToolReviewHandler {
    /// Create a new tool review handler
    pub fn new(manager: Arc<ReviewManager>) -> Self {
        Self { manager }
    }

    /// Request a review for a tool call (before execution)
    pub async fn request_tool_call_review(
        &self,
        execution_id: &str,
        tool_name: &str,
        tool_args: serde_json::Value,
        context: ReviewContext,
    ) -> HitlResult<ReviewRequestId> {
        let mut request = ReviewRequest::new(execution_id, ReviewType::Approval, context)
            .with_node_id(format!("tool:{}", tool_name));

        // Add tool-specific metadata
        request.metadata.tags.push("tool_execution".to_string());
        request.metadata.custom.insert(
            "tool_name".to_string(),
            serde_json::Value::String(tool_name.to_string()),
        );
        request
            .metadata
            .custom
            .insert("tool_args".to_string(), tool_args);

        self.manager.request_review(request).await
    }

    /// Request a review for tool output (after execution)
    pub async fn request_tool_output_review(
        &self,
        execution_id: &str,
        tool_name: &str,
        tool_output: serde_json::Value,
        context: ReviewContext,
    ) -> HitlResult<ReviewRequestId> {
        let mut request = ReviewRequest::new(execution_id, ReviewType::Feedback, context)
            .with_node_id(format!("tool_output:{}", tool_name));

        // Add tool-specific metadata
        request.metadata.tags.push("tool_output".to_string());
        request.metadata.custom.insert(
            "tool_name".to_string(),
            serde_json::Value::String(tool_name.to_string()),
        );
        request
            .metadata
            .custom
            .insert("tool_output".to_string(), tool_output);

        self.manager.request_review(request).await
    }

    /// Wait for a review to be resolved
    pub async fn wait_for_review(&self, review_id: &ReviewRequestId) -> HitlResult<ReviewResponse> {
        self.manager.wait_for_review(review_id, None).await
    }
}
