//! This example demonstrates the Inference Request Protocol (IRP) introduced 
//! in the `mofa-kernel`'s `llm::irp` module. It demonstrates:
//! - Creating various `InferenceRequest` envelopes (Text, Multimodal, ToolCall, Embedding).
//! - Defining a custom backend via `InferenceProtocol` that advertises generic `InferenceCapabilities`.
//! - Using `InferenceCapabilities::supports_modality()` to proactively catch bad requests.
//! - Dispatching requests dynamically through `.infer()`.
//!
//! Run with: `cargo run --example irp_demo`

use async_trait::async_trait;
use mofa_kernel::agent::AgentResult;
use mofa_kernel::llm::irp::{
    InferenceCapabilities, InferenceProtocol, InferenceRequest, InferenceResponse, RequestModality,
};
use mofa_kernel::llm::types::{
    ChatCompletionRequest, ChatCompletionResponse, ChatMessage, Choice, MessageContent, Role, Tool,
    FinishReason,
};
use tracing::{error, info};

// ==========================================
// 1. Implementing the InferenceProtocol mock
// ==========================================

/// A mock backend router that claims to support Text and Tool Calling, but
/// specifically does NOT support Embeddings or Multimodal inputs.
struct MockSecureRouter;

#[async_trait]
impl InferenceProtocol for MockSecureRouter {
    fn capabilities(&self) -> InferenceCapabilities {
        InferenceCapabilities {
            streaming: false,
            // We explicitly opt-IN to tool calling.
            tool_calling: true,
            // Opt-OUT functionality
            multimodal: false,
            embedding: false,
            ..Default::default()
        }
    }

    /// The default `.infer()` method will hit this function when the request modality
    /// is `Text` or `ToolCall`.
    async fn infer_chat(
        &self,
        request: ChatCompletionRequest,
    ) -> AgentResult<InferenceResponse> {
        let text = if request.tools.is_some() {
            "I'm a tool-calling backend. I've processed your function signature!".to_string()
        } else {
            "I'm a text chat backend. Hello there!".to_string()
        };

        // We wrap the pseudo-response into `InferenceResponse::Chat` as dictated by the IRP.
        Ok(InferenceResponse::Chat(ChatCompletionResponse {
            choices: vec![Choice {
                index: 0,
                message: ChatMessage {
                    role: Role::Assistant,
                    content: Some(MessageContent::Text(text)),
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                },
                finish_reason: Some(FinishReason::Stop),
                logprobs: None,
            }],
        }))
    }

    // We do NOT implement `infer_embedding` or `infer_multimodal` here. The default trait hooks
    // return an `AgentError` explicitly stating the backend doesn't support them.
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    info!("Starting Inference Request Protocol (IRP) demonstration...\n");

    let router = MockSecureRouter;
    let caps = router.capabilities();

    info!("Router Capabilities Advertised:");
    info!("  - Supports Text?        {}", caps.supports_modality(&RequestModality::Text));
    info!("  - Supports Tool-calls?  {}", caps.supports_modality(&RequestModality::ToolCall));
    info!("  - Supports Multimodal?  {}", caps.supports_modality(&RequestModality::Multimodal));
    info!("  - Supports Embeddings?  {}\n", caps.supports_modality(&RequestModality::Embedding));

    // ==========================================
    // Scenario 1: Plain Text Envelope
    // ==========================================
    info!("--- Scenario 1: Standard Text Request ---");
    let text_req = InferenceRequest::text("gpt-4o", "Hello!")
        .with_temperature(0.7)
        .expect("Failed to bind valid temp");
        
    assert!(caps.supports_modality(&text_req.modality));
    
    // Pass the agnostic envelope to the single `.infer()` dispatch point.
    let text_res = router.infer(text_req).await?;
    info!("  [Success] Response text extracted: {:?}", text_res.text_content().unwrap());


    // ==========================================
    // Scenario 2: Tool Call Envelope
    // ==========================================
    info!("\n--- Scenario 2: Tool-Call Request ---");
    let tool_req = InferenceRequest::text("gpt-4o", "Fetch the weather")
        .with_tool(Tool::function("get_weather", "Gets local climate", serde_json::json!({})));
        
    assert!(caps.supports_modality(&tool_req.modality));
    
    let tool_res = router.infer(tool_req).await?;
    info!("  [Success] Response text extracted: {:?}", tool_res.text_content().unwrap());


    // ==========================================
    // Scenario 3: Embedding Request (Blocked by Caps)
    // ==========================================
    info!("\n--- Scenario 3: Embedding Request (Caught proactively) ---");
    let emb_req = InferenceRequest::embedding("some-embed-model", "Embed this string.");
    
    // We can look before we leap! An orchestrator checks capabilities BEFORE dispatching network rules.
    if !caps.supports_modality(&emb_req.modality) {
        info!("  [Blocked] Capability check affirmatively blocked the embedding execution!");
    } else {
        error!("Should have blocked!");
    }


    // ==========================================
    // Scenario 4: Multimodal Request (Handled natively by default fallback)
    // ==========================================
    info!("\n--- Scenario 4: Multimodal Request (Caught reactively by trait) ---");
    // Say we bypassed the proactive `caps.supports_modality()` check blindly:
    let multi_req = InferenceRequest::multimodal(
        "gpt-4o-vision",
        vec![ChatMessage::user_with_image("Describe this image", "https://example.com/img.png")],
    );
    
    // The `.infer()` function parses the modality (Multimodal), translates the InferenceRequest into
    // a ChatCompletionRequest, and hands it off to `router.infer_multimodal()`.
    // Because we didn't override `infer_multimodal()`, the default hook falls vertically downwards
    // to `infer_chat()`. But wait, in the IRP `.infer()` source code, it first attempts to parse
    // out the payload.
    // By invoking `infer_multimodal()` without explicitly supporting it structurally, a robust router
    // can choose to intercept the request and throw custom Error logic if the backend prohibits it.
    // Currently, `InferenceProtocol` allows a passthrough. Since our `caps` says no, let's actually
    // perform the execution and observe we might want to manually reject it inside `infer_chat` if
    // we strictly strictly needed, BUT the `supports_modality` check is the intended semantic gate.
    
    match router.infer(multi_req).await {
        Ok(m_res) => info!("  [Warning] Handled via default trait passthrough to `infer_chat`: {:?}", m_res.text_content()),
        Err(e) => info!("  [Error] Caught error logically: {:?}", e),
    }

    info!("\nInference Request Protocol (IRP) demonstration complete.");
    Ok(())
}
