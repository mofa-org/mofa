//! compression caching example - demonstrating cache usage for performance

#[cfg(feature = "compression-cache")]
use mofa_foundation::agent::{CompressionCache, ContextCompressor, SemanticCompressor, TokenCounter};
#[cfg(feature = "compression-cache")]
use crate::helpers::{make_msg, print_compression_stats};
#[cfg(feature = "compression-cache")]
use std::sync::Arc;

#[cfg(feature = "compression-cache")]
pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    println!("\nscenario: compression caching");
    println!("use case: cache embeddings and summaries to reduce API calls\n");

    // create shared cache
    let cache = Arc::new(CompressionCache::new(1000, 500));

    // simulate repeated similar messages (common in customer support)
    let conversation1 = vec![
        make_msg("system", "You are a helpful assistant."),
        make_msg("user", "How do I return my order?"),
        make_msg("assistant", "You can return your order by logging into your account."),
        make_msg("user", "What's the return policy?"),
        make_msg("assistant", "We offer full refunds within 30 days."),
    ];

    let conversation2 = vec![
        make_msg("system", "You are a helpful assistant."),
        make_msg("user", "How do I return my order?"), // Same message - will use cache
        make_msg("assistant", "You can return your order by logging into your account."),
        make_msg("user", "What's the return policy?"), // Same message - will use cache
        make_msg("assistant", "We offer full refunds within 30 days."),
    ];

    println!("note: caching example requires a real LLM provider with embedding support");
    println!("this example demonstrates the API usage pattern\n");
    
    println!("example usage:");
    println!(r#"
use mofa_foundation::agent::{CompressionCache, SemanticCompressor};
use std::sync::Arc;

let cache = Arc::new(CompressionCache::new(1000, 500));
let compressor = SemanticCompressor::new(llm_provider)
    .with_cache(cache.clone())
    .with_similarity_threshold(0.85);

let result = compressor.compress_with_metrics(messages, 4000).await?;
"#);

    let stats = cache.stats().await;
    println!("\ncache statistics:");
    println!("  embedding entries: {}/{}", stats.embedding_entries, stats.max_embedding_entries);
    println!("  summary entries: {}/{}", stats.summary_entries, stats.max_summary_entries);

    println!("\nbenefits:");
    println!("  - second compression uses cached embeddings (faster)");
    println!("  - reduces API calls for repeated content");
    println!("  - automatic LRU eviction when cache is full");

    Ok(())
}

#[cfg(not(feature = "compression-cache"))]
pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    println!("\ncompression caching example requires 'compression-cache' feature");
    println!("enable it in Cargo.toml: features = [\"compression-cache\"]");
    Ok(())
}
