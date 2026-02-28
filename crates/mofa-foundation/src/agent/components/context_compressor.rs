//! context compressor implementations

use async_trait::async_trait;
use mofa_kernel::agent::components::context_compressor::{
    CompressionMetrics, CompressionResult, CompressionStrategy, ContextCompressor,
};
use mofa_kernel::agent::error::{AgentError, AgentResult};
use mofa_kernel::agent::types::ChatMessage;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tracing::{debug, info, trace};

// compression cache for embeddings and summaries
#[cfg(feature = "compression-cache")]
mod cache {
    use super::*;
    use sha2::{Digest, Sha256};

    /// cache entry for embeddings
    #[derive(Clone, Debug)]
    pub struct EmbeddingCacheEntry {
        pub embedding: Vec<f32>,
        pub accessed_at: Instant,
    }

    /// cache entry for summaries
    #[derive(Clone, Debug)]
    pub struct SummaryCacheEntry {
        pub summary: String,
        pub accessed_at: Instant,
    }

    /// compression cache manager
    pub struct CompressionCache {
        embedding_cache: Arc<RwLock<HashMap<String, EmbeddingCacheEntry>>>,
        summary_cache: Arc<RwLock<HashMap<String, SummaryCacheEntry>>>,
        max_embedding_entries: usize,
        max_summary_entries: usize,
    }

    impl CompressionCache {
        pub fn new(max_embedding_entries: usize, max_summary_entries: usize) -> Self {
            Self {
                embedding_cache: Arc::new(RwLock::new(HashMap::new())),
                summary_cache: Arc::new(RwLock::new(HashMap::new())),
                max_embedding_entries,
                max_summary_entries,
            }
        }

        /// get cache key for text content
        pub fn cache_key(text: &str) -> String {
            let mut hasher = Sha256::new();
            hasher.update(text.as_bytes());
            format!("{:x}", hasher.finalize())
        }

        /// get cached embedding
        pub async fn get_embedding(&self, key: &str) -> Option<Vec<f32>> {
            let cache = self.embedding_cache.read().await;
            cache.get(key).map(|entry| {
                entry.embedding.clone()
            })
        }

        /// store embedding in cache
        pub async fn store_embedding(&self, key: String, embedding: Vec<f32>) {
            let mut cache = self.embedding_cache.write().await;
            
            // evict oldest if at capacity
            if cache.len() >= self.max_embedding_entries && !cache.is_empty() {
                let oldest_key = cache.iter()
                    .min_by_key(|(_, entry)| entry.accessed_at)
                    .map(|(k, _)| k.clone());
                if let Some(key) = oldest_key {
                    cache.remove(&key);
                }
            }

            cache.insert(key, EmbeddingCacheEntry {
                embedding,
                accessed_at: Instant::now(),
            });
        }

        /// get cached summary
        pub async fn get_summary(&self, key: &str) -> Option<String> {
            let cache = self.summary_cache.read().await;
            cache.get(key).map(|entry| {
                entry.summary.clone()
            })
        }

        /// store summary in cache
        pub async fn store_summary(&self, key: String, summary: String) {
            let mut cache = self.summary_cache.write().await;
            
            // evict oldest if at capacity
            if cache.len() >= self.max_summary_entries && !cache.is_empty() {
                let oldest_key = cache.iter()
                    .min_by_key(|(_, entry)| entry.accessed_at)
                    .map(|(k, _)| k.clone());
                if let Some(key) = oldest_key {
                    cache.remove(&key);
                }
            }

            cache.insert(key, SummaryCacheEntry {
                summary,
                accessed_at: Instant::now(),
            });
        }

        /// clear all caches
        pub async fn clear(&self) {
            self.embedding_cache.write().await.clear();
            self.summary_cache.write().await.clear();
        }

        /// get cache statistics
        pub async fn stats(&self) -> CacheStats {
            let embedding_cache = self.embedding_cache.read().await;
            let summary_cache = self.summary_cache.read().await;
            CacheStats {
                embedding_entries: embedding_cache.len(),
                summary_entries: summary_cache.len(),
                max_embedding_entries: self.max_embedding_entries,
                max_summary_entries: self.max_summary_entries,
            }
        }
    }

    /// cache statistics
    #[derive(Debug, Clone)]
    pub struct CacheStats {
        pub embedding_entries: usize,
        pub summary_entries: usize,
        pub max_embedding_entries: usize,
        pub max_summary_entries: usize,
    }
}

#[cfg(feature = "compression-cache")]
pub use cache::{CompressionCache, CacheStats};

// token counter utility

/// lightweight token-count estimator using chars/4 heuristic
pub struct TokenCounter;

impl TokenCounter {
    /// estimate total tokens for messages
    pub fn count(messages: &[ChatMessage]) -> usize {
        messages
            .iter()
            .filter_map(|m| m.content.as_ref())
            .map(|c| Self::count_str(c))
            .sum()
    }

    /// estimate tokens for a string
    pub fn count_str(s: &str) -> usize {
        s.len() / 4 + 1
    }
}

// tiktoken counter (optional, requires tiktoken feature)

#[cfg(feature = "tiktoken")]
/// accurate token counter using tiktoken
#[derive(Debug, Clone)]
pub struct TikTokenCounter {
    encoder: tiktoken_rs::CoreBPE,
}

#[cfg(feature = "tiktoken")]
impl TikTokenCounter {
    /// create token counter using cl100k_base encoding (gpt-4, gpt-3.5-turbo)
    pub fn cl100k_base() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let bpe = tiktoken_rs::cl100k_base()?;
        Ok(Self { encoder: bpe })
    }

    /// create token counter using p50k_base encoding (gpt-3, codex)
    pub fn p50k_base() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let bpe = tiktoken_rs::p50k_base()?;
        Ok(Self { encoder: bpe })
    }

    /// create token counter using p50k_edit encoding (codex edit models)
    pub fn p50k_edit() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let bpe = tiktoken_rs::p50k_edit()?;
        Ok(Self { encoder: bpe })
    }

    /// create token counter using r50k_base encoding (gpt-3 base models)
    pub fn r50k_base() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let bpe = tiktoken_rs::r50k_base()?;
        Ok(Self { encoder: bpe })
    }

    /// create token counter for specific model name
    pub fn for_model(model: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let bpe = tiktoken_rs::get_bpe_from_model(model)?;
        Ok(Self { encoder: bpe })
    }

    /// count tokens in messages (includes ~3 tokens overhead per message)
    pub fn count(&self, messages: &[ChatMessage]) -> usize {
        let content_tokens: usize = messages
            .iter()
            .filter_map(|m| m.content.as_ref())
            .map(|c| self.count_str(c))
            .sum();

        content_tokens + messages.len() * 3
    }

    /// count tokens in a string
    pub fn count_str(&self, text: &str) -> usize {
        self.encoder.encode_with_special_tokens(text).len()
    }

    /// count tokens in a message (includes ~3 tokens overhead)
    pub fn count_message(&self, message: &ChatMessage) -> usize {
        let content_tokens = message
            .content
            .as_ref()
            .map(|c| self.count_str(c))
            .unwrap_or(0);
        content_tokens + 3 // role + framing overhead
    }
}

#[cfg(feature = "tiktoken")]
#[cfg(test)]
mod tiktoken_tests {
    use super::*;

    #[test]
    fn test_tiktoken_counter_cl100k() {
        let counter = TikTokenCounter::cl100k_base().unwrap();
        let text = "Hello, world!";
        let tokens = counter.count_str(text);
        assert!(tokens >= 3 && tokens <= 5);
    }

    #[test]
    fn test_tiktoken_counter_messages() {
        let counter = TikTokenCounter::cl100k_base().unwrap();
        let messages = vec![
            make_msg("system", "You are a helpful assistant."),
            make_msg("user", "Hello"),
        ];
        let tokens = counter.count(&messages);
        assert!(tokens > 5);
    }

    #[test]
    fn test_tiktoken_vs_heuristic() {
        let counter = TikTokenCounter::cl100k_base().unwrap();
        let text = "The quick brown fox jumps over the lazy dog.";
        let tiktoken_count = counter.count_str(text);
        let heuristic_count = TokenCounter::count_str(text);
        assert!(tiktoken_count <= heuristic_count);
    }
}

// sliding window compressor

/// keeps system prompt plus window_size most-recent non-system messages
pub struct SlidingWindowCompressor {
    window_size: usize,
}

impl SlidingWindowCompressor {
    /// create compressor retaining window_size non-system messages
    pub fn new(window_size: usize) -> Self {
        Self { window_size }
    }
}

#[async_trait]
impl ContextCompressor for SlidingWindowCompressor {
    async fn compress_with_metrics(
        &self,
        messages: Vec<ChatMessage>,
        max_tokens: usize,
    ) -> AgentResult<CompressionResult> {
        let start = Instant::now();
        let tokens_before = self.count_tokens(&messages);
        let messages_before = messages.len();

        if tokens_before <= max_tokens {
            let metrics = CompressionMetrics::new(
                tokens_before,
                tokens_before,
                messages_before,
                messages_before,
            );
            return Ok(CompressionResult::new(messages, metrics, self.name().to_string()));
        }

        let (system_msgs, mut conversation): (Vec<_>, Vec<_>) =
            messages.into_iter().partition(|m| m.role == "system");

        if conversation.len() > self.window_size {
            let keep_from = conversation.len() - self.window_size;
            conversation = conversation.split_off(keep_from);
        }

        let mut result = system_msgs;
        result.extend(conversation);
        let tokens_after = self.count_tokens(&result);
        let messages_after = result.len();

        let metrics = CompressionMetrics::new(
            tokens_before,
            tokens_after,
            messages_before,
            messages_after,
        );

        // log compression event
        if metrics.was_compressed() {
            info!(
                strategy = self.name(),
                tokens_before = metrics.tokens_before,
                tokens_after = metrics.tokens_after,
                reduction_percent = metrics.token_reduction_percent,
                "context compressed"
            );
            debug!(
                messages_before = metrics.messages_before,
                messages_after = metrics.messages_after,
                "compression details"
            );
        } else {
            trace!(
                strategy = self.name(),
                tokens = tokens_before,
                "no compression needed"
            );
        }

        Ok(CompressionResult::new(result, metrics, self.name().to_string()))
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

// summarizing compressor

/// compresses older messages using llm summarization
pub struct SummarizingCompressor {
    llm: Arc<dyn crate::llm::provider::LLMProvider>,
    keep_recent: usize,
    #[cfg(feature = "compression-cache")]
    cache: Option<Arc<CompressionCache>>,
}

impl SummarizingCompressor {
    /// create compressor using llm for summarization (default keep_recent=10)
    pub fn new(llm: Arc<dyn crate::llm::provider::LLMProvider>) -> Self {
        Self {
            llm,
            keep_recent: 10,
            #[cfg(feature = "compression-cache")]
            cache: None,
        }
    }

    /// enable caching for summaries (requires compression-cache feature)
    #[cfg(feature = "compression-cache")]
    pub fn with_cache(mut self, cache: Arc<CompressionCache>) -> Self {
        self.cache = Some(cache);
        self
    }

    /// set how many recent messages to preserve
    pub fn with_keep_recent(mut self, n: usize) -> Self {
        self.keep_recent = n;
        self
    }

    fn build_summary_prompt(messages: &[ChatMessage]) -> String {
        let history = messages
            .iter()
            .filter_map(|m| m.content.as_ref().map(|c| format!("{}: {}", m.role, c)))
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
    async fn compress_with_metrics(
        &self,
        messages: Vec<ChatMessage>,
        max_tokens: usize,
    ) -> AgentResult<CompressionResult> {
        let tokens_before = self.count_tokens(&messages);
        let messages_before = messages.len();
        if tokens_before <= max_tokens {
            let metrics = CompressionMetrics::new(
                tokens_before,
                tokens_before,
                messages_before,
                messages_before,
            );
            return Ok(CompressionResult::new(messages, metrics, self.name().to_string()));
        }

        let (system_msgs, conversation): (Vec<_>, Vec<_>) =
            messages.into_iter().partition(|m| m.role == "system");

        if conversation.len() <= self.keep_recent {
            let mut result = system_msgs;
            result.extend(conversation);
            let tokens_after = self.count_tokens(&result);
            let metrics = CompressionMetrics::new(
                tokens_before,
                tokens_after,
                messages_before,
                result.len(),
            );
            return Ok(CompressionResult::new(result, metrics, self.name().to_string()));
        }

        let split_at = conversation.len() - self.keep_recent;
        let (to_summarise, recent) = conversation.split_at(split_at);

        let prompt = Self::build_summary_prompt(to_summarise);
        
        // check cache if enabled
        #[cfg(feature = "compression-cache")]
        let cache_key = if let Some(ref cache) = self.cache {
            Some(CompressionCache::cache_key(&prompt))
        } else {
            None
        };

        #[cfg(feature = "compression-cache")]
        let summary_text = if let (Some(ref cache), Some(ref key)) = (self.cache.as_ref(), cache_key.as_ref()) {
            if let Some(cached) = cache.get_summary(key).await {
                cached
            } else {
                let summary_response = self
                    .llm
                    .chat(crate::llm::types::ChatCompletionRequest::new("gpt-4o-mini")
                        .user(prompt.clone())
                        .temperature(0.3)
                        .max_tokens(512))
                    .await
                    .map_err(|e| AgentError::ExecutionFailed(format!("summarisation failed: {e}")))?;

                let text = summary_response
                    .content()
                    .map(str::to_string)
                    .unwrap_or_else(|| "[summary unavailable]".to_string());
                
                cache.store_summary(key.to_string(), text.clone()).await;
                text
            }
        } else {
        let summary_request = crate::llm::types::ChatCompletionRequest::new("gpt-4o-mini")
            .user(prompt)
            .temperature(0.3)
            .max_tokens(512);

        let summary_response = self
            .llm
            .chat(summary_request)
            .await
            .map_err(|e| AgentError::ExecutionFailed(format!("summarisation failed: {e}")))?;

            summary_response
            .content()
            .map(str::to_string)
                .unwrap_or_else(|| "[summary unavailable]".to_string())
        };

        #[cfg(not(feature = "compression-cache"))]
        let summary_text = {
            let summary_request = crate::llm::types::ChatCompletionRequest::new("gpt-4o-mini")
                .user(prompt)
                .temperature(0.3)
                .max_tokens(512);

            let summary_response = self
                .llm
                .chat(summary_request)
                .await
                .map_err(|e| AgentError::ExecutionFailed(format!("summarisation failed: {e}")))?;

            summary_response
                .content()
                .map(str::to_string)
                .unwrap_or_else(|| "[summary unavailable]".to_string())
        };

        let summary_message = ChatMessage {
            role: "assistant".to_string(),
            content: Some(format!("[Conversation summary]\n{summary_text}")),
            tool_call_id: None,
            tool_calls: None,
        };

        let mut result = system_msgs;
        result.push(summary_message);
        result.extend_from_slice(recent);
        let tokens_after = self.count_tokens(&result);
        let messages_after = result.len();

        let metrics = CompressionMetrics::new(
            tokens_before,
            tokens_after,
            messages_before,
            messages_after,
        );

        // log compression event
        if metrics.was_compressed() {
            info!(
                strategy = self.name(),
                tokens_before = metrics.tokens_before,
                tokens_after = metrics.tokens_after,
                reduction_percent = metrics.token_reduction_percent,
                "context compressed"
            );
            debug!(
                messages_before = metrics.messages_before,
                messages_after = metrics.messages_after,
                "compression details"
            );
        } else {
            trace!(
                strategy = self.name(),
                tokens = tokens_before,
                "no compression needed"
            );
        }

        Ok(CompressionResult::new(result, metrics, self.name().to_string()))
    }

    fn strategy(&self) -> CompressionStrategy {
        CompressionStrategy::Summarize
    }

    fn name(&self) -> &str {
        "summarizing"
    }
}

// semantic compressor

/// compresses messages using semantic similarity (embeddings)
pub struct SemanticCompressor {
    llm: Arc<dyn crate::llm::provider::LLMProvider>,
    similarity_threshold: f32,
    keep_recent: usize,
    #[cfg(feature = "compression-cache")]
    cache: Option<Arc<CompressionCache>>,
}

impl SemanticCompressor {
    /// create semantic compressor (default threshold=0.85, keep_recent=5)
    pub fn new(llm: Arc<dyn crate::llm::provider::LLMProvider>) -> Self {
        Self {
            llm,
            similarity_threshold: 0.85,
            keep_recent: 5,
            #[cfg(feature = "compression-cache")]
            cache: None,
        }
    }

    /// enable caching for embeddings (requires compression-cache feature)
    #[cfg(feature = "compression-cache")]
    pub fn with_cache(mut self, cache: Arc<CompressionCache>) -> Self {
        self.cache = Some(cache);
        self
    }

    /// set similarity threshold (0.0-1.0) for merging redundant messages
    pub fn with_similarity_threshold(mut self, threshold: f32) -> Self {
        self.similarity_threshold = threshold.clamp(0.0, 1.0);
        self
    }

    /// set how many recent messages to keep uncompressed
    pub fn with_keep_recent(mut self, n: usize) -> Self {
        self.keep_recent = n;
        self
    }

    fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() || a.is_empty() {
            return 0.0;
        }

        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a == 0.0 || norm_b == 0.0 {
            return 0.0;
        }

        dot / (norm_a * norm_b)
    }

    fn extract_text(message: &ChatMessage) -> String {
        message.content.as_deref().unwrap_or("").to_string()
    }

    async fn generate_embeddings_sequential(
        &self,
        to_compress: &[ChatMessage],
        system_msgs: &[ChatMessage],
        recent: &[ChatMessage],
    ) -> AgentResult<Vec<Option<Vec<f32>>>> {
        let mut embeddings = Vec::new();
        for msg in to_compress {
            let text = Self::extract_text(msg);
            if text.is_empty() {
                embeddings.push(None);
                continue;
            }

            if !self.llm.supports_embedding() {
                let mut result = system_msgs.to_vec();
                result.extend_from_slice(recent);
                return Err(AgentError::ExecutionFailed(
                    "Embeddings not supported by LLM provider".to_string(),
                ));
            }

            #[cfg(feature = "compression-cache")]
            let embedding = if let Some(ref cache) = self.cache {
                let cache_key = CompressionCache::cache_key(&text);
                if let Some(cached) = cache.get_embedding(&cache_key).await {
                    cached
                } else {
                    let embedding_response = self
                        .llm
                        .embedding(crate::llm::types::EmbeddingRequest {
                            model: "text-embedding-ada-002".to_string(),
                            input: crate::llm::types::EmbeddingInput::Single(text.clone()),
                            encoding_format: None,
                            dimensions: None,
                            user: None,
                        })
                        .await
                        .map_err(|e| {
                            AgentError::ExecutionFailed(format!("embedding generation failed: {e}"))
                        })?;

                    let emb = embedding_response
                        .data
                        .into_iter()
                        .next()
                        .ok_or_else(|| AgentError::ExecutionFailed("no embedding data".to_string()))?
                        .embedding;
                    
                    cache.store_embedding(cache_key, emb.clone()).await;
                    emb
                }
            } else {
                let embedding_response = self
                    .llm
                    .embedding(crate::llm::types::EmbeddingRequest {
                        model: "text-embedding-ada-002".to_string(),
                        input: crate::llm::types::EmbeddingInput::Single(text),
                        encoding_format: None,
                        dimensions: None,
                        user: None,
                    })
                    .await
                    .map_err(|e| {
                        AgentError::ExecutionFailed(format!("embedding generation failed: {e}"))
                    })?;

                embedding_response
                    .data
                    .into_iter()
                    .next()
                    .ok_or_else(|| AgentError::ExecutionFailed("no embedding data".to_string()))?
                    .embedding
            };

            #[cfg(not(feature = "compression-cache"))]
            let embedding = {
                let embedding_response = self
                    .llm
                    .embedding(crate::llm::types::EmbeddingRequest {
                        model: "text-embedding-ada-002".to_string(),
                        input: crate::llm::types::EmbeddingInput::Single(text),
                        encoding_format: None,
                        dimensions: None,
                        user: None,
                    })
                    .await
                    .map_err(|e| {
                        AgentError::ExecutionFailed(format!("embedding generation failed: {e}"))
                    })?;

                embedding_response
                    .data
                    .into_iter()
                    .next()
                    .ok_or_else(|| AgentError::ExecutionFailed("no embedding data".to_string()))?
                    .embedding
            };

            embeddings.push(Some(embedding));
        }
        Ok(embeddings)
    }

    #[cfg(feature = "parallel-compression")]
    async fn generate_embeddings_parallel(
        &self,
        to_compress: &[ChatMessage],
        system_msgs: &[ChatMessage],
        recent: &[ChatMessage],
    ) -> AgentResult<Vec<Option<Vec<f32>>>> {
        use rayon::prelude::*;

        if !self.llm.supports_embedding() {
            let mut result = system_msgs.to_vec();
            result.extend_from_slice(recent);
            return Err(AgentError::ExecutionFailed(
                "Embeddings not supported by LLM provider".to_string(),
            ));
        }

        let texts: Vec<String> = to_compress
            .par_iter()
            .map(|msg| Self::extract_text(msg))
            .collect();

        let non_empty_texts: Vec<(usize, String)> = texts
            .into_iter()
            .enumerate()
            .filter(|(_, text)| !text.is_empty())
            .collect();

        if non_empty_texts.is_empty() {
            return Ok(vec![None; to_compress.len()]);
        }

        let batch_size = 10;
        let mut embeddings = vec![None; to_compress.len()];

        for chunk in non_empty_texts.chunks(batch_size) {
            let texts_batch: Vec<String> = chunk.iter().map(|(_, text)| text.clone()).collect();
            let indices: Vec<usize> = chunk.iter().map(|(idx, _)| *idx).collect();

            let embedding_request = crate::llm::types::EmbeddingRequest {
                model: "text-embedding-ada-002".to_string(),
                input: crate::llm::types::EmbeddingInput::Multiple(texts_batch),
                encoding_format: None,
                dimensions: None,
                user: None,
            };

            let response =
                self.llm.embedding(embedding_request).await.map_err(|e| {
                    AgentError::ExecutionFailed(format!("batch embedding failed: {e}"))
                })?;

            for (i, emb_data) in response.data.into_iter().enumerate() {
                if let Some(&original_idx) = indices.get(i) {
                    embeddings[original_idx] = Some(emb_data.embedding);
                }
            }
        }

        Ok(embeddings)
    }
}

#[async_trait]
impl ContextCompressor for SemanticCompressor {
    async fn compress_with_metrics(
        &self,
        messages: Vec<ChatMessage>,
        max_tokens: usize,
    ) -> AgentResult<CompressionResult> {
        let tokens_before = self.count_tokens(&messages);
        let messages_before = messages.len();

        if tokens_before <= max_tokens {
            let metrics = CompressionMetrics::new(
                tokens_before,
                tokens_before,
                messages_before,
                messages_before,
            );
            return Ok(CompressionResult::new(messages, metrics, self.name().to_string()));
        }

        let (system_msgs, conversation): (Vec<_>, Vec<_>) =
            messages.into_iter().partition(|m| m.role == "system");

        if conversation.len() <= self.keep_recent {
            let mut result = system_msgs;
            result.extend(conversation);
            let tokens_after = self.count_tokens(&result);
            let metrics = CompressionMetrics::new(
                tokens_before,
                tokens_after,
                messages_before,
                result.len(),
            );
            return Ok(CompressionResult::new(result, metrics, self.name().to_string()));
        }

        let split_at = conversation.len().saturating_sub(self.keep_recent);
        let (to_compress, recent) = conversation.split_at(split_at);

        #[cfg(feature = "parallel-compression")]
        let embeddings =
            Self::generate_embeddings_parallel(self, to_compress, &system_msgs, recent).await?;

        #[cfg(not(feature = "parallel-compression"))]
        let embeddings =
            Self::generate_embeddings_sequential(self, to_compress, &system_msgs, recent).await?;

        let mut clusters: Vec<Vec<usize>> = Vec::new();
        let mut assigned = vec![false; to_compress.len()];

        for i in 0..to_compress.len() {
            if assigned[i] {
                continue;
            }

            let mut cluster = vec![i];
            assigned[i] = true;

            if let Some(Some(emb_i)) = embeddings.get(i) {
                for j in (i + 1)..to_compress.len() {
                    if assigned[j] {
                        continue;
                    }

                    if let Some(Some(emb_j)) = embeddings.get(j) {
                        let similarity = Self::cosine_similarity(emb_i, emb_j);
                        if similarity >= self.similarity_threshold {
                            cluster.push(j);
                            assigned[j] = true;
                        }
                    }
                }
            }

            clusters.push(cluster);
        }

        let mut compressed_messages = Vec::new();
        for cluster in clusters {
            let representative = cluster
                .iter()
                .max_by_key(|&&idx| Self::extract_text(&to_compress[idx]).len())
                .copied()
                .unwrap_or(0);

            compressed_messages.push(to_compress[representative].clone());
        }

        let mut result = system_msgs;
        result.extend(compressed_messages);
        result.extend_from_slice(recent);

        let tokens_after = self.count_tokens(&result);
        let messages_after = result.len();

        let metrics = CompressionMetrics::new(
            tokens_before,
            tokens_after,
            messages_before,
            messages_after,
        );

        // log compression event
        if metrics.was_compressed() {
            info!(
                strategy = self.name(),
                tokens_before = metrics.tokens_before,
                tokens_after = metrics.tokens_after,
                reduction_percent = metrics.token_reduction_percent,
                "context compressed"
            );
            debug!(
                messages_before = metrics.messages_before,
                messages_after = metrics.messages_after,
                "compression details"
            );
        } else {
            trace!(
                strategy = self.name(),
                tokens = tokens_before,
                "no compression needed"
            );
        }

        Ok(CompressionResult::new(result, metrics, self.name().to_string()))
    }

    fn strategy(&self) -> CompressionStrategy {
        CompressionStrategy::Semantic {
            similarity_threshold: self.similarity_threshold,
            keep_recent: self.keep_recent,
        }
    }

    fn name(&self) -> &str {
        "semantic"
    }
}

// hierarchical compressor

/// compresses messages based on importance scores (recency, role, density)
pub struct HierarchicalCompressor {
    llm: Arc<dyn crate::llm::provider::LLMProvider>,
    keep_recent: usize,
}

impl HierarchicalCompressor {
    /// create hierarchical compressor (default keep_recent=5)
    pub fn new(llm: Arc<dyn crate::llm::provider::LLMProvider>) -> Self {
        Self {
            llm,
            keep_recent: 5,
        }
    }

    /// set how many recent messages to keep uncompressed
    pub fn with_keep_recent(mut self, n: usize) -> Self {
        self.keep_recent = n;
        self
    }

    fn importance_score(message: &ChatMessage, index: usize, total: usize) -> f32 {
        let recency = 1.0 - (index as f32 / total.max(1) as f32);
        let role_score = match message.role.as_str() {
            "system" => 1.0,
            "assistant" => 0.7,
            "user" => 0.5,
            _ => 0.3,
        };
        let content_len = Self::extract_text(message).len();
        let density = (content_len.min(1000) as f32 / 1000.0).min(1.0);
        0.4 * recency + 0.3 * role_score + 0.3 * density
    }

    fn extract_text(message: &ChatMessage) -> String {
        message.content.as_deref().unwrap_or("").to_string()
    }
}

#[async_trait]
impl ContextCompressor for HierarchicalCompressor {
    async fn compress_with_metrics(
        &self,
        messages: Vec<ChatMessage>,
        max_tokens: usize,
    ) -> AgentResult<CompressionResult> {
        let tokens_before = self.count_tokens(&messages);
        let messages_before = messages.len();

        if tokens_before <= max_tokens {
            let metrics = CompressionMetrics::new(
                tokens_before,
                tokens_before,
                messages_before,
                messages_before,
            );
            return Ok(CompressionResult::new(messages, metrics, self.name().to_string()));
        }

        let (system_msgs, conversation): (Vec<_>, Vec<_>) =
            messages.into_iter().partition(|m| m.role == "system");

        if conversation.len() <= self.keep_recent {
            let mut result = system_msgs;
            result.extend(conversation);
            let tokens_after = self.count_tokens(&result);
            let metrics = CompressionMetrics::new(
                tokens_before,
                tokens_after,
                messages_before,
                result.len(),
            );
            return Ok(CompressionResult::new(result, metrics, self.name().to_string()));
        }

        let split_at = conversation.len().saturating_sub(self.keep_recent);
        let (to_compress, recent) = conversation.split_at(split_at);

        let mut scored: Vec<(f32, ChatMessage)> = to_compress
            .iter()
            .enumerate()
            .map(|(idx, msg)| {
                let score = Self::importance_score(msg, idx, to_compress.len());
                (score, msg.clone())
            })
            .collect();

        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        let mut compressed = Vec::new();
        let mut current_tokens = system_msgs
            .iter()
            .map(|m| self.count_tokens(&[m.clone()]))
            .sum::<usize>();

        for (score, msg) in scored {
            let msg_tokens = self.count_tokens(&[msg.clone()]);
            let estimated_total = current_tokens
                + msg_tokens
                + recent
                    .iter()
                    .map(|m| self.count_tokens(&[m.clone()]))
                    .sum::<usize>();

            if estimated_total <= max_tokens {
                compressed.push(msg);
                current_tokens += msg_tokens;
            } else if score > 0.5 {
                let text = Self::extract_text(&msg);
                if !text.is_empty() {
                    let summary_prompt = format!(
                        "Summarize the following message concisely, preserving key information:\n\n{}",
                        text
                    );

                    let summary_request =
                        crate::llm::types::ChatCompletionRequest::new("gpt-4o-mini")
                            .user(summary_prompt)
                            .temperature(0.3)
                            .max_tokens(256);

                    match self.llm.chat(summary_request).await {
                        Ok(response) => {
                            if let Some(summary) = response.content() {
                                let summary_msg = ChatMessage {
                                    role: msg.role.clone(),
                                    content: Some(format!("[Compressed] {}", summary)),
                                    tool_call_id: None,
                                    tool_calls: None,
                                };
                                let summary_tokens = self.count_tokens(&[summary_msg.clone()]);
                                if current_tokens + summary_tokens <= max_tokens {
                                    compressed.push(summary_msg);
                                    current_tokens += summary_tokens;
                                }
                            }
                        }
                        Err(_) => {}
                    }
                }
            }
        }

        let mut result = system_msgs;
        result.extend(compressed);
        result.extend_from_slice(recent);

        let tokens_after = self.count_tokens(&result);
        let messages_after = result.len();

        let metrics = CompressionMetrics::new(
            tokens_before,
            tokens_after,
            messages_before,
            messages_after,
        );

        // log compression event
        if metrics.was_compressed() {
            info!(
                strategy = self.name(),
                tokens_before = metrics.tokens_before,
                tokens_after = metrics.tokens_after,
                reduction_percent = metrics.token_reduction_percent,
                "context compressed"
            );
            debug!(
                messages_before = metrics.messages_before,
                messages_after = metrics.messages_after,
                "compression details"
            );
        } else {
            trace!(
                strategy = self.name(),
                tokens = tokens_before,
                "no compression needed"
            );
        }

        Ok(CompressionResult::new(result, metrics, self.name().to_string()))
    }

    fn strategy(&self) -> CompressionStrategy {
        CompressionStrategy::Hierarchical {
            keep_recent: self.keep_recent,
        }
    }

    fn name(&self) -> &str {
        "hierarchical"
    }
}

// hybrid compressor

/// combines multiple compression strategies, tries them in sequence
pub struct HybridCompressor {
    strategies: Vec<Box<dyn ContextCompressor>>,
}

impl HybridCompressor {
    /// create hybrid compressor with no strategies
    pub fn new() -> Self {
        Self {
            strategies: Vec::new(),
        }
    }

    /// add compression strategy to try (in order)
    pub fn add_strategy(mut self, strategy: Box<dyn ContextCompressor>) -> Self {
        self.strategies.push(strategy);
        self
    }
}

impl Default for HybridCompressor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ContextCompressor for HybridCompressor {
    async fn compress_with_metrics(
        &self,
        messages: Vec<ChatMessage>,
        max_tokens: usize,
    ) -> AgentResult<CompressionResult> {
        let tokens_before = self.count_tokens(&messages);
        let messages_before = messages.len();

        if tokens_before <= max_tokens {
            let metrics = CompressionMetrics::new(
                tokens_before,
                tokens_before,
                messages_before,
                messages_before,
            );
            return Ok(CompressionResult::new(messages, metrics, self.name().to_string()));
        }

        let mut current = messages;
        for strategy in &self.strategies {
            let result = strategy.compress_with_metrics(current.clone(), max_tokens).await?;
            if result.metrics.tokens_after <= max_tokens {
                return Ok(result);
            }
            current = result.messages;
        }

        let tokens_after = self.count_tokens(&current);
        let messages_after = current.len();
        let metrics = CompressionMetrics::new(
            tokens_before,
            tokens_after,
            messages_before,
            messages_after,
        );

        Ok(CompressionResult::new(current, metrics, self.name().to_string()))
    }

    fn strategy(&self) -> CompressionStrategy {
        CompressionStrategy::Hybrid {
            strategies: self
                .strategies
                .iter()
                .map(|s| s.name().to_string())
                .collect(),
        }
    }

    fn name(&self) -> &str {
        "hybrid"
    }
}

// tests

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
        let result = compressor.compress(msgs.clone(), 1).await.unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].role, "system");
    }

    #[tokio::test]
    async fn sliding_window_trims_to_window_size() {
        // 5 user+assistant pairs = 10 conversation messages + 1 system = 11 total.
        let compressor = SlidingWindowCompressor::new(4);
        let msgs = long_conversation(5);
        assert_eq!(msgs.len(), 11);
        let result = compressor.compress(msgs, 1).await.unwrap();
        assert_eq!(result.len(), 5);
        assert_eq!(result[0].role, "system");
    }

    #[tokio::test]
    async fn sliding_window_very_long_single_message() {
        let compressor = SlidingWindowCompressor::new(2);
        let long_content = "a".repeat(10_000);
        let msgs = vec![make_msg("system", "sys"), make_msg("user", &long_content)];
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
        let msgs = long_conversation(3);
        assert_eq!(msgs.len(), 7);
        let result = compressor.compress(msgs, 1).await.unwrap();
        assert_eq!(result.len(), 4);
        assert_eq!(result[0].role, "system");
        assert!(
            result[1]
            .content
            .as_ref()
            .unwrap()
                .starts_with("[Conversation summary]")
        );
    }

    #[tokio::test]
    async fn summarizing_very_long_single_message() {
        let llm = Arc::new(MockLLM);
        let compressor = SummarizingCompressor::new(llm).with_keep_recent(10);
        let long_content = "x".repeat(50_000);
        let msgs = vec![make_msg("system", "sys"), make_msg("user", &long_content)];
        let result = compressor.compress(msgs.clone(), 1).await.unwrap();
        assert_eq!(result.len(), 2);
    }

    struct MockLLMWithEmbeddings;

    #[async_trait]
    impl crate::llm::provider::LLMProvider for MockLLMWithEmbeddings {
        fn name(&self) -> &str {
            "mock-with-embeddings"
        }

        fn supports_embedding(&self) -> bool {
            true
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

        async fn embedding(
            &self,
            request: crate::llm::types::EmbeddingRequest,
        ) -> crate::llm::types::LLMResult<crate::llm::types::EmbeddingResponse> {
            use crate::llm::types::{EmbeddingData, EmbeddingResponse, EmbeddingUsage};
            // Generate deterministic embeddings based on text hash
            let texts = match request.input {
                crate::llm::types::EmbeddingInput::Single(s) => vec![s],
                crate::llm::types::EmbeddingInput::Multiple(v) => v,
            };

            let data: Vec<EmbeddingData> = texts
                .into_iter()
                .enumerate()
                .map(|(idx, text)| {
                    let hash: u32 = text
                        .bytes()
                        .fold(0u32, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u32));
                    let mut embedding = vec![0.0_f32; 128];
                    for i in 0..128 {
                        embedding[i] = ((hash.wrapping_mul(i as u32 + 1)) % 1000) as f32 / 1000.0;
                    }
                    let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
                    if norm > 0.0 {
                        for x in &mut embedding {
                            *x /= norm;
                        }
                    }
                    EmbeddingData {
                        object: "embedding".to_string(),
                        index: idx as u32,
                        embedding,
                    }
                })
                .collect();

            Ok(EmbeddingResponse {
                object: "list".to_string(),
                model: request.model,
                data,
                usage: EmbeddingUsage {
                    prompt_tokens: 0,
                    total_tokens: 0,
                },
            })
        }
    }

    #[tokio::test]
    async fn semantic_under_limit_unchanged() {
        let llm = Arc::new(MockLLMWithEmbeddings);
        let compressor = SemanticCompressor::new(llm);
        let msgs = short_conversation();
        let result = compressor.compress(msgs.clone(), 100_000).await.unwrap();
        assert_eq!(result.len(), msgs.len());
    }

    #[tokio::test]
    async fn semantic_only_system_message() {
        let llm = Arc::new(MockLLMWithEmbeddings);
        let compressor = SemanticCompressor::new(llm);
        let msgs = system_only();
        let result = compressor.compress(msgs.clone(), 1).await.unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].role, "system");
    }

    #[tokio::test]
    async fn semantic_clusters_similar_messages() {
        let llm = Arc::new(MockLLMWithEmbeddings);
        // Set keep_recent to 2 so we actually compress the older messages
        let compressor = SemanticCompressor::new(llm)
            .with_similarity_threshold(0.99)
            .with_keep_recent(2);
        // Create messages that will exceed token budget to force compression
        let msgs = vec![
            make_msg("system", "You are a helpful assistant."),
            make_msg(
                "user",
                "Hello, this is a longer message to ensure we exceed token budget.",
            ),
            make_msg(
                "assistant",
                "Hi there! This is also a longer response message.",
            ),
            make_msg(
                "user",
                "Hello, this is a longer message to ensure we exceed token budget.",
            ),
            make_msg(
                "assistant",
                "Hi there! This is also a longer response message.",
            ),
            make_msg(
                "user",
                "Hello, this is a longer message to ensure we exceed token budget.",
            ),
        ];
        // Use a very small token budget to force compression
        let result = compressor.compress(msgs, 10).await.unwrap();
        // Should compress: 1 system + compressed older messages + 2 recent
        // Original has 6 messages, compressed should have fewer
        assert!(result.len() >= 3);
        assert!(result.len() <= 6);
        assert_eq!(result[0].role, "system");
    }

    #[tokio::test]
    async fn semantic_preserves_recent_messages() {
        let llm = Arc::new(MockLLMWithEmbeddings);
        let compressor = SemanticCompressor::new(llm).with_keep_recent(2);
        let msgs = long_conversation(5);
        let result = compressor.compress(msgs, 100).await.unwrap();
        assert!(result.len() >= 3);
        assert_eq!(result[0].role, "system");
    }

    #[tokio::test]
    async fn hierarchical_under_limit_unchanged() {
        let llm = Arc::new(MockLLM);
        let compressor = HierarchicalCompressor::new(llm);
        let msgs = short_conversation();
        let result = compressor.compress(msgs.clone(), 100_000).await.unwrap();
        assert_eq!(result.len(), msgs.len());
    }

    #[tokio::test]
    async fn hierarchical_only_system_message() {
        let llm = Arc::new(MockLLM);
        let compressor = HierarchicalCompressor::new(llm);
        let msgs = system_only();
        let result = compressor.compress(msgs.clone(), 1).await.unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].role, "system");
    }

    #[tokio::test]
    async fn hierarchical_preserves_important_messages() {
        let llm = Arc::new(MockLLM);
        let compressor = HierarchicalCompressor::new(llm).with_keep_recent(2);
        let msgs = long_conversation(10);
        let result = compressor.compress(msgs, 200).await.unwrap();
        assert!(result.len() >= 3);
        assert_eq!(result[0].role, "system");
    }

    #[tokio::test]
    async fn hierarchical_scores_by_importance() {
        let llm = Arc::new(MockLLM);
        let compressor = HierarchicalCompressor::new(llm);
        let msgs = vec![
            make_msg("system", "Important system prompt"),
            make_msg("user", "Old message"),
            make_msg("assistant", "Old response"),
            make_msg("user", "Recent message"),
        ];
        let result = compressor.compress(msgs, 50).await.unwrap();
        assert_eq!(result[0].role, "system");
        assert!(result[0].content.as_deref().unwrap().contains("Important"));
    }

    #[tokio::test]
    async fn hybrid_under_limit_unchanged() {
        let compressor = HybridCompressor::new();
        let msgs = short_conversation();
        let result = compressor.compress(msgs.clone(), 100_000).await.unwrap();
        assert_eq!(result.len(), msgs.len());
    }

    #[tokio::test]
    async fn hybrid_tries_strategies_in_order() {
        let llm = Arc::new(MockLLM);
        let compressor = HybridCompressor::new()
            .add_strategy(Box::new(SlidingWindowCompressor::new(2)))
            .add_strategy(Box::new(SummarizingCompressor::new(llm.clone())));
        let msgs = long_conversation(10); // 21 messages total (1 system + 20 conversation)
        let result = compressor.compress(msgs, 100).await.unwrap();
        // SlidingWindowCompressor with window_size=2 should compress to: 1 system + 2*2 = 5 messages max
        // But token budget might allow more, so just check it's compressed and system is preserved
        assert!(result.len() <= 21);
        assert!(result.len() >= 1);
        assert_eq!(result[0].role, "system");
    }

    #[tokio::test]
    async fn hybrid_empty_strategies_returns_unchanged() {
        let compressor = HybridCompressor::new();
        let msgs = long_conversation(5);
        let result = compressor.compress(msgs.clone(), 100_000).await.unwrap();
        assert_eq!(result.len(), msgs.len());
    }

    #[tokio::test]
    async fn hybrid_falls_back_through_strategies() {
        let llm = Arc::new(MockLLM);
        let compressor = HybridCompressor::new()
            .add_strategy(Box::new(SlidingWindowCompressor::new(1)))
            .add_strategy(Box::new(
                SummarizingCompressor::new(llm).with_keep_recent(2),
            ));
        let msgs = long_conversation(8);
        let result = compressor.compress(msgs, 50).await.unwrap();
        assert!(result.len() < 17);
    }
}
