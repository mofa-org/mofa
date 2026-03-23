//! HITL testing harness built on top of the real MoFA review manager.
//!
//! This module provides a scripted reviewer for deterministic approval-flow
//! tests without re-implementing review orchestration logic.

use mofa_foundation::hitl::{
    FoundationHitlError, InMemoryReviewStore, ReviewManager, ReviewManagerConfig, ReviewNotifier,
    ReviewPolicyEngine, ReviewStore, ToolReviewHandler, WorkflowReviewHandler,
};
use mofa_kernel::hitl::{
    ReviewContext, ReviewRequest, ReviewRequestId, ReviewResponse, ReviewStatus,
};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::time::sleep;

/// Deterministic scripted decision returned by a simulated reviewer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScriptedDecision {
    Approve { comment: Option<String> },
    Reject {
        reason: String,
        comment: Option<String>,
    },
    RequestChanges {
        changes: String,
        comment: Option<String>,
    },
    Defer { reason: String },
    Timeout,
}

impl ScriptedDecision {
    fn to_review_response(&self) -> Option<ReviewResponse> {
        match self {
            Self::Approve { comment } => Some(ReviewResponse::Approved {
                comment: comment.clone(),
            }),
            Self::Reject { reason, comment } => Some(ReviewResponse::Rejected {
                reason: reason.clone(),
                comment: comment.clone(),
            }),
            Self::RequestChanges { changes, comment } => Some(ReviewResponse::ChangesRequested {
                changes: changes.clone(),
                comment: comment.clone(),
            }),
            Self::Defer { reason } => Some(ReviewResponse::Deferred {
                reason: reason.clone(),
            }),
            Self::Timeout => None,
        }
    }
}

/// Queue-backed simulated reviewer for deterministic HITL tests.
#[derive(Debug, Clone)]
pub struct ScriptedReviewer {
    decisions: Arc<Mutex<VecDeque<ScriptedDecision>>>,
    default_decision: Arc<Mutex<ScriptedDecision>>,
    reviewer_id: Arc<str>,
}

impl Default for ScriptedReviewer {
    fn default() -> Self {
        Self::new("hitl-test-reviewer")
    }
}

impl ScriptedReviewer {
    pub fn new(reviewer_id: impl Into<String>) -> Self {
        Self {
            decisions: Arc::new(Mutex::new(VecDeque::new())),
            default_decision: Arc::new(Mutex::new(ScriptedDecision::Approve { comment: None })),
            reviewer_id: reviewer_id.into().into(),
        }
    }

    pub fn with_default_decision(mut self, decision: ScriptedDecision) -> Self {
        self.default_decision = Arc::new(Mutex::new(decision));
        self
    }

    pub fn set_default_decision(&self, decision: ScriptedDecision) {
        *self
            .default_decision
            .lock()
            .expect("scripted reviewer default decision lock poisoned") = decision;
    }

    pub fn push_decision(&self, decision: ScriptedDecision) {
        self.decisions
            .lock()
            .expect("scripted reviewer queue lock poisoned")
            .push_back(decision);
    }

    pub fn pending_decisions(&self) -> usize {
        self.decisions
            .lock()
            .expect("scripted reviewer queue lock poisoned")
            .len()
    }

    fn next_decision(&self) -> ScriptedDecision {
        self.decisions
            .lock()
            .expect("scripted reviewer queue lock poisoned")
            .pop_front()
            .unwrap_or_else(|| {
                self.default_decision
                    .lock()
                    .expect("scripted reviewer default decision lock poisoned")
                    .clone()
            })
    }

    pub fn reviewer_id(&self) -> &str {
        &self.reviewer_id
    }
}

/// Test harness for deterministic HITL review scenarios.
pub struct HitlTestHarness {
    store: Arc<InMemoryReviewStore>,
    manager: Arc<ReviewManager>,
    workflow_handler: WorkflowReviewHandler,
    tool_handler: ToolReviewHandler,
    reviewer: ScriptedReviewer,
}

impl Default for HitlTestHarness {
    fn default() -> Self {
        Self::new()
    }
}

impl HitlTestHarness {
    /// Build a harness with in-memory review storage and no external side effects.
    pub fn new() -> Self {
        Self::with_config(ReviewManagerConfig {
            default_expiration: Duration::from_secs(30),
            expiration_check_interval: Duration::from_secs(1),
            enable_rate_limiting: false,
        })
    }

    pub fn with_config(config: ReviewManagerConfig) -> Self {
        let store = Arc::new(InMemoryReviewStore::new());
        let manager = Arc::new(ReviewManager::new(
            store.clone() as Arc<dyn ReviewStore>,
            Arc::new(ReviewNotifier::default()),
            Arc::new(ReviewPolicyEngine::default()),
            None,
            config,
        ));
        let workflow_handler = WorkflowReviewHandler::new(manager.clone());
        let tool_handler = ToolReviewHandler::new(manager.clone());

        Self {
            store,
            manager,
            workflow_handler,
            tool_handler,
            reviewer: ScriptedReviewer::default(),
        }
    }

    pub fn reviewer(&self) -> &ScriptedReviewer {
        &self.reviewer
    }

    pub async fn request_workflow_review(
        &self,
        execution_id: &str,
        node_id: &str,
        context: ReviewContext,
    ) -> Result<ReviewRequestId, FoundationHitlError> {
        self.workflow_handler
            .request_node_review(execution_id, node_id, context)
            .await
    }

    pub async fn request_tool_call_review(
        &self,
        execution_id: &str,
        tool_name: &str,
        tool_args: serde_json::Value,
        context: ReviewContext,
    ) -> Result<ReviewRequestId, FoundationHitlError> {
        self.tool_handler
            .request_tool_call_review(execution_id, tool_name, tool_args, context)
            .await
    }

    pub async fn request_tool_output_review(
        &self,
        execution_id: &str,
        tool_name: &str,
        tool_output: serde_json::Value,
        context: ReviewContext,
    ) -> Result<ReviewRequestId, FoundationHitlError> {
        self.tool_handler
            .request_tool_output_review(execution_id, tool_name, tool_output, context)
            .await
    }

    /// Apply the next scripted reviewer decision to an existing review.
    pub async fn resolve_with_script(
        &self,
        review_id: &ReviewRequestId,
    ) -> Result<ScriptedDecision, FoundationHitlError> {
        let decision = self.reviewer.next_decision();

        if let Some(response) = decision.to_review_response() {
            self.manager
                .resolve_review(
                    review_id,
                    response,
                    self.reviewer.reviewer_id().to_string(),
                )
                .await?;
        }

        Ok(decision)
    }

    pub async fn wait_for_review(
        &self,
        review_id: &ReviewRequestId,
        timeout: Duration,
    ) -> Result<ReviewResponse, FoundationHitlError> {
        let start = std::time::Instant::now();
        let check_interval = Duration::from_millis(25);

        loop {
            if start.elapsed() > timeout {
                return Err(FoundationHitlError::InvalidConfig(format!(
                    "Review {} timed out",
                    review_id.as_str()
                )));
            }

            if let Some(review) = self.manager.get_review(review_id).await? {
                if review.is_expired() {
                    return Err(FoundationHitlError::InvalidConfig(format!(
                        "Review {} expired",
                        review_id.as_str()
                    )));
                }

                if let Some(response) = review.response {
                    match response {
                        ReviewResponse::Deferred { .. } => {}
                        terminal => return Ok(terminal),
                    }
                }
            }

            sleep(check_interval).await;
        }
    }

    pub async fn get_review(
        &self,
        review_id: &ReviewRequestId,
    ) -> Result<Option<ReviewRequest>, FoundationHitlError> {
        self.manager.get_review(review_id).await
    }

    pub async fn reviews_for_execution(
        &self,
        execution_id: &str,
    ) -> Result<Vec<ReviewRequest>, FoundationHitlError> {
        self.store
            .list_by_execution(execution_id)
            .await
            .map_err(FoundationHitlError::Store)
    }

    pub async fn review_status(
        &self,
        review_id: &ReviewRequestId,
    ) -> Result<Option<ReviewStatus>, FoundationHitlError> {
        Ok(self.get_review(review_id).await?.map(|review| review.status))
    }

    pub async fn is_approved(
        &self,
        review_id: &ReviewRequestId,
    ) -> Result<bool, FoundationHitlError> {
        self.workflow_handler.is_approved(review_id).await
    }
}
