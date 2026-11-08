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

#[cfg(test)]
mod tests {
    use super::*;
    use mofa_kernel::hitl::{
        AuditValidationPolicy, AuditingData, ExecutionTrace, ReviewContext, RiskLevel,
    };
    use serde_json::json;

    #[tokio::test]
    async fn test_policy_engine_with_audit_validation_policy() {
        let policy = Arc::new(AuditValidationPolicy::new(RiskLevel::High));
        let engine = ReviewPolicyEngine::new(vec![policy]);

        // 1. Critical risk transaction -> triggers review
        let payload = json!({"amount": 100_000, "action": "vault_drain"});
        let audit = AuditingData::new(
            "treasury-bot",
            "vault_drain",
            "Emergency reserve movement",
            "fintech",
            &payload,
        )
        .with_risk_level(RiskLevel::Critical);

        let trace = ExecutionTrace {
            steps: Vec::new(),
            duration_ms: 5,
        };
        let ctx = ReviewContext::new(trace, payload).with_auditing_data(audit);

        let req = engine.should_request_review(&ctx).await.unwrap();
        assert!(req.is_some());
        let request = req.unwrap();
        assert!(!engine.can_auto_approve(&request).await.unwrap());

        // 2. Low risk transaction -> should not request review
        let low_payload = json!({"status": "ping"});
        let low_audit = AuditingData::new(
            "health-bot",
            "ping",
            "Heartbeat check",
            "system",
            &low_payload,
        )
        .with_risk_level(RiskLevel::Low);

        let low_trace = ExecutionTrace {
            steps: Vec::new(),
            duration_ms: 1,
        };
        let low_ctx = ReviewContext::new(low_trace, low_payload).with_auditing_data(low_audit);
        assert!(engine.should_request_review(&low_ctx).await.unwrap().is_none());
    }
}
