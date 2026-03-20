//! RAG Agent Tool Demo — Integration Example
//!
//! Demonstrates how the [`RagTool`] integrates with the MoFA agent tool system.
//!
//! This demo shows:
//! 1. Creating a vector store with sample documents
//! 2. Setting up an embedding adapter (using a mock provider)
//! 3. Creating the RagTool with the store and embedder
//! 4. Executing a query through the tool interface
//! 5. Retrieving relevant context for agent queries
//!
//! # Running
//!
//! ```bash
//! cargo run -p rag_agent_demo
//! ```

use mofa_foundation::agent::components::tool::SimpleTool;
use mofa_foundation::agent::tools::rag::RagTool;
use mofa_foundation::llm::client::LLMClient;
use mofa_foundation::llm::provider::LLMProvider;
use mofa_foundation::llm::types::{
    ChatCompletionRequest, ChatCompletionResponse, EmbeddingData, EmbeddingInput,
    EmbeddingRequest, EmbeddingResponse, EmbeddingUsage, LLMError, LLMResult,
};
use mofa_foundation::rag::embedding_adapter::{LlmEmbeddingAdapter, RagEmbeddingConfig};
use mofa_foundation::rag::{ChunkConfig, DocumentChunk, InMemoryVectorStore, TextChunker};
use mofa_kernel::agent::components::tool::ToolInput;
use mofa_kernel::rag::VectorStore;
use std::sync::Arc;
use tokio::sync::RwLock;

// ---------------------------------------------------------------------------
// Mock LLM provider for embeddings
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

        let total: usize = data.iter().map(|d| d.embedding.len()).sum();
        Ok(EmbeddingResponse {
            object: "list".into(),
            data,
            model: request.model.clone(),
            usage: EmbeddingUsage {
                prompt_tokens: 0,
                total_tokens: total as u32,
            },
        })
    }
}

// ---------------------------------------------------------------------------
// Main Demo
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("=== RAG Agent Tool Demo ===\n");

    // Step 1: Set up the embedding provider and adapter
    println!("1. Setting up embedding provider and adapter...");
    let mock_provider = Arc::new(MockEmbeddingProvider { dimensions: 384 });
    let llm_client = LLMClient::new(mock_provider);
    let embedder_config = RagEmbeddingConfig::default().with_dimensions(384);
    let embedder = Arc::new(LlmEmbeddingAdapter::new(llm_client, embedder_config));
    println!("   Embedding adapter ready (dimensions: 384)\n");

    // Step 2: Create sample documents
    println!("2. Creating sample documents...");
    let sample_documents = vec![
        ("doc1", "The capital of France is Paris. It is known for the Eiffel Tower and the Louvre Museum. Paris has a population of about 2.1 million people."),
        ("doc2", "Rust is a systems programming language that focuses on safety and performance. It provides memory safety without using a garbage collector. Rust is used for web servers, embedded systems, and command-line tools."),
        ("doc3", "Machine learning is a subset of artificial intelligence that enables systems to learn and improve from experience. Deep learning uses neural networks with multiple layers. Common applications include image recognition and natural language processing."),
        ("doc4", "The MoFA framework provides a microkernel architecture for building AI agents. It separates concerns across kernel, foundation, runtime, and SDK layers. The foundation layer provides concrete implementations of kernel traits."),
    ];
    println!("   Created {} sample documents\n", sample_documents.len());

    // Step 3: Chunk the documents and index them directly
    println!("3. Chunking and indexing documents...");
    let chunker = TextChunker::new(ChunkConfig::new(100, 20));
    let mut store = InMemoryVectorStore::cosine();

    for (doc_id, text) in &sample_documents {
        let chunks = chunker.chunk_by_chars(text);
        let texts: Vec<String> = chunks.clone();
        
        // Embed all chunks at once
        let embeddings = embedder.embed_batch(&texts).await?;
        
        // Insert into store
        for (idx, (chunk_text, embedding)) in chunks.into_iter().zip(embeddings).enumerate() {
            let chunk_id = format!("{}-{}", doc_id, idx);
            let chunk = DocumentChunk::new(&chunk_id, &chunk_text, embedding);
            store.upsert(chunk).await?;
        }
        println!("   Document '{}' -> {} chunks", doc_id, texts.len());
    }
    println!("   Documents indexed successfully\n");

    // Step 4: Create the RagTool
    println!("4. Creating RagTool...");
    let store = Arc::new(RwLock::new(store));
    let rag_tool = RagTool::new(store, embedder);
    println!("   RagTool created with name: '{}'\n", rag_tool.name());
    println!("   Description: {}\n", rag_tool.description());

    // Step 5: Execute a query through the tool
    println!("5. Executing query through RagTool...");
    let query = "What is the main topic of the document about programming languages?";

    // Create tool input
    let tool_input = ToolInput::from_json(serde_json::json!({
        "query": query,
        "top_k": 2
    }));

    // Execute the tool
    let result = rag_tool.execute(tool_input).await;

    println!("   Query: {}\n", query);

    if result.success {
        println!("=== TOOL EXECUTION SUCCESS ===\n");

        // Parse and display results - ToolResult output is directly a Value
        let output = result.output;
        
        if let Some(results) = output.get("results").and_then(|r| r.as_array()) {
            println!("Retrieved {} chunks:\n", results.len());
            for (i, chunk) in results.iter().enumerate() {
                let content = chunk.get("content").and_then(|c| c.as_str()).unwrap_or("");
                let score = chunk.get("score").and_then(|s| s.as_f64()).unwrap_or(0.0);
                println!("--- Chunk {} (score: {:.4}) ---", i + 1, score);
                println!("{}\n", content);
            }
        }

        // Display combined context
        if let Some(context) = output.get("combined_context").and_then(|c| c.as_str()) {
            println!("=== COMBINED CONTEXT ===");
            println!("{}\n", context);
        }

        println!("=== AGENT CAN NOW USE THIS CONTEXT ===");
        println!("The agent would receive this context to generate a final answer.\n");
    } else {
        println!("=== TOOL EXECUTION FAILED ===");
        println!("Error: {:?}\n", result.error);
    }

    println!("=== Demo Complete ===");

    Ok(())
}
