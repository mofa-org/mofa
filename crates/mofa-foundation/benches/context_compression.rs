//! Benchmarks for context compression strategies
//!
//! Run with: `cargo bench --package mofa-foundation --bench context_compression`

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use mofa_foundation::agent::components::{
    ContextCompressor, HierarchicalCompressor, HybridCompressor, SemanticCompressor,
    SlidingWindowCompressor, SummarizingCompressor, TokenCounter,
};
use mofa_kernel::agent::types::ChatMessage;
use std::sync::Arc;

fn make_msg(role: &str, content: &str) -> ChatMessage {
    ChatMessage {
        role: role.to_string(),
        content: Some(content.to_string()),
        tool_call_id: None,
        tool_calls: None,
    }
}

fn build_conversation(size: usize) -> Vec<ChatMessage> {
    let mut msgs = vec![make_msg(
        "system",
        "You are a helpful assistant specializing in Rust programming.",
    )];
    for i in 0..size {
        msgs.push(make_msg(
            "user",
            &format!("Question {}: What is ownership in Rust?", i),
        ));
        msgs.push(make_msg(
            "assistant",
            &format!(
                "Answer {}: Ownership is Rust's memory management system...",
                i
            ),
        ));
    }
    msgs
}

// Mock LLM for testing (doesn't make real API calls)
struct MockLLM;

#[async_trait::async_trait]
impl mofa_foundation::llm::provider::LLMProvider for MockLLM {
    fn name(&self) -> &str {
        "mock"
    }

    fn supports_embedding(&self) -> bool {
        true
    }

    async fn chat(
        &self,
        _request: mofa_foundation::llm::types::ChatCompletionRequest,
    ) -> mofa_foundation::llm::types::LLMResult<mofa_foundation::llm::types::ChatCompletionResponse>
    {
        use mofa_foundation::llm::types::{
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
                    content: Some(MessageContent::Text("Summary text".to_string())),
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
        request: mofa_foundation::llm::types::EmbeddingRequest,
    ) -> mofa_foundation::llm::types::LLMResult<mofa_foundation::llm::types::EmbeddingResponse>
    {
        use mofa_foundation::llm::types::{EmbeddingData, EmbeddingResponse, EmbeddingUsage};
        let texts = match request.input {
            mofa_foundation::llm::types::EmbeddingInput::Single(s) => vec![s],
            mofa_foundation::llm::types::EmbeddingInput::Multiple(v) => v,
        };

        let data: Vec<EmbeddingData> = texts
            .into_iter()
            .enumerate()
            .map(|(idx, text)| {
                // Deterministic embedding based on text hash
                let hash: u32 = text
                    .bytes()
                    .fold(0u32, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u32));
                let mut embedding = vec![0.0_f32; 128];
                for i in 0..128 {
                    embedding[i] = ((hash.wrapping_mul(i as u32 + 1)) % 1000) as f32 / 1000.0;
                }
                // Normalize
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

fn bench_token_counting(c: &mut Criterion) {
    let mut group = c.benchmark_group("token_counting");

    for size in [10, 50, 100, 500].iter() {
        let messages = build_conversation(*size);
        group.bench_with_input(BenchmarkId::new("heuristic", size), &messages, |b, msgs| {
            b.iter(|| TokenCounter::count(black_box(msgs)));
        });
    }

    group.finish();
}

fn bench_sliding_window(c: &mut Criterion) {
    let mut group = c.benchmark_group("sliding_window");

    for size in [10, 50, 100].iter() {
        let messages = build_conversation(*size);
        let compressor = SlidingWindowCompressor::new(10);
        let budget = TokenCounter::count(&messages) / 2;

        group.bench_with_input(BenchmarkId::new("compress", size), &messages, |b, msgs| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            b.iter(|| {
                rt.block_on(compressor.compress(black_box(msgs.clone()), budget))
                    .unwrap()
            });
        });
    }

    group.finish();
}

fn bench_semantic_compression(c: &mut Criterion) {
    let mut group = c.benchmark_group("semantic_compression");

    let llm = Arc::new(MockLLM);

    for size in [10, 20, 30].iter() {
        let messages = build_conversation(*size);
        let compressor = SemanticCompressor::new(llm.clone())
            .with_similarity_threshold(0.85)
            .with_keep_recent(5);
        let budget = TokenCounter::count(&messages) / 2;

        group.bench_with_input(BenchmarkId::new("compress", size), &messages, |b, msgs| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            b.iter(|| {
                rt.block_on(compressor.compress(black_box(msgs.clone()), budget))
                    .unwrap()
            });
        });
    }

    group.finish();
}

fn bench_hierarchical_compression(c: &mut Criterion) {
    let mut group = c.benchmark_group("hierarchical_compression");

    let llm = Arc::new(MockLLM);

    for size in [10, 20, 30].iter() {
        let messages = build_conversation(*size);
        let compressor = HierarchicalCompressor::new(llm.clone()).with_keep_recent(5);
        let budget = TokenCounter::count(&messages) / 2;

        group.bench_with_input(BenchmarkId::new("compress", size), &messages, |b, msgs| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            b.iter(|| {
                rt.block_on(compressor.compress(black_box(msgs.clone()), budget))
                    .unwrap()
            });
        });
    }

    group.finish();
}

fn bench_hybrid_compression(c: &mut Criterion) {
    let mut group = c.benchmark_group("hybrid_compression");

    let llm = Arc::new(MockLLM);

    for size in [10, 20, 30].iter() {
        let messages = build_conversation(*size);
        let compressor = HybridCompressor::new()
            .add_strategy(Box::new(SlidingWindowCompressor::new(10)))
            .add_strategy(Box::new(SummarizingCompressor::new(llm.clone())));
        let budget = TokenCounter::count(&messages) / 2;

        group.bench_with_input(BenchmarkId::new("compress", size), &messages, |b, msgs| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            b.iter(|| {
                rt.block_on(compressor.compress(black_box(msgs.clone()), budget))
                    .unwrap()
            });
        });
    }

    group.finish();
}

fn bench_compression_ratios(c: &mut Criterion) {
    let mut group = c.benchmark_group("compression_ratios");

    let messages = build_conversation(50);
    let tokens_before = TokenCounter::count(&messages);
    let budget = tokens_before / 2;

    let llm = Arc::new(MockLLM);

    let compressors = vec![
        (
            "sliding_window",
            Box::new(SlidingWindowCompressor::new(10))
                as Box<dyn mofa_kernel::agent::components::context_compressor::ContextCompressor>,
        ),
        (
            "semantic",
            Box::new(SemanticCompressor::new(llm.clone()).with_similarity_threshold(0.85)),
        ),
        (
            "hierarchical",
            Box::new(HierarchicalCompressor::new(llm.clone())),
        ),
    ];

    for (name, compressor) in compressors {
        group.bench_function(name, |b| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            b.iter(|| {
                let compressed = rt
                    .block_on(compressor.compress(messages.clone(), budget))
                    .unwrap();
                let tokens_after = TokenCounter::count(&compressed);
                let ratio = tokens_after as f64 / tokens_before as f64;
                black_box(ratio);
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_token_counting,
    bench_sliding_window,
    bench_semantic_compression,
    bench_hierarchical_compression,
    bench_hybrid_compression,
    bench_compression_ratios
);
criterion_main!(benches);
