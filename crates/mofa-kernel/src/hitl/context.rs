//! Review Context
//!
//! Context information captured for review and cryptographic auditing.
//!
//! # Security Guarantees: Drift Detection vs. Adversarial Tamper-Evidence
//!
//! MoFA provides two tiers of cryptographic integrity protection for Human-in-the-Loop actions:
//!
//! 1. **Accidental Drift Detection (Unkeyed SHA-256)**:
//!    - Produced by [`AuditingData::new`] or [`AuditingDataBuilder::seal_unkeyed`].
//!    - Computes a SHA-256 digest over the canonical transaction context, auxiliary metadata, and payload.
//!    - Protects against bit-rot, serialization drift, accidental mutations, or non-malicious payload corruption.
//!    - *Threat model limitation*: Because unkeyed SHA-256 is publicly computable, an adversary with write access
//!      to both the payload and the audit record can recompute a matching seal.
//!
//! 2. **Adversarial Tamper-Evidence (Keyed HMAC-SHA256 via `SealKeyProvider`)**:
//!    - Produced by [`AuditingData::new_keyed`] or [`AuditingDataBuilder::seal_keyed`].
//!    - Uses HMAC-SHA256 with an auditor-held secret key managed by a pluggable [`SealKeyProvider`].
//!    - Guarantees authenticity and cryptographic non-repudiation: an adversary modifying the actor, intent,
//!      metadata, or payload cannot forge a matching seal without possessing the auditor's private key.
//!    - All equality checks utilize constant-time comparison ([`subtle::ConstantTimeEq`]) to eliminate
//!      timing side-channel attacks.
//!
//! For enterprise immutability and multi-actor audit ledgers, individual `AuditingData` records can additionally
//! be chained sequentially into an append-only cryptographic log using previous seal references.

use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use subtle::ConstantTimeEq;

type HmacSha256 = Hmac<Sha256>;

/// Canonical prefix used for unkeyed SHA-256 security seals (drift detection).
pub const SEAL_PREFIX_UNKEYED_V1: &str = "MOFA_SECURITY_SEAL_V1:";

/// Canonical prefix used for keyed HMAC-SHA256 security seals (adversarial tamper-evidence).
pub const SEAL_PREFIX_HMAC_V1: &str = "MOFA_SECURITY_SEAL_HMAC_V1:";

/// Compare two hex-encoded security seals in constant time to prevent timing attacks.
pub fn constant_time_eq_hex(a: &str, b: &str) -> bool {
    let clean_a = a.strip_prefix("hmac-sha256:").unwrap_or(a);
    let clean_b = b.strip_prefix("hmac-sha256:").unwrap_or(b);

    let Ok(a_bytes) = hex::decode(clean_a) else {
        return false;
    };
    let Ok(b_bytes) = hex::decode(clean_b) else {
        return false;
    };

    if a_bytes.len() != b_bytes.len() || a_bytes.is_empty() {
        return false;
    }

    a_bytes.ct_eq(&b_bytes).into()
}

/// Pluggable trait for providing secret cryptographic keys for signing and verifying
/// security seals (HMAC-SHA256).
pub trait SealKeyProvider: Send + Sync {
    /// Retrieve the secret key for the given domain and actor.
    ///
    /// Returns `Some(key)` if a key is authorized and available, or `None` if unconfigured.
    fn get_seal_key(&self, domain: &str, actor: &str) -> Option<Vec<u8>>;
}

/// Static key provider holding a fixed secret key for all domains and actors.
#[derive(Debug, Clone)]
pub struct StaticKeyProvider {
    key: Vec<u8>,
}

impl StaticKeyProvider {
    /// Create a static key provider from raw key bytes.
    pub fn new(key: impl Into<Vec<u8>>) -> Self {
        Self { key: key.into() }
    }

    /// Attempt to load a static key provider from an environment variable.
    pub fn from_env(var_name: &str) -> Option<Self> {
        std::env::var(var_name)
            .ok()
            .map(|val| Self::new(val.into_bytes()))
    }
}

impl SealKeyProvider for StaticKeyProvider {
    fn get_seal_key(&self, _domain: &str, _actor: &str) -> Option<Vec<u8>> {
        Some(self.key.clone())
    }
}

/// An in-memory keyring that allows configuring secret keys per domain, per actor,
/// or with a fallback default key.
#[derive(Debug, Clone, Default)]
pub struct InMemoryKeyRing {
    domain_keys: HashMap<String, Vec<u8>>,
    actor_keys: HashMap<(String, String), Vec<u8>>,
    default_key: Option<Vec<u8>>,
}

impl InMemoryKeyRing {
    /// Create a new empty keyring.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a default fallback key.
    pub fn with_default_key(mut self, key: impl Into<Vec<u8>>) -> Self {
        self.default_key = Some(key.into());
        self
    }

    /// Register a key for an entire domain (e.g. "fintech", "ecommerce").
    pub fn with_domain_key(mut self, domain: impl Into<String>, key: impl Into<Vec<u8>>) -> Self {
        self.domain_keys.insert(domain.into(), key.into());
        self
    }

    /// Register a specific key for an actor within a domain.
    pub fn with_actor_key(
        mut self,
        domain: impl Into<String>,
        actor: impl Into<String>,
        key: impl Into<Vec<u8>>,
    ) -> Self {
        self.actor_keys
            .insert((domain.into(), actor.into()), key.into());
        self
    }
}

impl SealKeyProvider for InMemoryKeyRing {
    fn get_seal_key(&self, domain: &str, actor: &str) -> Option<Vec<u8>> {
        if let Some(key) = self
            .actor_keys
            .get(&(domain.to_string(), actor.to_string()))
        {
            return Some(key.clone());
        }
        if let Some(key) = self.domain_keys.get(domain) {
            return Some(key.clone());
        }
        self.default_key.clone()
    }
}

impl<F> SealKeyProvider for F
where
    F: Fn(&str, &str) -> Option<Vec<u8>> + Send + Sync,
{
    fn get_seal_key(&self, domain: &str, actor: &str) -> Option<Vec<u8>> {
        (self)(domain, actor)
    }
}

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
    /// Cryptographic SHA-256 or HMAC-SHA256 integrity seal calculated over canonical transaction metadata and payload.
    pub security_seal: String,
    /// Domain-specific auxiliary metadata covered by the security seal.
    pub metadata: HashMap<String, serde_json::Value>,
}

impl AuditingData {
    /// Create a new `AuditingData` record with an unkeyed SHA-256 seal (drift detection).
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

    /// Create a new `AuditingData` record with an initial metadata map and an unkeyed SHA-256 seal.
    pub fn new_with_metadata(
        actor: impl Into<String>,
        action: impl Into<String>,
        intent: impl Into<String>,
        domain: impl Into<String>,
        metadata: HashMap<String, serde_json::Value>,
        payload: &serde_json::Value,
    ) -> Self {
        let actor = actor.into();
        let action = action.into();
        let intent = intent.into();
        let domain = domain.into();
        let timestamp_ms = chrono::Utc::now().timestamp_millis() as u64;

        let security_seal = Self::compute_seal_with_metadata(
            &actor,
            &action,
            &intent,
            timestamp_ms,
            &domain,
            &metadata,
            payload,
        );

        Self {
            actor,
            action,
            intent,
            timestamp_ms,
            domain,
            risk_level: RiskLevel::Low,
            security_seal,
            metadata,
        }
    }

    /// Create a new `AuditingData` record with a keyed HMAC-SHA256 seal (adversarial tamper-evidence).
    pub fn new_keyed(
        actor: impl Into<String>,
        action: impl Into<String>,
        intent: impl Into<String>,
        domain: impl Into<String>,
        key: &[u8],
        payload: &serde_json::Value,
    ) -> Self {
        Self::new_keyed_with_metadata(actor, action, intent, domain, HashMap::new(), key, payload)
    }

    /// Create a new `AuditingData` record with metadata and a keyed HMAC-SHA256 seal.
    pub fn new_keyed_with_metadata(
        actor: impl Into<String>,
        action: impl Into<String>,
        intent: impl Into<String>,
        domain: impl Into<String>,
        metadata: HashMap<String, serde_json::Value>,
        key: &[u8],
        payload: &serde_json::Value,
    ) -> Self {
        let actor = actor.into();
        let action = action.into();
        let intent = intent.into();
        let domain = domain.into();
        let timestamp_ms = chrono::Utc::now().timestamp_millis() as u64;

        let security_seal = Self::compute_seal_hmac(
            key,
            &actor,
            &action,
            &intent,
            timestamp_ms,
            &domain,
            &metadata,
            payload,
        );

        Self {
            actor,
            action,
            intent,
            timestamp_ms,
            domain,
            risk_level: RiskLevel::Low,
            security_seal,
            metadata,
        }
    }

    /// Create a builder to construct and seal an `AuditingData` audit record.
    pub fn builder(
        actor: impl Into<String>,
        action: impl Into<String>,
        intent: impl Into<String>,
        domain: impl Into<String>,
    ) -> AuditingDataBuilder {
        AuditingDataBuilder::new(actor, action, intent, domain)
    }

    /// Returns `true` if this audit record was generated using a keyed HMAC-SHA256 algorithm.
    pub fn is_keyed(&self) -> bool {
        self.security_seal.starts_with("hmac-sha256:")
    }

    /// Compute canonical canonical metadata bytes into a byte vector for deterministic hashing.
    fn canonicalize_metadata(metadata: &HashMap<String, serde_json::Value>) -> Vec<u8> {
        let mut sorted_keys: Vec<&String> = metadata.keys().collect();
        sorted_keys.sort();

        let mut buf = Vec::new();
        for key in sorted_keys {
            buf.extend_from_slice(key.as_bytes());
            buf.push(b'=');
            let val_bytes = serde_json::to_vec(&metadata[key]).unwrap_or_default();
            buf.extend_from_slice(&val_bytes);
            buf.push(b';');
        }
        buf
    }

    /// Compute a canonical unkeyed SHA-256 seal for transaction parameters (without auxiliary metadata).
    pub fn compute_seal(
        actor: &str,
        action: &str,
        intent: &str,
        timestamp_ms: u64,
        domain: &str,
        payload: &serde_json::Value,
    ) -> String {
        Self::compute_seal_with_metadata(
            actor,
            action,
            intent,
            timestamp_ms,
            domain,
            &HashMap::new(),
            payload,
        )
    }

    /// Compute a canonical unkeyed SHA-256 seal covering transaction parameters and auxiliary metadata.
    pub fn compute_seal_with_metadata(
        actor: &str,
        action: &str,
        intent: &str,
        timestamp_ms: u64,
        domain: &str,
        metadata: &HashMap<String, serde_json::Value>,
        payload: &serde_json::Value,
    ) -> String {
        let mut hasher = Sha256::new();
        hasher.update(SEAL_PREFIX_UNKEYED_V1.as_bytes());
        hasher.update(actor.as_bytes());
        hasher.update(b"|");
        hasher.update(action.as_bytes());
        hasher.update(b"|");
        hasher.update(intent.as_bytes());
        hasher.update(b"|");
        hasher.update(timestamp_ms.to_string().as_bytes());
        hasher.update(b"|");
        hasher.update(domain.as_bytes());
        hasher.update(b"|meta:");
        hasher.update(Self::canonicalize_metadata(metadata));
        hasher.update(b"|payload:");
        let payload_bytes = serde_json::to_vec(payload).unwrap_or_default();
        hasher.update(&payload_bytes);
        hex::encode(hasher.finalize())
    }

    /// Compute a keyed HMAC-SHA256 seal covering transaction parameters, metadata, and payload.
    #[allow(clippy::too_many_arguments)]
    pub fn compute_seal_hmac(
        key: &[u8],
        actor: &str,
        action: &str,
        intent: &str,
        timestamp_ms: u64,
        domain: &str,
        metadata: &HashMap<String, serde_json::Value>,
        payload: &serde_json::Value,
    ) -> String {
        let mut mac =
            HmacSha256::new_from_slice(key).expect("HMAC-SHA256 accepts keys of any byte length");

        mac.update(SEAL_PREFIX_HMAC_V1.as_bytes());
        mac.update(actor.as_bytes());
        mac.update(b"|");
        mac.update(action.as_bytes());
        mac.update(b"|");
        mac.update(intent.as_bytes());
        mac.update(b"|");
        mac.update(timestamp_ms.to_string().as_bytes());
        mac.update(b"|");
        mac.update(domain.as_bytes());
        mac.update(b"|meta:");
        mac.update(&Self::canonicalize_metadata(metadata));
        mac.update(b"|payload:");
        let payload_bytes = serde_json::to_vec(payload).unwrap_or_default();
        mac.update(&payload_bytes);

        format!("hmac-sha256:{}", hex::encode(mac.finalize().into_bytes()))
    }

    /// Verify whether this audit record's security seal matches the provided payload.
    ///
    /// For keyed seals, use [`Self::verify_seal_keyed`] or [`Self::verify_seal_with_provider`].
    /// Uses constant-time comparison to prevent timing side-channels.
    pub fn verify_seal(&self, payload: &serde_json::Value) -> bool {
        // 1. Check with current metadata
        let computed_with_meta = Self::compute_seal_with_metadata(
            &self.actor,
            &self.action,
            &self.intent,
            self.timestamp_ms,
            &self.domain,
            &self.metadata,
            payload,
        );
        if constant_time_eq_hex(&self.security_seal, &computed_with_meta) {
            return true;
        }

        // 2. Backward-compatibility fallback for unkeyed seals computed prior to metadata attachment
        let empty_meta = HashMap::new();
        let computed_legacy = Self::compute_seal_with_metadata(
            &self.actor,
            &self.action,
            &self.intent,
            self.timestamp_ms,
            &self.domain,
            &empty_meta,
            payload,
        );
        constant_time_eq_hex(&self.security_seal, &computed_legacy)
    }

    /// Verify whether this audit record's security seal matches the provided payload
    /// using an auditor secret key (HMAC-SHA256).
    ///
    /// Strictly verifies that both the payload and the auxiliary metadata have not been altered.
    /// Uses constant-time equality comparison to prevent timing attacks.
    pub fn verify_seal_keyed(&self, key: &[u8], payload: &serde_json::Value) -> bool {
        let computed = Self::compute_seal_hmac(
            key,
            &self.actor,
            &self.action,
            &self.intent,
            self.timestamp_ms,
            &self.domain,
            &self.metadata,
            payload,
        );
        constant_time_eq_hex(&self.security_seal, &computed)
    }

    /// Verify this audit record's security seal against a pluggable [`SealKeyProvider`].
    pub fn verify_seal_with_provider(
        &self,
        provider: &dyn SealKeyProvider,
        payload: &serde_json::Value,
    ) -> bool {
        let Some(key) = provider.get_seal_key(&self.domain, &self.actor) else {
            return false;
        };
        self.verify_seal_keyed(&key, payload)
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

    /// Reseal this audit record with the updated metadata and payload using the unkeyed algorithm.
    pub fn reseal(&mut self, payload: &serde_json::Value) {
        self.security_seal = Self::compute_seal_with_metadata(
            &self.actor,
            &self.action,
            &self.intent,
            self.timestamp_ms,
            &self.domain,
            &self.metadata,
            payload,
        );
    }

    /// Reseal this audit record with the updated metadata and payload using a secret key (HMAC-SHA256).
    pub fn reseal_keyed(&mut self, key: &[u8], payload: &serde_json::Value) {
        self.security_seal = Self::compute_seal_hmac(
            key,
            &self.actor,
            &self.action,
            &self.intent,
            self.timestamp_ms,
            &self.domain,
            &self.metadata,
            payload,
        );
    }

    /// Add a metadata key-value pair and recompute the unkeyed seal covering the new metadata.
    pub fn with_resealed_metadata(
        mut self,
        key: impl Into<String>,
        value: serde_json::Value,
        payload: &serde_json::Value,
    ) -> Self {
        self.metadata.insert(key.into(), value);
        self.reseal(payload);
        self
    }

    /// Add a metadata key-value pair and recompute the keyed HMAC seal covering the new metadata.
    pub fn with_resealed_keyed_metadata(
        mut self,
        key: impl Into<String>,
        value: serde_json::Value,
        seal_key: &[u8],
        payload: &serde_json::Value,
    ) -> Self {
        self.metadata.insert(key.into(), value);
        self.reseal_keyed(seal_key, payload);
        self
    }
}

/// Builder for constructing and sealing an [`AuditingData`] audit record.
#[derive(Debug, Clone)]
pub struct AuditingDataBuilder {
    actor: String,
    action: String,
    intent: String,
    domain: String,
    timestamp_ms: u64,
    risk_level: RiskLevel,
    metadata: HashMap<String, serde_json::Value>,
}

impl AuditingDataBuilder {
    /// Create a new builder for an audit record.
    pub fn new(
        actor: impl Into<String>,
        action: impl Into<String>,
        intent: impl Into<String>,
        domain: impl Into<String>,
    ) -> Self {
        Self {
            actor: actor.into(),
            action: action.into(),
            intent: intent.into(),
            domain: domain.into(),
            timestamp_ms: chrono::Utc::now().timestamp_millis() as u64,
            risk_level: RiskLevel::Low,
            metadata: HashMap::new(),
        }
    }

    /// Explicitly override the timestamp.
    pub fn with_timestamp_ms(mut self, timestamp_ms: u64) -> Self {
        self.timestamp_ms = timestamp_ms;
        self
    }

    /// Set the risk level.
    pub fn with_risk_level(mut self, risk_level: RiskLevel) -> Self {
        self.risk_level = risk_level;
        self
    }

    /// Add a metadata key-value pair to be covered by the seal.
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Merge a map of metadata pairs to be covered by the seal.
    pub fn with_metadata_map(mut self, metadata: HashMap<String, serde_json::Value>) -> Self {
        self.metadata.extend(metadata);
        self
    }

    /// Seal this audit record using an unkeyed SHA-256 digest (drift detection).
    pub fn seal_unkeyed(self, payload: &serde_json::Value) -> AuditingData {
        let security_seal = AuditingData::compute_seal_with_metadata(
            &self.actor,
            &self.action,
            &self.intent,
            self.timestamp_ms,
            &self.domain,
            &self.metadata,
            payload,
        );
        AuditingData {
            actor: self.actor,
            action: self.action,
            intent: self.intent,
            timestamp_ms: self.timestamp_ms,
            domain: self.domain,
            risk_level: self.risk_level,
            security_seal,
            metadata: self.metadata,
        }
    }

    /// Seal this audit record using HMAC-SHA256 with an auditor secret key (adversarial tamper-evidence).
    pub fn seal_keyed(self, key: &[u8], payload: &serde_json::Value) -> AuditingData {
        let security_seal = AuditingData::compute_seal_hmac(
            key,
            &self.actor,
            &self.action,
            &self.intent,
            self.timestamp_ms,
            &self.domain,
            &self.metadata,
            payload,
        );
        AuditingData {
            actor: self.actor,
            action: self.action,
            intent: self.intent,
            timestamp_ms: self.timestamp_ms,
            domain: self.domain,
            risk_level: self.risk_level,
            security_seal,
            metadata: self.metadata,
        }
    }

    /// Seal this audit record by retrieving the appropriate key from a [`SealKeyProvider`].
    ///
    /// Returns `None` if the provider has no configured key for this domain and actor.
    pub fn seal_with_provider(
        self,
        provider: &dyn SealKeyProvider,
        payload: &serde_json::Value,
    ) -> Option<AuditingData> {
        let key = provider.get_seal_key(&self.domain, &self.actor)?;
        Some(self.seal_keyed(&key, payload))
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

    #[test]
    fn test_keyed_hmac_seal_generation_and_verification() {
        let key = b"super-secret-auditor-signing-key";
        let payload = json!({
            "treasury_id": "TREASURY-01",
            "allocated_usd": 500_000.0,
            "currency": "USD"
        });

        let audit = AuditingData::builder(
            "defi-rebalancer",
            "reallocate_funds",
            "Quarterly reserve rebalancing",
            "fintech",
        )
        .with_risk_level(RiskLevel::Critical)
        .with_metadata("approver_id", json!("lead-finance-ops"))
        .with_metadata("ticket_number", json!("FIN-8821"))
        .seal_keyed(key, &payload);

        assert!(audit.is_keyed());
        assert!(audit.security_seal.starts_with("hmac-sha256:"));

        // 1. Valid key and payload must verify
        assert!(audit.verify_seal_keyed(key, &payload));

        // 2. Incorrect key must be rejected
        let wrong_key = b"malicious-attacker-signing-key";
        assert!(!audit.verify_seal_keyed(wrong_key, &payload));

        // 3. Tampered payload must be rejected
        let tampered_payload = json!({
            "treasury_id": "TREASURY-01",
            "allocated_usd": 999_999.0, // Modified!
            "currency": "USD"
        });
        assert!(!audit.verify_seal_keyed(key, &tampered_payload));

        // 4. Tampering with covered metadata must invalidate the seal
        let mut tampered_audit = audit.clone();
        tampered_audit
            .metadata
            .insert("ticket_number".to_string(), json!("FIN-9999"));
        assert!(!tampered_audit.verify_seal_keyed(key, &payload));
    }

    #[test]
    fn test_seal_key_providers_static_and_in_memory() {
        let payload = json!({"sensor_id": "TEMP-42", "reading": 78.4});

        // 1. Static key provider
        let static_provider = StaticKeyProvider::new(b"global-static-secret-key");
        let audit = AuditingData::builder("iot-gateway", "telemetry", "Log reading", "system")
            .with_metadata("node", json!("us-east-1"))
            .seal_with_provider(&static_provider, &payload)
            .expect("Static key provider must always provide key");

        assert!(audit.is_keyed());
        assert!(audit.verify_seal_with_provider(&static_provider, &payload));

        // 2. InMemoryKeyRing with domain and actor segregation
        let keyring = InMemoryKeyRing::new()
            .with_default_key(b"fallback-key")
            .with_domain_key("fintech", b"fintech-auditor-key")
            .with_actor_key("fintech", "vip-trader", b"trader-specific-key");

        // Specific actor key
        let trader_audit = AuditingData::builder(
            "vip-trader",
            "margin_trade",
            "High leverage order",
            "fintech",
        )
        .seal_with_provider(&keyring, &payload)
        .unwrap();

        assert!(trader_audit.verify_seal_keyed(b"trader-specific-key", &payload));
        assert!(trader_audit.verify_seal_with_provider(&keyring, &payload));
        // Must reject wrong domain key
        assert!(!trader_audit.verify_seal_keyed(b"fintech-auditor-key", &payload));

        // Domain-fallback key
        let general_audit = AuditingData::builder(
            "general-agent",
            "fee_collection",
            "Collect standard fee",
            "fintech",
        )
        .seal_with_provider(&keyring, &payload)
        .unwrap();
        assert!(general_audit.verify_seal_keyed(b"fintech-auditor-key", &payload));
        assert!(general_audit.verify_seal_with_provider(&keyring, &payload));

        // Unknown domain fallback to default key
        let unknown_audit =
            AuditingData::builder("unknown-agent", "ping", "Healthcheck", "analytics")
                .seal_with_provider(&keyring, &payload)
                .unwrap();
        assert!(unknown_audit.verify_seal_keyed(b"fallback-key", &payload));
    }

    #[test]
    fn test_closure_key_provider() {
        let payload = json!({"action": "deploy_smart_contract"});
        let closure_provider = |domain: &str, _actor: &str| -> Option<Vec<u8>> {
            if domain == "web3" {
                Some(b"web3-validator-secret".to_vec())
            } else {
                None
            }
        };

        let audit = AuditingData::builder("contract-deployer", "deploy", "Deploy v2", "web3")
            .seal_with_provider(&closure_provider, &payload)
            .expect("Should seal for web3 domain");

        assert!(audit.verify_seal_with_provider(&closure_provider, &payload));

        // Non-matching domain should return None
        let rejected = AuditingData::builder("some-agent", "action", "intent", "healthcare")
            .seal_with_provider(&closure_provider, &payload);
        assert!(rejected.is_none());
    }

    #[test]
    fn test_unkeyed_builder_metadata_tampering_fails() {
        let payload = json!({"amount": 100});
        let audit = AuditingData::builder("bot", "transfer", "Payment", "fintech")
            .with_metadata("destination", json!("vault-1"))
            .seal_unkeyed(&payload);

        assert!(!audit.is_keyed());
        assert!(audit.verify_seal(&payload));

        // Tampering with metadata must fail unkeyed verification
        let mut tampered = audit.clone();
        tampered
            .metadata
            .insert("destination".to_string(), json!("hacker-vault"));
        assert!(!tampered.verify_seal(&payload));
    }

    #[test]
    fn test_reseal_helpers() {
        let key = b"reseal-secret";
        let payload = json!({"field": "val"});
        let mut audit = AuditingData::new_keyed("actor", "act", "intent", "dom", key, &payload);
        assert!(audit.verify_seal_keyed(key, &payload));

        // Add metadata without reseal -> verification fails
        audit
            .metadata
            .insert("new_key".to_string(), json!("new_val"));
        assert!(!audit.verify_seal_keyed(key, &payload));

        // Reseal keyed -> verification passes again
        audit.reseal_keyed(key, &payload);
        assert!(audit.verify_seal_keyed(key, &payload));

        // Fluent with_resealed_keyed_metadata
        let audit2 = audit.with_resealed_keyed_metadata("k2", json!("v2"), key, &payload);
        assert!(audit2.verify_seal_keyed(key, &payload));
    }

    #[test]
    fn test_constant_time_comparison_behavior() {
        assert!(constant_time_eq_hex("aabbcc", "aabbcc"));
        assert!(constant_time_eq_hex("hmac-sha256:aabbcc", "aabbcc"));
        assert!(!constant_time_eq_hex("aabbcc", "aabbcd"));
        assert!(!constant_time_eq_hex("aabb", "aabbcc"));
        assert!(!constant_time_eq_hex("invalid-hex", "invalid-hex"));
    }
}
