# Context Compression Guide

## Overview

MoFA provides powerful context compression capabilities to manage conversation length when interacting with LLMs. When accumulated context exceeds token limits, compressors intelligently trim or summarize message history so agents can run indefinitely.

## Quick Start

```rust
use mofa_foundation::agent::{ContextCompressor, SlidingWindowCompressor};

let compressor = SlidingWindowCompressor::new(10);
let compressed = compressor.compress(messages, 4000).await?;
```

## Compression Strategies

### 1. Sliding Window

**Best for**: Simple truncation, fast performance

Keeps the system prompt plus the N most recent messages.

```rust
use mofa_foundation::agent::SlidingWindowCompressor;

let compressor = SlidingWindowCompressor::new(10); // Keep last 10 messages
let result = compressor.compress(messages, 4000).await?;
```

**Characteristics**:
- Fastest (<1ms)
- Simple and predictable
- Loses older context completely

### 2. Summarizing

**Best for**: Long conversations, preserving semantic meaning

Uses the LLM to summarize older messages while keeping recent ones intact.

```rust
use mofa_foundation::agent::SummarizingCompressor;
use std::sync::Arc;

let compressor = SummarizingCompressor::new(llm_provider)
    .with_keep_recent(5); // Keep last 5 messages uncompressed
let result = compressor.compress(messages, 4000).await?;
```

**Characteristics**:
- Preserves semantic meaning
- Creates summary messages
- Slower (200ms-1s)
- Uses LLM API calls

### 3. Semantic

**Best for**: Repetitive conversations, removing redundancy

Uses embeddings to identify and merge similar messages.

```rust
use mofa_foundation::agent::SemanticCompressor;

let compressor = SemanticCompressor::new(llm_provider)
    .with_similarity_threshold(0.85) // Merge messages >85% similar
    .with_keep_recent(5);
let result = compressor.compress(messages, 4000).await?;
```

**Characteristics**:
- Removes redundant information
- Preserves diverse information
- Moderate speed (100-500ms)
- Uses embedding API calls

### 4. Hierarchical

**Best for**: Technical discussions, preserving important details

Scores messages by importance (recency, role, information density) and compresses less important ones.

```rust
use mofa_foundation::agent::HierarchicalCompressor;

let compressor = HierarchicalCompressor::new(llm_provider)
    .with_keep_recent(5);
let result = compressor.compress(messages, 4000).await?;
```

**Characteristics**:
- Smart importance scoring
- Preserves critical information
- Slower (200ms-1s)
- Uses LLM API calls

### 5. Hybrid

**Best for**: Unpredictable patterns, adaptive compression

Tries multiple strategies in sequence until the token budget is met.

```rust
use mofa_foundation::agent::{HybridCompressor, SlidingWindowCompressor, SummarizingCompressor};

let compressor = HybridCompressor::new()
    .add_strategy(Box::new(SlidingWindowCompressor::new(10)))
    .add_strategy(Box::new(SummarizingCompressor::new(llm_provider)));
let result = compressor.compress(messages, 4000).await?;
```

**Characteristics**:
- Adaptive fallback
- Optimal compression
- Variable speed
- May use multiple API calls

## Compression Metrics

All compressors return detailed metrics about the compression operation:

```rust
use mofa_foundation::agent::ContextCompressor;

let result = compressor.compress_with_metrics(messages, 4000).await?;

println!("Tokens: {} → {} ({:.1}% reduction)", 
    result.metrics.tokens_before,
    result.metrics.tokens_after,
    result.metrics.token_reduction_percent);

println!("Messages: {} → {} ({:.1}% reduction)",
    result.metrics.messages_before,
    result.metrics.messages_after,
    result.metrics.message_reduction_percent);

println!("Compression ratio: {:.2}", result.metrics.compression_ratio);
println!("Strategy used: {}", result.strategy_name);
```

### Metrics Fields

- `tokens_before`: Token count before compression
- `tokens_after`: Token count after compression
- `messages_before`: Message count before compression
- `messages_after`: Message count after compression
- `compression_ratio`: Ratio of tokens_after / tokens_before (0.0-1.0)
- `token_reduction_percent`: Percentage reduction in tokens (0.0-100.0)
- `message_reduction_percent`: Percentage reduction in messages (0.0-100.0)
- `was_compressed()`: Whether compression actually occurred
- `tokens_saved()`: Number of tokens saved

## Choosing a Strategy

| Scenario | Recommended Strategy | Reason |
|----------|---------------------|--------|
| Fast, simple truncation | Sliding Window | Fastest, predictable |
| Long conversations | Summarizing | Preserves semantic meaning |
| Repetitive queries | Semantic | Removes redundancy |
| Technical discussions | Hierarchical | Preserves important details |
| Unpredictable patterns | Hybrid | Adaptive fallback |

## Performance Comparison

| Strategy | Speed | API Calls | Token Reduction | Quality |
|----------|-------|-----------|-----------------|---------|
| Sliding Window | <1ms | 0 | High | Low |
| Summarizing | 200ms-1s | 1+ | Medium | High |
| Semantic | 100-500ms | 1+ | Medium | Medium |
| Hierarchical | 200ms-1s | 1+ | Medium | High |
| Hybrid | Variable | Variable | Variable | High |

## Best Practices

1. **Always preserve system messages**: All compressors automatically preserve system prompts
2. **Monitor compression metrics**: Use `compress_with_metrics()` to track effectiveness
3. **Choose appropriate keep_recent**: Balance between compression and context preservation
4. **Use Hybrid for production**: Provides best adaptive behavior
5. **Cache embeddings**: Enable `parallel-compression` feature for better performance

## Examples

See `examples/compression_practical/` for comprehensive examples:
- Customer support agent
- Code review assistant
- Research assistant
- E-commerce chatbot
- Hybrid strategy
- Real-world integration
- Performance comparison

## Compression Caching

Enable caching to reduce API calls and improve performance:

```rust
use mofa_foundation::agent::{CompressionCache, SemanticCompressor};
use std::sync::Arc;

// Create shared cache
let cache = Arc::new(CompressionCache::new(1000, 500)); // 1000 embeddings, 500 summaries

// Use cache with compressor
let compressor = SemanticCompressor::new(llm_provider)
    .with_cache(cache.clone())
    .with_similarity_threshold(0.85);

let result = compressor.compress(messages, 4000).await?;
```

**Benefits**:
- Reduces API calls for repeated content
- Faster compression for similar messages
- Configurable cache sizes
- Automatic LRU eviction

**Enable in Cargo.toml**:
```toml
[dependencies]
mofa-foundation = { path = "../..", features = ["compression-cache"] }
```

## Advanced Usage

### Custom Token Counting

```rust
use mofa_foundation::agent::TikTokenCounter;

let counter = TikTokenCounter::cl100k_base()?;
let tokens = counter.count(&messages);
```

### Parallel Processing

Enable the `parallel-compression` feature for faster embedding generation:

```toml
[dependencies]
mofa-foundation = { path = "../..", features = ["parallel-compression"] }
```

### Integration with AgentExecutor

```rust
use mofa_foundation::agent::{AgentExecutor, SemanticCompressor};

let compressor = Arc::new(
    SemanticCompressor::new(llm_provider.clone())
        .with_similarity_threshold(0.80)
        .with_keep_recent(5)
);

let executor = AgentExecutor::with_config(...)
    .with_compressor(compressor);
```

## Troubleshooting

**Problem**: Compression not working
- **Solution**: Check that `max_tokens` is less than current token count

**Problem**: Too much information lost
- **Solution**: Increase `keep_recent` parameter or use Summarizing/Hierarchical strategies

**Problem**: Compression too slow
- **Solution**: Use SlidingWindow for speed, or enable `parallel-compression` feature

**Problem**: API costs too high
- **Solution**: Use SlidingWindow (no API calls) or reduce compression frequency

## See Also

- [API Reference](../mofa-doc/api-reference/foundation/context-compression.md)
- [Architecture Guide](./architecture.md)
- [Examples](../examples/compression_practical/)
