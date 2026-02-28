//! Context compressor implementations
//!
//! Provides two concrete [`ContextCompressor`] strategies:
//!
//! - [`SlidingWindowCompressor`] — keeps the system prompt and the N most
//!   recent messages, discarding anything older.
//! - [`SummarizingCompressor`] — asks the LLM to condense older turns into a
//!   single summary message, preserving semantic content while reducing token
//!   count.
//!
//! A [`TokenCounter`] utility is also exported for callers that want to
//! estimate token usage without instantiating a compressor.

use async_trait::async_trait;
use mofa_kernel::agent::components::context_compressor::{CompressionStrategy, ContextCompressor};
use mofa_kernel::agent::error::{AgentError, AgentResult};
use mofa_kernel::agent::types::ChatMessage;
use std::sync::Arc;

// ============================================================================
// Token counter utility
// ============================================================================

/// Lightweight token-count estimator using the `chars / 4` heuristic.
///
/// This is intentionally approximate.  For production use where billing
/// accuracy matters, replace the implementation with a tiktoken-style counter.
pub struct TokenCounter;

impl TokenCounter {
    /// Estimate total tokens for a slice of messages.
    pub fn count(messages: &[ChatMessage]) -> usize {
        messages
            .iter()
            .filter_map(|m| m.content.as_ref())
            .map(|c| Self::count_str(c))
            .sum()
    }

    /// Estimate tokens for a single string.
    pub fn count_str(s: &str) -> usize {
        s.len() / 4 + 1
    }
}

// ============================================================================
// Sliding window compressor
// ============================================================================

/// Keeps the system prompt plus the `window_size` most-recent non-system
/// messages.  Older messages are discarded entirely.
///
/// This is the simplest possible strategy — zero latency, no external calls —
/// but it loses older context completely.
///
/// # Example
///
/// ```rust,ignore
/// let compressor = SlidingWindowCompressor::new(20);
/// let trimmed = compressor.compress(messages, 4096).await?;
/// ```
pub struct SlidingWindowCompressor {
    window_size: usize,
}

impl SlidingWindowCompressor {
    /// Create a new compressor that retains at most `window_size` non-system
    /// messages after the system prompt.
    pub fn new(window_size: usize) -> Self {
        Self { window_size }
    }
}

#[async_trait]
impl ContextCompressor for SlidingWindowCompressor {
    async fn compress(
        &self,
        messages: Vec<ChatMessage>,
        max_tokens: usize,
    ) -> AgentResult<Vec<ChatMessage>> {
        // If already within budget, return unchanged.
        if self.count_tokens(&messages) <= max_tokens {
            return Ok(messages);
        }

        // Split system messages from the rest.
        let (system_msgs, mut conversation): (Vec<_>, Vec<_>) =
            messages.into_iter().partition(|m| m.role == "system");

        // Keep only the most-recent window_size messages.
        if conversation.len() > self.window_size {
            let keep_from = conversation.len() - self.window_size;
            conversation = conversation.split_off(keep_from);
        }

        let mut result = system_msgs;
        result.extend(conversation);
        Ok(result)
    }

    fn strategy(&self) -> CompressionStrategy {
        CompressionStrategy::SlidingWindow {
            window_size: self.window_size,
        }
    }

    fn name(&self) -> &str {
        "sliding_window"
    }
}

// ============================================================================
// Summarizing compressor
// ============================================================================

/// Compresses older conversation turns by asking the LLM to produce a concise
/// summary, then replaces those turns with a single assistant message containing
/// that summary.
///
/// The most-recent `keep_recent` non-system messages are left untouched so the
/// LLM retains immediate context.
///
/// This compressor accepts any provider that implements the foundation's
/// [`LLMProvider`](crate::llm::provider::LLMProvider) trait, which includes
/// `OpenAIProvider`, `AnthropicProvider`, and `OllamaProvider`.
///
/// # Example
///
/// ```rust,ignore
/// let compressor = SummarizingCompressor::new(llm.clone())
///     .with_keep_recent(8);
/// let trimmed = compressor.compress(messages, 4096).await?;
/// ```
pub struct SummarizingCompressor {
    llm: Arc<dyn crate::llm::provider::LLMProvider>,
    keep_recent: usize,
}

impl SummarizingCompressor {
    /// Create a new compressor using `llm` for summarisation.
    /// Defaults to keeping the 10 most-recent non-system messages intact.
    pub fn new(llm: Arc<dyn crate::llm::provider::LLMProvider>) -> Self {
        Self {
            llm,
            keep_recent: 10,
        }
    }

    /// Override how many recent messages to preserve without summarisation.
    pub fn with_keep_recent(mut self, n: usize) -> Self {
        self.keep_recent = n;
        self
    }

    /// Build the summarisation prompt from the messages to be condensed.
    fn build_summary_prompt(messages: &[ChatMessage]) -> String {
        let history = messages
            .iter()
            .filter_map(|m| {
                m.content
                    .as_ref()
                    .map(|c| format!("{}: {}", m.role, c))
            })
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            "Summarise the following conversation concisely, preserving all \
             important facts, decisions, and context. Write in third person.\n\n\
             ---\n{}\n---",
            history
        )
    }
}

#[async_trait]
impl ContextCompressor for SummarizingCompressor {
    async fn compress(
        &self,
        messages: Vec<ChatMessage>,
        max_tokens: usize,
    ) -> AgentResult<Vec<ChatMessage>> {
        // If already within budget, return unchanged.
        if self.count_tokens(&messages) <= max_tokens {
            return Ok(messages);
        }

        // Separate system messages from conversation turns.
        let (system_msgs, conversation): (Vec<_>, Vec<_>) =
            messages.into_iter().partition(|m| m.role == "system");

        // Nothing to summarise if the conversation is too short.
        if conversation.len() <= self.keep_recent {
            let mut result = system_msgs;
            result.extend(conversation);
            return Ok(result);
        }

        let split_at = conversation.len() - self.keep_recent;
        let (to_summarise, recent) = conversation.split_at(split_at);

        // Ask the LLM for a summary of the older turns using the foundation's
        // ChatCompletionRequest which is what the actual providers understand.
        let prompt = Self::build_summary_prompt(to_summarise);
        let summary_request = crate::llm::types::ChatCompletionRequest::new("gpt-4o-mini")
            .user(prompt)
            .temperature(0.3)
            .max_tokens(512);

        let summary_response = self
            .llm
            .chat(summary_request)
            .await
            .map_err(|e| AgentError::ExecutionFailed(format!("summarisation failed: {e}")))?;

        let summary_text = summary_response
            .content()
            .map(str::to_string)
            .unwrap_or_else(|| "[summary unavailable]".to_string());

        // Reassemble: system prompt → summary → recent turns.
        let summary_message = ChatMessage {
            role: "assistant".to_string(),
            content: Some(format!("[Conversation summary]\n{summary_text}")),
            tool_call_id: None,
            tool_calls: None,
        };

        let mut result = system_msgs;
        result.push(summary_message);
        result.extend_from_slice(recent);
        Ok(result)
    }

    fn strategy(&self) -> CompressionStrategy {
        CompressionStrategy::Summarize
    }

    fn name(&self) -> &str {
        "summarizing"
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use mofa_kernel::agent::types::ChatMessage;

    fn make_msg(role: &str, content: &str) -> ChatMessage {
        ChatMessage {
            role: role.to_string(),
            content: Some(content.to_string()),
            tool_call_id: None,
            tool_calls: None,
        }
    }

    fn system_only() -> Vec<ChatMessage> {
        vec![make_msg("system", "You are a helpful assistant.")]
    }

    fn short_conversation() -> Vec<ChatMessage> {
        vec![
            make_msg("system", "You are a helpful assistant."),
            make_msg("user", "Hello"),
            make_msg("assistant", "Hi there!"),
        ]
    }

    fn long_conversation(n: usize) -> Vec<ChatMessage> {
        let mut msgs = vec![make_msg("system", "You are a helpful assistant.")];
        for i in 0..n {
            msgs.push(make_msg("user", &format!("Message {i}")));
            msgs.push(make_msg("assistant", &format!("Response {i}")));
        }
        msgs
    }

    // A mock LLM that always returns "summary text" for testing.
    // Implements the foundation's LLMProvider trait (which real providers use).
    struct MockLLM;

    #[async_trait]
    impl crate::llm::provider::LLMProvider for MockLLM {
        fn name(&self) -> &str {
            "mock"
        }

        async fn chat(
            &self,
            _request: crate::llm::types::ChatCompletionRequest,
        ) -> crate::llm::types::LLMResult<crate::llm::types::ChatCompletionResponse> {
            use crate::llm::types::{
                ChatCompletionResponse, ChatMessage, Choice, MessageContent, Role,
            };
            Ok(ChatCompletionResponse {
                id: "mock-id".to_string(),
                object: "chat.completion".to_string(),
                created: 0,
                model: "mock".to_string(),
                choices: vec![Choice {
                    index: 0,
                    message: ChatMessage {
                        role: Role::Assistant,
                        content: Some(MessageContent::Text("summary text".to_string())),
                        name: None,
                        tool_calls: None,
                        tool_call_id: None,
                    },
                    finish_reason: None,
                    logprobs: None,
                }],
                usage: None,
                system_fingerprint: None,
            })
        }
    }

    // ---- TokenCounter -------------------------------------------------------

    #[test]
    fn token_counter_empty() {
        assert_eq!(TokenCounter::count(&[]), 0);
    }

    #[test]
    fn token_counter_heuristic() {
        let msgs = vec![make_msg("user", "hello")]; // "hello" = 5 chars → 5/4+1 = 2
        assert_eq!(TokenCounter::count(&msgs), 2);
    }

    #[test]
    fn token_counter_no_content() {
        let msg = ChatMessage {
            role: "assistant".to_string(),
            content: None,
            tool_call_id: None,
            tool_calls: None,
        };
        assert_eq!(TokenCounter::count(&[msg]), 0);
    }

    // ---- SlidingWindowCompressor -------------------------------------------

    #[tokio::test]
    async fn sliding_window_under_limit_unchanged() {
        let compressor = SlidingWindowCompressor::new(20);
        let msgs = short_conversation();
        let result = compressor.compress(msgs.clone(), 100_000).await.unwrap();
        assert_eq!(result.len(), msgs.len());
    }

    #[tokio::test]
    async fn sliding_window_only_system_message() {
        let compressor = SlidingWindowCompressor::new(5);
        let msgs = system_only();
        // Force compression by using tiny max_tokens budget.
        // TokenCounter for this system message: "You are a helpful assistant." = 28 chars → 8 tokens
        // Use a budget that is LESS than 8 to trigger compression.
        let result = compressor.compress(msgs.clone(), 1).await.unwrap();
        // System message is always kept; no conversation to drop.
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].role, "system");
    }

    #[tokio::test]
    async fn sliding_window_trims_to_window_size() {
        // 5 user+assistant pairs = 10 conversation messages + 1 system = 11 total.
        let compressor = SlidingWindowCompressor::new(4);
        let msgs = long_conversation(5);
        assert_eq!(msgs.len(), 11);
        // Force compression.
        let result = compressor.compress(msgs, 1).await.unwrap();
        // 1 system + 4 recent conversation messages.
        assert_eq!(result.len(), 5);
        assert_eq!(result[0].role, "system");
    }

    #[tokio::test]
    async fn sliding_window_very_long_single_message() {
        let compressor = SlidingWindowCompressor::new(2);
        let long_content = "a".repeat(10_000);
        let msgs = vec![
            make_msg("system", "sys"),
            make_msg("user", &long_content),
        ];
        // Even though the single message exceeds the token budget, the window
        // compressor keeps it because it is within window_size.
        let result = compressor.compress(msgs, 1).await.unwrap();
        assert_eq!(result.len(), 2);
    }

    #[tokio::test]
    async fn sliding_window_preserves_system_prompt() {
        let compressor = SlidingWindowCompressor::new(2);
        let msgs = long_conversation(10); // 21 messages
        let result = compressor.compress(msgs, 1).await.unwrap();
        assert_eq!(result[0].role, "system");
    }

    // ---- SummarizingCompressor ---------------------------------------------

    #[tokio::test]
    async fn summarizing_under_limit_unchanged() {
        let llm = Arc::new(MockLLM);
        let compressor = SummarizingCompressor::new(llm);
        let msgs = short_conversation();
        let result = compressor.compress(msgs.clone(), 100_000).await.unwrap();
        assert_eq!(result.len(), msgs.len());
    }

    #[tokio::test]
    async fn summarizing_only_system_message() {
        let llm = Arc::new(MockLLM);
        let compressor = SummarizingCompressor::new(llm);
        let msgs = system_only();
        let result = compressor.compress(msgs, 1).await.unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].role, "system");
    }

    #[tokio::test]
    async fn summarizing_injects_summary_message() {
        let llm = Arc::new(MockLLM);
        let compressor = SummarizingCompressor::new(llm).with_keep_recent(2);
        // 1 system + 6 conversation messages (3 pairs).
        let msgs = long_conversation(3);
        assert_eq!(msgs.len(), 7);
        // Force compression.
        let result = compressor.compress(msgs, 1).await.unwrap();
        // Result: system + summary + 2 recent = 4
        assert_eq!(result.len(), 4);
        assert_eq!(result[0].role, "system");
        assert!(result[1]
            .content
            .as_ref()
            .unwrap()
            .starts_with("[Conversation summary]"));
    }

    #[tokio::test]
    async fn summarizing_very_long_single_message() {
        let llm = Arc::new(MockLLM);
        let compressor = SummarizingCompressor::new(llm).with_keep_recent(10);
        let long_content = "x".repeat(50_000);
        let msgs = vec![
            make_msg("system", "sys"),
            make_msg("user", &long_content),
        ];
        // Only 1 conversation message which is <= keep_recent=10, so no
        // summarisation happens; messages are returned as-is even over budget.
        let result = compressor.compress(msgs.clone(), 1).await.unwrap();
        assert_eq!(result.len(), 2);
    }
}
