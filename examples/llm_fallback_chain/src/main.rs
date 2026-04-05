//! LLM Provider Fallback Chain — demonstration
//!
//! Shows how to build a [`FallbackChain`] that automatically routes requests
//! to the next provider when the current one fails with a qualifying error.
//!
//! The example uses three mock providers to simulate a real-world priority order:
//!   1. Primary   — fast cloud provider (simulates rate-limiting on first call)
//!   2. Secondary — backup cloud provider (simulates quota exhaustion)
//!   3. Local     — always available local inference (never fails)
//!
//! Run with:
//!   cargo run -p llm_fallback_chain

use async_trait::async_trait;
use mofa_foundation::llm::{
    ChatCompletionRequest, ChatCompletionResponse, ChatMessage, Choice, FallbackChain,
    FallbackCondition, FallbackTrigger, FinishReason, LLMError, LLMProvider, LLMResult,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use tracing::{Level, info, warn};

// ============================================================================
// Mock providers
// ============================================================================

/// Simulates a cloud LLM that is rate-limited for the first N calls.
struct PrimaryProvider {
    name: String,
    fail_count: usize,
    calls: AtomicUsize,
}

impl PrimaryProvider {
    fn new(fail_count: usize) -> Self {
        Self {
            name: "primary-openai".to_string(),
            fail_count,
            calls: AtomicUsize::new(0),
        }
    }
}

#[async_trait]
impl LLMProvider for PrimaryProvider {
    fn name(&self) -> &str {
        &self.name
    }

    async fn chat(&self, _request: ChatCompletionRequest) -> LLMResult<ChatCompletionResponse> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        if call < self.fail_count {
            warn!(provider = %self.name, call, "Simulating rate-limit error");
            Err(LLMError::RateLimited(
                "429 Too Many Requests".to_string(),
            ))
        } else {
            info!(provider = %self.name, "Request succeeded");
            Ok(make_response(&self.name, "Hello from the primary provider!"))
        }
    }
}

/// Simulates a backup cloud LLM that has exhausted its monthly quota.
struct SecondaryProvider {
    name: String,
}

impl SecondaryProvider {
    fn new() -> Self {
        Self {
            name: "secondary-anthropic".to_string(),
        }
    }
}

#[async_trait]
impl LLMProvider for SecondaryProvider {
    fn name(&self) -> &str {
        &self.name
    }

    async fn chat(&self, _request: ChatCompletionRequest) -> LLMResult<ChatCompletionResponse> {
        warn!(provider = %self.name, "Simulating quota-exceeded error");
        Err(LLMError::QuotaExceeded(
            "Monthly quota exhausted".to_string(),
        ))
    }
}

/// Simulates a local inference server that is always available.
struct LocalProvider {
    name: String,
}

impl LocalProvider {
    fn new() -> Self {
        Self {
            name: "local-ollama".to_string(),
        }
    }
}

#[async_trait]
impl LLMProvider for LocalProvider {
    fn name(&self) -> &str {
        &self.name
    }

    async fn chat(&self, _request: ChatCompletionRequest) -> LLMResult<ChatCompletionResponse> {
        info!(provider = %self.name, "Serving request from local inference");
        Ok(make_response(
            &self.name,
            "Hello from the local provider (offline fallback)!",
        ))
    }
}

// ============================================================================
// Helpers
// ============================================================================

fn make_response(model: &str, text: &str) -> ChatCompletionResponse {
    ChatCompletionResponse {
        id: "demo".to_string(),
        object: "chat.completion".to_string(),
        created: 0,
        model: model.to_string(),
        choices: vec![Choice {
            index: 0,
            message: ChatMessage::assistant(text),
            finish_reason: Some(FinishReason::Stop),
            logprobs: None,
        }],
        usage: None,
        system_fingerprint: None,
    }
}

// ============================================================================
// Main
// ============================================================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    info!("=== LLM Provider Fallback Chain Demo ===\n");

    // ----------------------------------------------------------------
    // Scenario 1: Primary is rate-limited → falls back to secondary →
    //             secondary quota exceeded → falls back to local
    // ----------------------------------------------------------------
    info!("--- Scenario 1: full chain fallback ---");

    let chain = FallbackChain::builder()
        .name("demo-chain")
        // Primary: uses default trigger (fallback on RateLimited, QuotaExceeded,
        // NetworkError, Timeout, AuthError)
        .add_provider(PrimaryProvider::new(999))  // always rate-limited
        // Secondary: same default trigger
        .add_provider(SecondaryProvider::new())   // always quota-exceeded
        // Local: terminal — never falls back from here
        .add_last(LocalProvider::new())
        .build();

    let request = ChatCompletionRequest::new("gpt-4");
    let response = chain.chat(request).await?;
    info!(
        answer = response.content().unwrap_or("(empty)"),
        "Got answer from chain"
    );
    println!("\nResponse: {}\n", response.content().unwrap_or("(empty)"));

    // ----------------------------------------------------------------
    // Scenario 2: Primary succeeds on second call (after one rate-limit)
    // ----------------------------------------------------------------
    info!("--- Scenario 2: primary recovers on second call ---");

    let chain2 = FallbackChain::builder()
        .name("recovering-chain")
        .add_provider(PrimaryProvider::new(1))    // rate-limited only on first call
        .add_last(LocalProvider::new())
        .build();

    // First call — primary rate-limited → local handles it
    let r1 = chain2.chat(ChatCompletionRequest::new("gpt-4")).await?;
    println!("Call 1: {}", r1.content().unwrap_or("(empty)"));

    // Second call — primary is now healthy
    let r2 = chain2.chat(ChatCompletionRequest::new("gpt-4")).await?;
    println!("Call 2: {}", r2.content().unwrap_or("(empty)"));

    // ----------------------------------------------------------------
    // Scenario 3: Custom trigger — only fallback on auth errors
    // ----------------------------------------------------------------
    info!("--- Scenario 3: custom trigger (auth-only fallback) ---");

    let chain3 = FallbackChain::builder()
        .name("auth-sensitive-chain")
        .add_with_trigger(
            PrimaryProvider::new(999),   // always rate-limited
            FallbackTrigger::on_conditions(vec![FallbackCondition::AuthError]),
        )
        .add_last(LocalProvider::new())
        .build();

    // Rate-limit is NOT in the trigger — error should propagate, not fall back
    let result = chain3.chat(ChatCompletionRequest::new("gpt-4")).await;
    match result {
        Err(LLMError::RateLimited(msg)) => {
            println!("Scenario 3: rate-limit propagated as expected: {msg}");
        }
        other => println!("Unexpected result: {other:?}"),
    }

    info!("\n=== Demo complete ===");
    Ok(())
}
