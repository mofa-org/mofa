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

/// Audit Validation Policy (The Guard)
///
/// Automatically intercepts high-risk transactions and enforces cryptographic
/// security seal validation to eliminate unaudited ghost transactions.
#[derive(Debug, Clone)]
pub struct AuditValidationPolicy {
    /// Minimum risk level that triggers mandatory human-in-the-loop review.
    /// Default: `RiskLevel::High`.
    pub review_threshold: crate::hitl::context::RiskLevel,
    /// Whether to cryptographically verify the SHA-256 security seal against the transaction input.
    /// Default: `true`.
    pub require_valid_seal: bool,
    /// Whether transactions without `auditing_data` must be escalated for review.
    /// Default: `true`.
    pub require_auditing_data: bool,
}

impl Default for AuditValidationPolicy {
    fn default() -> Self {
        Self {
            review_threshold: crate::hitl::context::RiskLevel::High,
            require_valid_seal: true,
            require_auditing_data: true,
        }
    }
}

impl AuditValidationPolicy {
    /// Create a new audit validation policy with specified review threshold.
    pub fn new(review_threshold: crate::hitl::context::RiskLevel) -> Self {
        Self {
            review_threshold,
            ..Default::default()
        }
    }

    /// Builder to toggle requiring valid cryptographic seal.
    pub fn with_require_valid_seal(mut self, require: bool) -> Self {
        self.require_valid_seal = require;
        self
    }

    /// Builder to toggle requiring auditing data presence.
    pub fn with_require_auditing_data(mut self, require: bool) -> Self {
        self.require_auditing_data = require;
        self
    }
}

#[async_trait]
impl ReviewPolicy for AuditValidationPolicy {
    async fn should_request_review(
        &self,
        context: &ReviewContext,
    ) -> HitlResult<Option<ReviewRequest>> {
        let auditing = match &context.auditing_data {
            Some(data) => data,
            None => {
                if self.require_auditing_data {
                    let mut req = ReviewRequest::new(
                        "unknown",
                        ReviewType::Approval,
                        context.clone(),
                    );
                    req.metadata.custom.insert(
                        "reason".to_string(),
                        serde_json::Value::String(
                            "Unsealed transaction: missing mandatory AuditingData record"
                                .to_string(),
                        ),
                    );
                    return Ok(Some(req));
                } else {
                    return Ok(None);
                }
            }
        };

        // 1. Verify cryptographic seal integrity
        if self.require_valid_seal && !auditing.verify_seal(&context.input_data) {
            let mut req = ReviewRequest::new(
                "unknown",
                ReviewType::Approval,
                context.clone(),
            );
            req.metadata.custom.insert(
                "reason".to_string(),
                serde_json::Value::String(format!(
                    "Security seal verification failed for actor '{}': payload tampered or seal invalid",
                    auditing.actor
                )),
            );
            return Ok(Some(req));
        }

        // 2. Evaluate risk threshold
        if auditing.risk_level >= self.review_threshold {
            let mut req = ReviewRequest::new(
                "unknown",
                ReviewType::Approval,
                context.clone(),
            );
            req.metadata.custom.insert(
                "reason".to_string(),
                serde_json::Value::String(format!(
                    "High-risk transaction detected: [{}] actor '{}' action '{}' in domain '{}' (intent: {})",
                    auditing.risk_level, auditing.actor, auditing.action, auditing.domain, auditing.intent
                )),
            );
            return Ok(Some(req));
        }

        Ok(None)
    }

    async fn can_auto_approve(&self, request: &ReviewRequest) -> HitlResult<bool> {
        let auditing = match &request.context.auditing_data {
            Some(data) => data,
            None => return Ok(!self.require_auditing_data),
        };

        if self.require_valid_seal && !auditing.verify_seal(&request.context.input_data) {
            return Ok(false);
        }

        if auditing.risk_level >= self.review_threshold {
            return Ok(false);
        }

        Ok(true)
    }

    fn name(&self) -> &str {
        "AuditValidationPolicy"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hitl::context::{AuditingData, ExecutionTrace, ReviewContext, RiskLevel};
    use serde_json::json;

    fn test_context(auditing: Option<AuditingData>, payload: serde_json::Value) -> ReviewContext {
        let trace = ExecutionTrace {
            steps: Vec::new(),
            duration_ms: 10,
        };
        let mut ctx = ReviewContext::new(trace, payload);
        if let Some(a) = auditing {
            ctx = ctx.with_auditing_data(a);
        }
        ctx
    }

    #[tokio::test]
    async fn test_audit_validation_triggers_on_high_risk() {
        let payload = json!({"amount": 50_000, "token": "USDC"});
        let audit = AuditingData::new(
            "defi-agent",
            "liquidity_rebalance",
            "Move capital to pool A",
            "fintech",
            &payload,
        )
        .with_risk_level(RiskLevel::High);

        let ctx = test_context(Some(audit), payload);
        let policy = AuditValidationPolicy::new(RiskLevel::High);

        let review_req = policy.should_request_review(&ctx).await.unwrap();
        assert!(review_req.is_some());
        let req = review_req.unwrap();
        assert!(
            req.metadata.custom["reason"]
                .as_str()
                .unwrap()
                .contains("High-risk transaction detected")
        );
        assert!(!policy.can_auto_approve(&req).await.unwrap());
    }

    #[tokio::test]
    async fn test_audit_validation_passes_low_risk_verified() {
        let payload = json!({"query": "fetch_balance"});
        let audit = AuditingData::new(
            "read-agent",
            "query",
            "User balance check",
            "fintech",
            &payload,
        )
        .with_risk_level(RiskLevel::Low);

        let ctx = test_context(Some(audit), payload);
        let policy = AuditValidationPolicy::new(RiskLevel::High);

        let review_req = policy.should_request_review(&ctx).await.unwrap();
        assert!(review_req.is_none());
    }

    #[tokio::test]
    async fn test_audit_validation_detects_tampered_payload() {
        let original_payload = json!({"amount": 100});
        let audit = AuditingData::new(
            "transfer-agent",
            "send",
            "Disburse payment",
            "fintech",
            &original_payload,
        )
        .with_risk_level(RiskLevel::Low); // Even if low risk, tampering must trigger review!

        // Pass tampered payload into context
        let tampered_payload = json!({"amount": 10_000});
        let ctx = test_context(Some(audit), tampered_payload);

        let policy = AuditValidationPolicy::new(RiskLevel::High);
        let review_req = policy.should_request_review(&ctx).await.unwrap();
        assert!(review_req.is_some());
        let desc = review_req.unwrap().metadata.custom["reason"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(desc.contains("Security seal verification failed"));
    }

    #[tokio::test]
    async fn test_audit_validation_catches_unsealed_transactions() {
        let payload = json!({"action": "secret_update"});
        let ctx = test_context(None, payload); // No auditing data!

        let policy = AuditValidationPolicy::default(); // require_auditing_data = true
        let review_req = policy.should_request_review(&ctx).await.unwrap();
        assert!(review_req.is_some());
        let desc = review_req.unwrap().metadata.custom["reason"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(desc.contains("Unsealed transaction"));

        // If require_auditing_data is disabled, it passes
        let permissive_policy = policy.with_require_auditing_data(false);
        assert!(permissive_policy.should_request_review(&ctx).await.unwrap().is_none());
    }
}
