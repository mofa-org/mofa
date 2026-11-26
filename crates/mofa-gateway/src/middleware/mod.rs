//! Gateway middleware modules

pub mod rate_limit;
pub mod semantic_cache;

pub use rate_limit::RateLimiter;
pub use semantic_cache::{
    CacheHit, CacheStats, CachedItem, LlmPromptEmbedder, MockPromptEmbedder, PromptEmbedder,
    SemanticCache, SemanticCacheConfig, cosine_similarity,
};
