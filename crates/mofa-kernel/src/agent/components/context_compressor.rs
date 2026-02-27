//! Context compression component
//!
//! Defines the interface for managing conversation length when interacting with LLMs.
//! When accumulated context exceeds token limits, a ContextCompressor intelligently
//! trims or summarises the message history so agents can run indefinitely.
//!
//! # Architecture
//!
//! This module only contains the trait definition and associated data types.
//! Concrete implementations live in `mofa-foundation`.
//!
//! # Example
//!
//! ```rust,ignore
//! use mofa_kernel::agent::components::context_compressor::{ContextCompressor, CompressionStrategy};
//!
//! async fn ensure_fits(compressor: &dyn ContextCompressor, messages: Vec<ChatMessage>) -> Vec<ChatMessage> {
//!     let tokens = compressor.count_tokens(&messages);
//!     if tokens > 4096 {
//!         compressor.compress(messages, 4096).await.unwrap()
//!     } else {
//!         messages
//!     }
//! }
//! ```

use crate::agent::error::AgentResult;
use crate::agent::types::ChatMessage;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// The strategy a compressor uses to reduce context length.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum CompressionStrategy {
    /// Discard older messages, always keeping the system prompt and the most
    /// recent `window_size` non-system messages.
    SlidingWindow {
        /// Number of recent non-system messages to retain.
        window_size: usize,
    },
    /// Use the LLM itself to summarise older portions of the conversation,
    /// replacing them with a single condensed assistant message.
    Summarize,
}

/// Trait for context compression implementations.
///
/// Implementors decide *how* to shorten a message list when it grows beyond
/// `max_tokens`.  Two strategies ship with `mofa-foundation`:
/// - [`SlidingWindowCompressor`](mofa_foundation::agent::components::SlidingWindowCompressor)
/// - [`SummarizingCompressor`](mofa_foundation::agent::components::SummarizingCompressor)
#[async_trait]
pub trait ContextCompressor: Send + Sync {
    /// Shorten `messages` so that the estimated token count fits within
    /// `max_tokens`.  The system prompt (role `"system"`) must always be
    /// preserved; the most recent messages must be kept when possible.
    ///
    /// If the conversation is already within the budget, return it unchanged.
    async fn compress(
        &self,
        messages: Vec<ChatMessage>,
        max_tokens: usize,
    ) -> AgentResult<Vec<ChatMessage>>;

    /// Estimate the number of tokens consumed by a slice of messages.
    ///
    /// The default implementation uses the `chars / 4` heuristic which is a
    /// reasonable approximation for English text with GPT-family tokenisers.
    /// Override this method to plug in a tiktoken-style counter.
    fn count_tokens(&self, messages: &[ChatMessage]) -> usize {
        messages
            .iter()
            .filter_map(|m| m.content.as_ref())
            .map(|c| c.len() / 4 + 1)
            .sum()
    }

    /// The compression strategy this compressor uses.
    fn strategy(&self) -> CompressionStrategy;

    /// A short human-readable name for this compressor (used in logs).
    fn name(&self) -> &str;
}
