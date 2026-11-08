//! Review Context
//! Context information captured for review

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

/// Transaction risk classification level for security audit evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum RiskLevel {
    /// Routine, low-stakes operation (read-only, non-sensitive).
    #[default]
    Low = 0,
    /// Medium-stakes action (standard update, non-critical parameter changes).
    Medium = 1,
    /// High-stakes transaction (financial transfers, privilege elevation, critical state mutations).
    High = 2,
    /// Critical transaction (large fund movements, key rotations, irreversible bulk modifications).
    Critical = 3,
}

impl std::fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RiskLevel::Low => write!(f, "low"),
            RiskLevel::Medium => write!(f, "medium"),
            RiskLevel::High => write!(f, "high"),
            RiskLevel::Critical => write!(f, "critical"),
        }
    }
}

/// Standardized, immutable audit record ("Security Seal") capturing the "Who, When, Why, What"
/// of an automated or human agentic action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditingData {
    /// Identity of the acting agent or human principal ("Who").
    pub actor: String,
    /// Action or operation name ("What").
    pub action: String,
    /// Explicit human-readable rationale or business intent ("Why").
    pub intent: String,
    /// Execution timestamp in milliseconds since Unix epoch ("When").
    pub timestamp_ms: u64,
    /// Industry or business domain (e.g. "fintech", "ecommerce", "system", "healthcare").
    pub domain: String,
    /// Assessed transaction risk level.
    pub risk_level: RiskLevel,
    /// Cryptographic SHA-256 integrity seal calculated over canonical transaction metadata and payload.
    pub security_seal: String,
    /// Domain-specific auxiliary metadata.
    pub metadata: HashMap<String, serde_json::Value>,
}

impl AuditingData {
    /// Create a new `AuditingData` record with a generated cryptographic security seal.
    pub fn new(
        actor: impl Into<String>,
        action: impl Into<String>,
        intent: impl Into<String>,
        domain: impl Into<String>,
        payload: &serde_json::Value,
    ) -> Self {
        let actor = actor.into();
        let action = action.into();
        let intent = intent.into();
        let domain = domain.into();
        let timestamp_ms = chrono::Utc::now().timestamp_millis() as u64;

        let security_seal =
            Self::compute_seal(&actor, &action, &intent, timestamp_ms, &domain, payload);

        Self {
            actor,
            action,
            intent,
            timestamp_ms,
            domain,
            risk_level: RiskLevel::Low,
            security_seal,
            metadata: HashMap::new(),
        }
    }

    /// Compute a canonical SHA-256 seal for the given transaction parameters.
    pub fn compute_seal(
        actor: &str,
        action: &str,
        intent: &str,
        timestamp_ms: u64,
        domain: &str,
        payload: &serde_json::Value,
    ) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"MOFA_SECURITY_SEAL_V1:");
        hasher.update(actor.as_bytes());
        hasher.update(b"|");
        hasher.update(action.as_bytes());
        hasher.update(b"|");
        hasher.update(intent.as_bytes());
        hasher.update(b"|");
        hasher.update(timestamp_ms.to_string().as_bytes());
        hasher.update(b"|");
        hasher.update(domain.as_bytes());
        hasher.update(b"|");
        let payload_bytes = serde_json::to_vec(payload).unwrap_or_default();
        hasher.update(&payload_bytes);
        hex::encode(hasher.finalize())
    }

    /// Verify whether this audit record's security seal matches the provided payload.
    pub fn verify_seal(&self, payload: &serde_json::Value) -> bool {
        let computed = Self::compute_seal(
            &self.actor,
            &self.action,
            &self.intent,
            self.timestamp_ms,
            &self.domain,
            payload,
        );
        computed == self.security_seal
    }

    /// Set the risk level.
    pub fn with_risk_level(mut self, risk_level: RiskLevel) -> Self {
        self.risk_level = risk_level;
        self
    }

    /// Add a metadata key-value pair.
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }
}

/// Execution trace snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionTrace {
    /// Execution steps
    pub steps: Vec<ExecutionStep>,
    /// Total duration (milliseconds)
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStep {
    pub step_id: String,
    pub step_type: String,
    pub timestamp_ms: u64,
    pub input: Option<serde_json::Value>,
    pub output: Option<serde_json::Value>,
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Telemetry snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetrySnapshot {
    /// Metrics at time of review request
    pub metrics: HashMap<String, f64>,
    /// Logs (last N lines)
    pub recent_logs: Vec<String>,
    /// Performance data
    pub performance: Option<PerformanceData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceData {
    pub cpu_usage: f64,
    pub memory_usage_bytes: u64,
    pub latency_ms: u64,
}

/// Diff representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diff {
    pub diff_type: String, // "text", "json", "structured"
    pub before: serde_json::Value,
    pub after: serde_json::Value,
    pub changes: Vec<Change>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Change {
    pub path: String,
    pub change_type: String, // "added", "removed", "modified"
    pub old_value: Option<serde_json::Value>,
    pub new_value: Option<serde_json::Value>,
}

/// Review context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewContext {
    /// Execution trace
    pub execution_trace: ExecutionTrace,
    /// Input data
    pub input_data: serde_json::Value,
    /// Output data (if available)
    pub output_data: Option<serde_json::Value>,
    /// Diff (if applicable)
    pub diff: Option<Diff>,
    /// Telemetry snapshot
    pub telemetry: TelemetrySnapshot,
    /// Optional immutable auditing data ("Security Seal")
    pub auditing_data: Option<AuditingData>,
    /// Additional context
    pub additional: HashMap<String, serde_json::Value>,
}

impl ReviewContext {
    pub fn new(execution_trace: ExecutionTrace, input_data: serde_json::Value) -> Self {
        Self {
            execution_trace,
            input_data,
            output_data: None,
            diff: None,
            telemetry: TelemetrySnapshot {
                metrics: HashMap::new(),
                recent_logs: Vec::new(),
                performance: None,
            },
            auditing_data: None,
            additional: HashMap::new(),
        }
    }

    pub fn with_output(mut self, output: serde_json::Value) -> Self {
        self.output_data = Some(output);
        self
    }

    pub fn with_diff(mut self, diff: Diff) -> Self {
        self.diff = Some(diff);
        self
    }

    pub fn with_telemetry(mut self, telemetry: TelemetrySnapshot) -> Self {
        self.telemetry = telemetry;
        self
    }

    pub fn with_auditing_data(mut self, auditing_data: AuditingData) -> Self {
        self.auditing_data = Some(auditing_data);
        self
    }
}

/// Review metadata (re-exported from types for convenience)
pub use crate::hitl::types::ReviewMetadata;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_auditing_data_seal_generation_and_verification() {
        let payload = json!({
            "recipient": "0x742d35Cc6634C0532925a3b844Bc454e4438f44e",
            "amount_sol": 150.0,
            "currency": "SOL"
        });

        let audit = AuditingData::new(
            "agent-trader-1",
            "transfer",
            "Execute treasury rebalancing",
            "fintech",
            &payload,
        )
        .with_risk_level(RiskLevel::Critical)
        .with_metadata("tx_id", json!("0xabc123"));

        assert_eq!(audit.actor, "agent-trader-1");
        assert_eq!(audit.action, "transfer");
        assert_eq!(audit.risk_level, RiskLevel::Critical);
        assert!(!audit.security_seal.is_empty());

        // Verifying with exact payload must succeed
        assert!(audit.verify_seal(&payload));

        // Tampering with payload amount must invalidate seal
        let tampered_payload = json!({
            "recipient": "0x742d35Cc6634C0532925a3b844Bc454e4438f44e",
            "amount_sol": 999.0, // Tampered!
            "currency": "SOL"
        });
        assert!(!audit.verify_seal(&tampered_payload));
    }

    #[test]
    fn test_risk_level_ordering() {
        assert!(RiskLevel::Low < RiskLevel::Medium);
        assert!(RiskLevel::Medium < RiskLevel::High);
        assert!(RiskLevel::High < RiskLevel::Critical);
    }

    #[test]
    fn test_review_context_with_auditing_data() {
        let trace = ExecutionTrace {
            steps: Vec::new(),
            duration_ms: 42,
        };
        let payload = json!({"order_id": "ORD-999", "total": 1200.0});
        let audit = AuditingData::new(
            "checkout-bot",
            "place_order",
            "Customer VIP checkout",
            "ecommerce",
            &payload,
        )
        .with_risk_level(RiskLevel::Medium);

        let ctx = ReviewContext::new(trace, payload.clone()).with_auditing_data(audit.clone());

        assert!(ctx.auditing_data.is_some());
        let attached = ctx.auditing_data.unwrap();
        assert_eq!(attached.actor, "checkout-bot");
        assert_eq!(attached.domain, "ecommerce");
        assert!(attached.verify_seal(&payload));
    }
}
