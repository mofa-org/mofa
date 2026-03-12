//! RAG Document Indexing — Integration Example
//!
//! Demonstrates how the **indexing pipeline** (`index_documents`) integrates
//! with other MoFA components end-to-end:
//!
//! 1. A mock `LLMProvider` (stands in for OpenAI / Ollama)
//! 2. `LlmEmbeddingAdapter` — the foundation adapter from `embedding_adapter`
//! 3. `InMemoryVectorStore` — kernel `VectorStore` implementation
//! 4. `index_documents` — the high-level orchestration function
//!
//! After indexing, the example verifies that chunks are searchable inside the
//! vector store, proving the pipeline is not an isolated module but connects
//! all MoFA layers (kernel traits → foundation implementations → user code).
//!
//! # Running
//!
//! ```bash
//! cargo run -p rag_indexing_demo
//! ```

use mofa_foundation::llm::client::LLMClient;
use mofa_foundation::llm::provider::LLMProvider;
use mofa_foundation::llm::types::{
    ChatCompletionRequest, ChatCompletionResponse, EmbeddingData, EmbeddingInput,
    EmbeddingRequest, EmbeddingResponse, EmbeddingUsage, LLMError, LLMResult,
};
use mofa_foundation::rag::embedding_adapter::{LlmEmbeddingAdapter, RagEmbeddingConfig};
use mofa_foundation::rag::indexing::{IndexDocument, IndexMode, RagIndexConfig, index_documents};
use mofa_foundation::rag::InMemoryVectorStore;
use mofa_kernel::rag::VectorStore;
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Mock LLM provider (simulates OpenAI / Ollama embeddings without network)
// ---------------------------------------------------------------------------

/// A lightweight mock provider that produces deterministic embeddings from
/// text content.  In production this would be `OpenAIProvider` or
/// `OllamaProvider`, but for this integration demo we avoid network deps.
struct MockEmbeddingProvider {
    dimensions: usize,
}

#[async_trait::async_trait]
impl LLMProvider for MockEmbeddingProvider {
    fn name(&self) -> &str {
        "mock-embedding"
    }
    fn default_model(&self) -> &str {
        "mock-embed-v1"
    }
    fn supports_streaming(&self) -> bool {
        false
    }
    fn supports_tools(&self) -> bool {
        false
    }
    fn supports_vision(&self) -> bool {
        false
    }

    async fn chat(&self, _req: ChatCompletionRequest) -> LLMResult<ChatCompletionResponse> {
        Err(LLMError::Other("chat not supported in embedding provider".into()))
    }
    async fn chat_stream(
        &self,
        _req: ChatCompletionRequest,
    ) -> LLMResult<mofa_foundation::llm::provider::ChatStream> {
        Err(LLMError::Other("streaming not supported".into()))
    }

    /// Produce a deterministic embedding vector from text content.
    ///
    /// The vector is derived from byte values of the input so that
    /// semantically similar texts produce similar vectors.
    async fn embedding(&self, request: EmbeddingRequest) -> LLMResult<EmbeddingResponse> {
        let inputs = match request.input {
            EmbeddingInput::Single(s) => vec![s],
            EmbeddingInput::Multiple(v) => v,
        };

        let data: Vec<EmbeddingData> = inputs
            .iter()
            .enumerate()
            .map(|(idx, text)| {
                let mut vec = vec![0.0f32; self.dimensions];
                for (i, b) in text.bytes().enumerate() {
                    vec[i % self.dimensions] += b as f32 / 255.0;
                }
                // Normalize for cosine similarity
                let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
                if norm > 0.0 {
                    for v in &mut vec {
                        *v /= norm;
                    }
                }
                EmbeddingData {
                    object: "embedding".into(),
                    embedding: vec,
                    index: idx as u32,
                }
            })
            .collect();

        Ok(EmbeddingResponse {
            object: "list".into(),
            data,
            model: request.model,
            usage: EmbeddingUsage {
                prompt_tokens: 0,
                total_tokens: 0,
            },
        })
    }
}

// ---------------------------------------------------------------------------
// Integration demo
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() {
    println!("=== RAG Document Indexing — Integration Demo ===\n");

    // -----------------------------------------------------------------------
    // 1. Wire together real MoFA components
    // -----------------------------------------------------------------------
    let dimensions = 64;
    let provider = Arc::new(MockEmbeddingProvider { dimensions });
    let llm_client = LLMClient::new(provider);
    let config = RagEmbeddingConfig::default().with_dimensions(dimensions);
    let adapter = LlmEmbeddingAdapter::new(llm_client, config);

    let mut store = InMemoryVectorStore::cosine();

    println!("[1] Components wired:");
    println!("    • LLMClient → MockEmbeddingProvider ({dimensions}-dim)");
    println!("    • LlmEmbeddingAdapter (batch_size=256, timeout=30s)");
    println!("    • InMemoryVectorStore (cosine similarity)\n");

    // -----------------------------------------------------------------------
    // 2. Prepare documents (simulating real-world MoFA documentation)
    // -----------------------------------------------------------------------
    let documents = vec![
        IndexDocument::new(
            "arch-001",
            "MoFA uses a microkernel architecture where the core only defines \
             trait interfaces. Concrete implementations live in mofa-foundation, \
             ensuring the kernel stays minimal and stable.",
        )
        .with_metadata("category", "architecture")
        .with_metadata("source", "docs/architecture.md"),
        IndexDocument::new(
            "plugin-001",
            "The dual plugin system supports compile-time Rust/WASM plugins for \
             performance-critical paths and runtime Rhai scripts for dynamic \
             business logic that can be hot-reloaded without restart.",
        )
        .with_metadata("category", "plugins")
        .with_metadata("source", "docs/plugins.md"),
        IndexDocument::new(
            "coord-001",
            "Multi-agent coordination in MoFA supports seven patterns including \
             request-response, publish-subscribe, consensus, debate, parallel, \
             sequential, and custom coordination strategies.",
        )
        .with_metadata("category", "coordination")
        .with_metadata("source", "docs/coordination.md"),
    ];

    println!("[2] Prepared {} documents for indexing", documents.len());

    // -----------------------------------------------------------------------
    // 3. Index using the pipeline (chunk → embed → upsert)
    // -----------------------------------------------------------------------
    let index_config = RagIndexConfig::default()
        .with_chunk_size(200)
        .with_chunk_overlap(30)
        .with_index_mode(IndexMode::Upsert);

    println!("[3] Indexing with config:");
    println!("    • chunk_size=200, overlap=30, mode=upsert");

    let result = index_documents(&mut store, &adapter, &documents, &index_config)
        .await
        .expect("indexing should succeed");

    println!("\n    ✓ Indexed successfully!");
    println!("    • Total chunks produced: {}", result.chunks_total);
    println!("    • Chunks upserted:       {}", result.chunks_upserted);
    println!("    • Documents processed:   {:?}\n", result.document_ids);

    // -----------------------------------------------------------------------
    // 4. Verify integration: search the store directly to prove it works
    // -----------------------------------------------------------------------
    let store_count = store.count().await.expect("count should work");
    println!("[4] Integration verification:");
    println!("    • VectorStore.count() = {store_count}");
    assert_eq!(store_count, result.chunks_total, "store count must match indexed chunks");

    // Search for a query to prove chunks are retrievable
    let query = "How does MoFA handle plugins?";
    let query_embedding = adapter
        .embed_one(query)
        .await
        .expect("query embedding should work");

    let search_results = store
        .search(&query_embedding, 3, None)
        .await
        .expect("search should work");

    println!("    • Query: \"{query}\"");
    println!("    • Top {} results:", search_results.len());
    for (i, r) in search_results.iter().enumerate() {
        let source = r.metadata.get("source").map(String::as_str).unwrap_or("n/a");
        println!(
            "      {}. [score: {:.4}] ({}) \"{}...\"",
            i + 1,
            r.score,
            source,
            &r.text[..r.text.len().min(60)]
        );
    }

    // -----------------------------------------------------------------------
    // 5. Idempotency check: re-index same docs, count stays the same
    // -----------------------------------------------------------------------
    println!("\n[5] Idempotency check (re-indexing same documents):");
    let _result2 = index_documents(&mut store, &adapter, &documents, &index_config)
        .await
        .expect("re-indexing should succeed");

    let count_after = store.count().await.expect("count should work");
    println!("    • Chunks after re-index: {count_after}");
    println!("    • Same as before:        {} ✓", count_after == store_count);
    assert_eq!(
        count_after, store_count,
        "re-indexing identical content must be idempotent"
    );

    println!("\n=== All integration checks passed! ===");
    println!(
        "\nThis example proved that index_documents() orchestrates:\n\
         \n  LLMProvider → LlmEmbeddingAdapter → TextChunker → VectorStore\n\
         \nAll connected via real MoFA kernel traits and foundation implementations."
    );
}
