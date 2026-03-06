//! This example demonstrates the Streaming Response Protocol (SRP) introduced 
//! in the `mofa-kernel`'s `llm::srp` module. It demonstrates:
//! - Normal stream handling processing `StreamEvent::Delta` frames and exiting on `StreamEvent::Done`.
//! - Stream cancellation triggered by `CancellationToken` generating `StreamEvent::Cancelled`.
//! - Dealing with slow backend models through automated keepalives emitting `StreamEvent::Heartbeat`.
//!
//! Run with: `cargo run --example srp_demo`

use async_trait::async_trait;
use futures::StreamExt;
use mofa_kernel::agent::AgentResult;
use mofa_kernel::llm::provider::LLMProvider;
use mofa_kernel::llm::srp::{stream_inference, SrpConfig, StreamEvent};
use mofa_kernel::llm::types::{
    ChatCompletionChunk, ChatCompletionRequest, ChatCompletionResponse, ChunkChoice, ChunkDelta,
    EmbeddingRequest, EmbeddingResponse, FinishReason,
};
use std::time::Duration;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

// ==========================================
// 1. Defining a Mock Provider
// ==========================================

/// A mock LLM provider that simulates 3 different behaviors based on the input request model string.
#[derive(Clone)]
struct MockStreamingProvider;

#[async_trait]
impl LLMProvider for MockStreamingProvider {
    fn name(&self) -> &str {
        "SRPMockAI"
    }

    async fn chat(&self, _r: ChatCompletionRequest) -> AgentResult<ChatCompletionResponse> {
        unimplemented!("Not used in streaming demo")
    }

    async fn embedding(&self, _r: EmbeddingRequest) -> AgentResult<EmbeddingResponse> {
        unimplemented!("Not used in streaming demo")
    }

    fn supports_streaming(&self) -> bool {
        true
    }

    async fn chat_stream(
        &self,
        request: ChatCompletionRequest,
    ) -> AgentResult<mofa_kernel::llm::provider::ChatStream> {
        // Our mock detects which scenario is running via the `model` parameter.
        let stream = match request.model.as_str() {
            "happy-path" => Box::pin(futures::stream::unfold(
                (vec!["Hello", " ", "MoFA!", " ", "Powered", " by", " SRP"], 0),
                move |(tokens, idx)| async move {
                    if idx < tokens.len() {
                        sleep(Duration::from_millis(150)).await;
                        let chunk = Self::text_chunk(tokens[idx], None);
                        Some((Ok(chunk), (tokens, idx + 1)))
                    } else if idx == tokens.len() {
                        let chunk = Self::text_chunk("", Some(FinishReason::Stop));
                        Some((Ok(chunk), (tokens, idx + 1)))
                    } else {
                        None
                    }
                },
            )) as mofa_kernel::llm::provider::ChatStream,

            "slow-model" => Box::pin(futures::stream::unfold((), |()| async {
                // This model never returns data; it just sleeps forever to simulate a stall,
                // which will trigger Heartbeats.
                sleep(Duration::from_secs(60)).await;
                None::<((AgentResult<ChatCompletionChunk>), ())>
            })) as mofa_kernel::llm::provider::ChatStream,

            "cancellation-model" => Box::pin(futures::stream::unfold((), |()| async {
                // Returns very slowly, giving enough time for the client task to cancel.
                sleep(Duration::from_secs(1)).await;
                let chunk = Self::text_chunk("Slow chunk...", None);
                Some((Ok(chunk), ()))
            })) as mofa_kernel::llm::provider::ChatStream,

            _ => panic!("Unknown mock scenario"),
        };

        Ok(stream)
    }
}

impl MockStreamingProvider {
    fn text_chunk(text: &str, finish_reason: Option<FinishReason>) -> ChatCompletionChunk {
        ChatCompletionChunk {
            choices: vec![ChunkChoice {
                index: 0,
                delta: ChunkDelta {
                    role: None,
                    content: Some(text.to_string()),
                    tool_calls: None,
                },
                finish_reason,
            }],
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    info!("Starting Streaming Response Protocol (SRP) demonstration...\n");

    let provider = MockStreamingProvider;

    // ==========================================
    // Scenario 1: Happy Path
    // ==========================================
    info!("--- Scenario 1: Normal Happy Path Stream ---");
    let req1 = ChatCompletionRequest::new("happy-path");
    let token1 = CancellationToken::new();
    let config1 = SrpConfig::default();

    let mut stream1 = stream_inference(&provider, req1, token1.clone(), config1).await?;
    let mut generated_text = String::new();

    while let Some(event) = stream1.next().await {
        match event {
            StreamEvent::Delta(chunk) => {
                let text = chunk.delta;
                info!("  [Delta]     Recv: {:?}", text);
                generated_text.push_str(&text);
            }
            StreamEvent::Done => {
                info!("  [Done]      Stream Finished cleanly!");
                break;
            }
            StreamEvent::Heartbeat => info!("  [Heartbeat] Keepalive"),
            StreamEvent::Cancelled => warn!("  [Cancelled] Stream aborted"),
            _ => warn!("  [Unknown]   Unhandled variant structure"),
        }
    }
    info!("Result: {}\n", generated_text);

    // ==========================================
    // Scenario 2: Heartbeats
    // ==========================================
    info!("--- Scenario 2: Handling a stalled model with Heartbeats ---");
    let req2 = ChatCompletionRequest::new("slow-model");
    let token2 = CancellationToken::new();
    
    // We set a very short heartbeat interval (400ms) to trigger it easily,
    // since the mock will pause for 60 seconds without data.
    let config2 = SrpConfig {
        heartbeat_interval: Duration::from_millis(400),
        channel_capacity: 64,
    };

    let mut stream2 = stream_inference(&provider, req2, token2.clone(), config2).await?;
    let mut heartbeat_count = 0;

    while let Some(event) = stream2.next().await {
        match event {
            StreamEvent::Heartbeat => {
                heartbeat_count += 1;
                info!("  [Heartbeat] Backend is stalled but connection is alive (#{heartbeat_count})");

                // Stop the demonstration after 3 heartbeats.
                if heartbeat_count >= 3 {
                    info!("Got enough heartbeats. Cancelling the stalled stream.");
                    token2.cancel();
                }
            }
            StreamEvent::Cancelled => {
                info!("  [Cancelled] Cleanly caught cancellation triggered by heartbeat timer limit.");
                break;
            }
            _ => {}
        }
    }
    info!("Successfully demonstrated automated keepalives.\n");

    // ==========================================
    // Scenario 3: Mid-stream Cancellation
    // ==========================================
    info!("--- Scenario 3: Client driven cancellation ---");
    let req3 = ChatCompletionRequest::new("cancellation-model");
    let token3 = CancellationToken::new();
    let token_clone = token3.clone();
    let config3 = SrpConfig::default();

    let mut stream3 = stream_inference(&provider, req3, token_clone, config3).await?;

    // We start the stream, but in 700ms an external factor (e.g. a user clicking "Stop Generating" or
    // a timeout in the supervisor) triggers the cancellation token.
    tokio::spawn(async move {
        sleep(Duration::from_millis(700)).await;
        info!("  [Async Task] User requested cancellation!");
        token3.cancel();
    });

    while let Some(event) = stream3.next().await {
        match event {
            StreamEvent::Delta(_) => warn!("  [Delta] We shouldn't receive this far due to cancellation"),
            StreamEvent::Cancelled => {
                info!("  [Cancelled] SRP Framework cleanly terminated the underlying iteration upon cancellation.");
                break;
            }
            _ => {}
        }
    }

    info!("\nStreaming Response Protocol (SRP) demonstration complete.");
    Ok(())
}
