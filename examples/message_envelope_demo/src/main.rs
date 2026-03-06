//! This example demonstrates the advanced messaging features introduced 
//! in the `mofa-kernel`'s `llm::types` module, including:
//! - Creating and manipulating `MessageEnvelope`s.
//! - Setting and propagating `trace_id`s for distributed tracing.
//! - Protocol version negotiation using `ProtocolVersion`.
//! - Safe deserialization of both legacy out-of-date and future payloads.
//!
//! Run with: `cargo run --example message_envelope_demo`
//! (if testing inside the crate directly, simply `cargo run` inside `examples/message_envelope_demo`)

use mofa_kernel::llm::types::{MessageEnvelope, ProtocolVersion};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

// Define a simple custom payload for our application logic
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct ChatPayload {
    user: String,
    message: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    info!("Starting MessageEnvelope protocol demonstration...\n");

    // ==========================================
    // Use Case 1: Happy Path with Trace ID
    // ==========================================
    info!("--- Use Case 1: Happy Path (V1 / trace_id) ---");
    let original_payload = ChatPayload {
        user: "Alice".into(),
        message: "Hello MoFA!".into(),
    };

    // We wrap our payload in a new envelope and assign a distributed trace ID.
    // By default, `MessageEnvelope::new` applies `ProtocolVersion::V1`.
    let envelope = MessageEnvelope::new(original_payload)
        .with_trace_id("trace-req-12345");

    info!("Created V1 Envelope: {:?}", envelope);
    assert_eq!(envelope.version, ProtocolVersion::V1);
    
    // We can always verify the envelope version using `check_version()`.
    // This returns an AgentError::ProtocolVersionMismatch if the build does not support the version.
    envelope.check_version()?; 
    info!("Version check passed successfully for V1.");

    // Serialize it to simulate network transmission
    let json_bytes = serde_json::to_string_pretty(&envelope)?;
    info!("Serialized Envelope JSON:\n{}\n", json_bytes);

    // ==========================================
    // Use Case 2: Backward Compatibility
    // ==========================================
    info!("--- Use Case 2: Backward Compatibility (Legacy Payloads) ---");
    // Imagine we receive a message from an older MoFA node built *before* `ProtocolVersion` existed.
    // It emits JSON without a `"version"` field.
    let legacy_json = r#"{
        "payload": {
            "user": "Bob",
            "message": "I'm sending this from last week!"
        }
    }"#;

    // Deserializing this works seamlessly; the `version` field defaults to `ProtocolVersion::V1`.
    let legacy_envelope: MessageEnvelope<ChatPayload> = serde_json::from_str(legacy_json)?;
    info!("Parsed legacy JSON successfully.");
    info!("Implicitly assigned Protocol Version: {}", legacy_envelope.version);
    info!("Payload: {:?}\n", legacy_envelope.payload);
    assert_eq!(legacy_envelope.version, ProtocolVersion::V1);

    // ==========================================
    // Use Case 3: Future Version Handling
    // ==========================================
    info!("--- Use Case 3: Future Protocol Version Mismatch ---");
    // Imagine we receive a message from a *newer* MoFA node emitting protocol "2".
    let future_json = r#"{
        "version": "2",
        "trace_id": "trace-req-99999",
        "payload": {
            "user": "Eve",
            "message": "I'm from the future!"
        }
    }"#;

    // The deserialization completes WITHOUT returning a serde Parse error.
    // Instead, the version string "2" falls back to the `ProtocolVersion::Unknown` variant.
    let future_envelope: MessageEnvelope<ChatPayload> = serde_json::from_str(future_json)?;
    info!("Parsed future JSON without serde panics.");
    info!("Detected Version Variant: {:?}", future_envelope.version);
    
    // However, when we explicitly validate it before processing business logic:
    match future_envelope.check_version() {
        Ok(_) => warn!("Wait, this shouldn't happen!"),
        Err(e) => info!("Correctly caught Version Mismatch error: {}", e),
    }
    info!("");

    // ==========================================
    // Use Case 4: Payload Mapping
    // ==========================================
    info!("--- Use Case 4: Payload Mapping ---");
    // Often, you want to transform the inner payload type without losing
    // the envelope metadata (version + trace_id). The `map` function does exactly this.
    let initial_envelope = MessageEnvelope::new(42u32)
        .with_trace_id("numeric-trace-id");
    
    info!("Initial Envelope:  {:?}", initial_envelope);

    let string_envelope = initial_envelope.map(|num| format!("Converted number: {}", num));
    info!("Mapped Envelope: {:?}", string_envelope);

    assert_eq!(string_envelope.version, ProtocolVersion::V1);
    assert_eq!(string_envelope.trace_id.as_deref(), Some("numeric-trace-id"));
    assert_eq!(string_envelope.payload, "Converted number: 42");

    info!("\nDemonstration complete.");
    Ok(())
}
