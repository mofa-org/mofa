//! RAG Retrieval Pipeline — Integration Example
//!
//! Demonstrates how the **retrieval pipeline** (`query_documents`) integrates
//! with other MoFA components end-to-end:
//!
//! 1. A mock `LLMProvider` (stands in for OpenAI / Ollama)
//! 2. `LlmEmbeddingAdapter` — the foundation adapter from `embedding_adapter`
//! 3. `InMemoryVectorStore` — kernel `VectorStore` implementation
//! 4. `query_documents` — the high-level retrieval function with:
//!    - Metadata-based post-filtering
//!    - Score-based reranking
//!    - Context packing within a byte budget
//!
//! The example first populates a vector store with chunked documents,
//! then runs several retrieval scenarios to show how the pipeline connects
//! all MoFA layers (kernel traits → foundation implementations → user code).
//!
//! # Running
//!
//! ```bash
//! cargo run -p rag_retrieval_demo
//! ```

use mofa_foundation::llm::client::LLMClient;
use mofa_foundation::llm::provider::LLMProvider;
use mofa_foundation::llm::types::{
    ChatCompletionRequest, ChatCompletionResponse, EmbeddingData, EmbeddingInput,
    EmbeddingRequest, EmbeddingResponse, EmbeddingUsage, LLMError, LLMResult,
};
use mofa_foundation::rag::embedding_adapter::{
    LlmEmbeddingAdapter, RagEmbeddingConfig, deterministic_chunk_id,
};
use mofa_foundation::rag::retrieval::{RagQueryConfig, query_documents};
use mofa_foundation::rag::{ChunkConfig, InMemoryVectorStore, TextChunker};
use mofa_kernel::rag::{DocumentChunk, VectorStore};
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Mock LLM provider (same pattern used in the existing rag_pipeline example)
// ---------------------------------------------------------------------------

/// A lightweight mock provider producing deterministic embeddings.
/// In production, swap for `OpenAIProvider` or `OllamaProvider`.
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
        Err(LLMError::Other("chat not supported".into()))
    }
    async fn chat_stream(
        &self,
        _req: ChatCompletionRequest,
    ) -> LLMResult<mofa_foundation::llm::provider::ChatStream> {
        Err(LLMError::Other("streaming not supported".into()))
    }

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
// Helper: pre-populate the vector store with chunked + embedded documents
// ---------------------------------------------------------------------------

struct KnowledgeDoc {
    id: &'static str,
    text: &'static str,
    category: &'static str,
}

async fn populate_store(
    store: &mut InMemoryVectorStore,
    adapter: &LlmEmbeddingAdapter,
) -> usize {
    let docs = vec![
        KnowledgeDoc {
            id: "arch-001",
            text: "MoFA uses a microkernel architecture where the core only defines \
                   trait interfaces. Concrete implementations live in mofa-foundation, \
                   ensuring the kernel stays minimal and stable.",
            category: "architecture",
        },
        KnowledgeDoc {
            id: "plugin-001",
            text: "The dual plugin system supports compile-time Rust/WASM plugins for \
                   performance-critical paths and runtime Rhai scripts for dynamic \
                   business logic that can be hot-reloaded without restart.",
            category: "plugins",
        },
        KnowledgeDoc {
            id: "coord-001",
            text: "Multi-agent coordination in MoFA supports seven patterns including \
                   request-response, publish-subscribe, consensus, debate, parallel, \
                   sequential, and custom coordination strategies.",
            category: "coordination",
        },
        KnowledgeDoc {
            id: "rag-001",
            text: "The RAG pipeline provides document chunking, embedding via pluggable \
                   LLM providers, ANN search through the VectorStore trait, metadata \
                   filtering, reranking, and context packing with byte budgets.",
            category: "rag",
        },
        KnowledgeDoc {
            id: "deploy-001",
            text: "MoFA agents can be deployed as standalone binaries, Docker containers, \
                   or as libraries embedded in other applications via UniFFI bindings \
                   for Python, Java, Swift, Kotlin, and Go.",
            category: "deployment",
        },
    ];

    let chunker = TextChunker::new(ChunkConfig::new(200, 30));
    let mut total_chunks = 0;

    for doc in &docs {
        let chunks = chunker.chunk_by_chars(doc.text);
        let texts: Vec<String> = chunks.clone();
        let embeddings = adapter.embed_batch(&texts).await.expect("embedding works");

        for (idx, (text, emb)) in chunks.into_iter().zip(embeddings).enumerate() {
            let chunk_id = deterministic_chunk_id(doc.id, idx, &text);
            let dc = DocumentChunk::new(chunk_id, &text, emb)
                .with_metadata("category", doc.category)
                .with_metadata("source_doc_id", doc.id)
                .with_metadata("chunk_index", &idx.to_string());
            store.upsert(dc).await.expect("upsert works");
            total_chunks += 1;
        }
    }

    total_chunks
}

// ---------------------------------------------------------------------------
// Integration demo
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() {
    println!("=== RAG Retrieval Pipeline — Integration Demo ===\n");

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
    // 2. Populate the store with chunked, embedded documents
    // -----------------------------------------------------------------------
    let chunk_count = populate_store(&mut store, &adapter).await;
    println!("[2] Populated store with {chunk_count} chunks from 5 documents\n");

    // -----------------------------------------------------------------------
    // 3. Basic retrieval: top-3 results, no filters
    // -----------------------------------------------------------------------
    println!("─── Scenario 1: Basic retrieval (top 3) ───\n");

    let query = "How does MoFA handle plugins and extensibility?";
    let basic_config = RagQueryConfig::default().with_top_k(3);

    let result = query_documents(&store, &adapter, query, &basic_config)
        .await
        .expect("retrieval should succeed");

    println!("    Query:   \"{query}\"");
    println!("    Results: {} chunks", result.chunks.len());
    for (i, chunk) in result.chunks.iter().enumerate() {
        let cat = chunk.metadata.get("category").map(String::as_str).unwrap_or("n/a");
        println!(
            "      {}. [score: {:.4}] [{}] \"{}...\"",
            i + 1,
            chunk.score,
            cat,
            &chunk.text[..chunk.text.len().min(60)]
        );
    }
    println!("\n    Context ({} bytes):", result.context_bytes);
    println!("    \"{}...\"", &result.context[..result.context.len().min(120)]);

    // -----------------------------------------------------------------------
    // 4. Filtered retrieval: only "coordination" docs
    // -----------------------------------------------------------------------
    println!("\n─── Scenario 2: Metadata filter (category=coordination) ───\n");

    let filtered_config = RagQueryConfig::default()
        .with_top_k(5)
        .with_filter("category", "coordination");

    let result = query_documents(&store, &adapter, "agent patterns", &filtered_config)
        .await
        .expect("filtered retrieval should succeed");

    println!("    Query:   \"agent patterns\" (filter: category=coordination)");
    println!("    Results: {} chunks", result.chunks.len());
    for (i, chunk) in result.chunks.iter().enumerate() {
        let cat = chunk.metadata.get("category").map(String::as_str).unwrap_or("n/a");
        println!("      {}. [score: {:.4}] [{}]", i + 1, chunk.score, cat);
        assert_eq!(cat, "coordination", "filter must only return 'coordination' chunks");
    }
    println!("    ✓ All results correctly filtered to 'coordination' category");

    // -----------------------------------------------------------------------
    // 5. Budget-limited retrieval: max 200 bytes of context
    // -----------------------------------------------------------------------
    println!("\n─── Scenario 3: Context budget (max 200 bytes) ───\n");

    let budget_config = RagQueryConfig::default()
        .with_top_k(5)
        .with_max_context_chars(200);

    let result = query_documents(&store, &adapter, "deployment options", &budget_config)
        .await
        .expect("budget retrieval should succeed");

    println!("    Query:   \"deployment options\" (budget: 200 bytes)");
    println!("    Results: {} chunks (trimmed to budget)", result.chunks.len());
    println!("    Context: {} bytes", result.context_bytes);
    assert!(
        result.context_bytes <= 200,
        "context must fit within 200-byte budget, got {}",
        result.context_bytes
    );
    println!("    ✓ Context fits within budget");

    // -----------------------------------------------------------------------
    // 6. Reranked retrieval: keep only top-2 after reranking
    // -----------------------------------------------------------------------
    println!("\n─── Scenario 4: Rerank top-2 ───\n");

    let rerank_config = RagQueryConfig::default()
        .with_top_k(5)
        .with_rerank_top_k(2);

    let result = query_documents(&store, &adapter, "microkernel architecture", &rerank_config)
        .await
        .expect("reranked retrieval should succeed");

    println!("    Query:   \"microkernel architecture\" (rerank_top_k=2)");
    println!("    Results: {} chunks", result.chunks.len());
    assert!(
        result.chunks.len() <= 2,
        "rerank_top_k=2 must limit results to at most 2, got {}",
        result.chunks.len()
    );
    for (i, chunk) in result.chunks.iter().enumerate() {
        println!(
            "      {}. [score: {:.4}] \"{}...\"",
            i + 1,
            chunk.score,
            &chunk.text[..chunk.text.len().min(60)]
        );
    }
    println!("    ✓ Results correctly limited by rerank_top_k");

    // -----------------------------------------------------------------------
    // 7. Edge case: empty query
    // -----------------------------------------------------------------------
    println!("\n─── Scenario 5: Error handling (empty query) ───\n");

    let err = query_documents(&store, &adapter, "   ", &basic_config)
        .await
        .expect_err("empty query should fail");

    println!("    Empty query → Error: {err}");
    println!("    ✓ Empty queries rejected gracefully");

    // -----------------------------------------------------------------------
    // Summary
    // -----------------------------------------------------------------------
    println!("\n=== All integration checks passed! ===");
    println!(
        "\nThis example proved that query_documents() orchestrates:\n\
         \n  Query → LlmEmbeddingAdapter → VectorStore.search()\n\
         \n       → Metadata Filter → Rerank → Context Pack\n\
         \nAll connected via real MoFA kernel traits and foundation implementations.\n\
         \nIn production, the packed context feeds directly into an LLM prompt\n\
         via LLMClient.chat() for the final Retrieval-Augmented Generation step."
    );
}
