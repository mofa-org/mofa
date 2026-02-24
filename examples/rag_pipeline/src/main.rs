//! RAG Pipeline Example
//!
//! Demonstrates Retrieval-Augmented Generation using MoFA's vector store
//! abstractions with both in-memory and Qdrant backends.
//!
//! # Running
//!
//! In-memory mode (no external dependencies):
//! ```bash
//! cargo run -p rag_pipeline
//! ```
//!
//! With Qdrant (start Qdrant first):
//! ```bash
//! docker run -p 6333:6333 -p 6334:6334 qdrant/qdrant
//! QDRANT_URL=http://localhost:6334 cargo run -p rag_pipeline -- qdrant
//! ```

use anyhow::Result;
use mofa_foundation::rag::{
    ChunkConfig, DocumentChunk, GeneratorExt, InMemoryVectorStore, PassthroughStreamingGenerator,
    QdrantConfig, QdrantVectorStore, SimilarityMetric, TextChunker, VectorStore,
};
use mofa_kernel::rag::{Generator, GeneratorChunk, PipelineResult};
use async_trait::async_trait;

/// Generate a simple deterministic embedding from text.
///
/// This is a toy embedding function for demonstration purposes only.
/// In production, replace this with a real embedding model such as
/// OpenAI text-embedding-3-small or a local model like all-MiniLM-L6-v2.
fn simple_embedding(text: &str, dimensions: usize) -> Vec<f32> {
    let mut embedding = vec![0.0_f32; dimensions];
    for (i, byte) in text.bytes().enumerate() {
        embedding[i % dimensions] += byte as f32 / 255.0;
    }
    // Normalize to unit vector for cosine similarity
    let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in &mut embedding {
            *x /= norm;
        }
    }
    embedding
}

/// Demonstrates a basic RAG pipeline using the in-memory vector store.
///
/// Steps: chunk documents, embed, store, search, build context for LLM.
async fn basic_rag_pipeline() -> Result<()> {
    println!("--- Basic RAG Pipeline (In-Memory) ---\n");

    let mut store = InMemoryVectorStore::cosine();
    let dimensions = 64;

    // Knowledge base: a few paragraphs about MoFA
    let documents = vec![
        "MoFA is a modular framework for building AI agents in Rust. It uses a microkernel \
         architecture where the core only defines trait interfaces and concrete implementations \
         are provided by the foundation layer.",
        "The dual plugin system in MoFA supports both compile-time Rust/WASM plugins for \
         performance-critical paths and runtime Rhai scripts for hot-reloadable business logic.",
        "MoFA supports seven multi-agent coordination patterns: request-response, \
         publish-subscribe, consensus, debate, parallel, sequential, and custom modes.",
        "The secretary agent pattern in MoFA provides human-in-the-loop workflow management \
         with five phases: receive ideas, clarify requirements, schedule dispatch, monitor \
         feedback, and acceptance report.",
        "MoFA uses UniFFI for cross-language bindings, allowing agents built in Rust to be \
         called from Python, Java, Swift, Kotlin, and Go.",
    ];

    // Chunk and embed each document
    let chunker = TextChunker::new(ChunkConfig {
        chunk_size: 200,
        chunk_overlap: 30,
    });

    let mut all_chunks = Vec::new();
    for (doc_idx, document) in documents.iter().enumerate() {
        let text_chunks = chunker.chunk_by_chars(document);
        for (chunk_idx, text) in text_chunks.iter().enumerate() {
            let id = format!("doc-{doc_idx}-chunk-{chunk_idx}");
            let embedding = simple_embedding(text, dimensions);
            let chunk = DocumentChunk::new(&id, text.as_str(), embedding)
                .with_metadata("source", &format!("document_{doc_idx}"))
                .with_metadata("chunk_index", &chunk_idx.to_string());
            all_chunks.push(chunk);
        }
    }

    println!("Indexed {} chunks from {} documents\n", all_chunks.len(), documents.len());

    store.upsert_batch(all_chunks).await?;

    // Search with a query
    let query = "How does MoFA handle multiple agents working together?";
    let query_embedding = simple_embedding(query, dimensions);

    let results = store.search(&query_embedding, 3, None).await?;

    println!("Query: \"{query}\"\n");
    println!("Top {} results:", results.len());
    for (i, result) in results.iter().enumerate() {
        println!(
            "\n  {}. [score: {:.4}] (source: {})\n     \"{}\"",
            i + 1,
            result.score,
            result.metadata.get("source").unwrap_or(&"unknown".into()),
            truncate_text(&result.text, 120),
        );
    }

    // Build a context string that would be fed to an LLM
    let context: String = results
        .iter()
        .map(|r| r.text.clone())
        .collect::<Vec<_>>()
        .join("\n\n");

    println!("\n--- Context for LLM ---");
    println!("Given the following context:\n{context}");
    println!("\nAnswer the question: {query}");
    println!("\n(In production, this context + question would be sent to an LLM)\n");

    Ok(())
}

/// Demonstrates multi-document ingestion with metadata tracking.
async fn document_ingestion_demo() -> Result<()> {
    println!("--- Document Ingestion Demo (In-Memory) ---\n");

    let mut store = InMemoryVectorStore::cosine();
    let dimensions = 64;

    // Simulate ingesting multiple files
    let files = vec![
        ("architecture.md", "The microkernel pattern keeps the core small and extensible. All concrete implementations live in the foundation layer. The kernel only defines trait interfaces."),
        ("plugins.md", "Compile-time plugins use Rust traits for zero-cost abstractions. Runtime plugins use Rhai scripting with built-in JSON processing. Both layers can interoperate seamlessly."),
        ("deployment.md", "MoFA agents can be deployed as standalone binaries, Docker containers, or as libraries embedded in other applications. The CLI tool provides project scaffolding and management."),
    ];

    let chunker = TextChunker::new(ChunkConfig::default());

    let mut total_chunks = 0;
    for (filename, content) in &files {
        let text_chunks = chunker.chunk_by_sentences(content);
        let mut chunks = Vec::new();
        for (i, text) in text_chunks.iter().enumerate() {
            let id = format!("{filename}-{i}");
            let embedding = simple_embedding(text, dimensions);
            let chunk = DocumentChunk::new(&id, text.as_str(), embedding)
                .with_metadata("filename", *filename)
                .with_metadata("chunk_index", &i.to_string());
            chunks.push(chunk);
        }
        total_chunks += chunks.len();
        store.upsert_batch(chunks).await?;
    }

    println!("Ingested {total_chunks} chunks from {} files", files.len());
    println!("Store contains {} chunks\n", store.count().await?);

    // Search across all documents
    let query = "How are plugins implemented?";
    let query_embedding = simple_embedding(query, dimensions);
    let results = store.search(&query_embedding, 2, None).await?;

    println!("Query: \"{query}\"");
    for (i, result) in results.iter().enumerate() {
        println!(
            "  {}. [score: {:.4}] from {}: \"{}\"",
            i + 1,
            result.score,
            result.metadata.get("filename").unwrap_or(&"unknown".into()),
            truncate_text(&result.text, 100),
        );
    }

    println!();
    Ok(())
}

/// Demonstrates using Qdrant as the vector store backend.
async fn qdrant_rag_pipeline(qdrant_url: &str) -> Result<()> {
    println!("--- Qdrant RAG Pipeline ---\n");

    let dimensions: u64 = 64;
    let collection_name = "mofa_rag_example";

    let config = QdrantConfig {
        url: qdrant_url.into(),
        api_key: std::env::var("QDRANT_API_KEY").ok(),
        collection_name: collection_name.into(),
        vector_dimensions: dimensions,
        metric: SimilarityMetric::Cosine,
        create_collection: true,
    };

    let mut store = QdrantVectorStore::new(config).await?;

    // Clear any previous data
    store.clear().await?;

    let documents = vec![
        ("intro", "MoFA stands for Modular Framework for Agents. It is built in Rust for performance and safety."),
        ("architecture", "MoFA uses a microkernel architecture. The kernel defines traits, the foundation provides implementations."),
        ("agents", "Agents in MoFA can coordinate using patterns like debate, consensus, parallel execution, and sequential pipelines."),
        ("tools", "MoFA agents can use tools defined as Rust traits. Tools handle web search, code execution, file operations, and more."),
    ];

    // Ingest documents
    let mut chunks = Vec::new();
    for (name, text) in &documents {
        let embedding = simple_embedding(text, dimensions as usize);
        let chunk = DocumentChunk::new(*name, *text, embedding)
            .with_metadata("source", *name);
        chunks.push(chunk);
    }

    store.upsert_batch(chunks).await?;
    println!("Stored {} documents in Qdrant collection '{collection_name}'", documents.len());
    println!("Total count: {}\n", store.count().await?);

    // Search
    let query = "What coordination patterns does MoFA support?";
    let query_embedding = simple_embedding(query, dimensions as usize);
    let results = store.search(&query_embedding, 2, None).await?;

    println!("Query: \"{query}\"");
    for (i, result) in results.iter().enumerate() {
        println!(
            "  {}. [score: {:.4}] {}: \"{}\"",
            i + 1,
            result.score,
            result.id,
            truncate_text(&result.text, 100),
        );
    }

    // Demonstrate delete
    store.delete("intro").await?;
    println!("\nDeleted 'intro', count now: {}", store.count().await?);

    // Cleanup
    store.clear().await?;
    println!("Cleared collection, count: {}\n", store.count().await?);

    Ok(())
}

/// Truncate text to a maximum length with ellipsis.
fn truncate_text(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        text.to_string()
    } else {
        format!("{}...", &text[..max_len])
    }
}

/// A mock generator that simulates LLM generation for demonstration.
///
/// In production, this would be replaced with a real LLM provider.
#[derive(Debug, Clone)]
struct MockGenerator {
    /// Simulated response delay in milliseconds
    delay_ms: u64,
}

impl MockGenerator {
    fn new() -> Self {
        Self { delay_ms: 100 }
    }

    /// Simulate token-by-token streaming
    fn simulate_streaming_tokens(&self, response: &str) -> Vec<GeneratorChunk> {
        let words: Vec<&str> = response.split_whitespace().collect();
        let mut chunks = Vec::new();
        
        for (i, word) in words.iter().enumerate() {
            // Add space before word except for first
            if i > 0 {
                chunks.push(GeneratorChunk::text(" "));
            }
            chunks.push(GeneratorChunk::text(*word));
        }
        
        chunks.push(GeneratorChunk::end());
        chunks
    }
}

#[async_trait]
impl Generator for MockGenerator {
    async fn generate(&self, context: &str, query: &str) -> PipelineResult<String> {
        // Simulate some processing time
        tokio::time::sleep(tokio::time::Duration::from_millis(self.delay_ms)).await;
        
        // Build a simulated response
        let response = format!(
            "Based on the context provided, here's what I found about '{}':\n\n{}",
            query,
            context.lines().next().unwrap_or("No relevant information found.")
        );
        
        Ok(response)
    }

    async fn stream(
        &self,
        context: &str,
        query: &str,
    ) -> PipelineResult<mofa_kernel::rag::GeneratorStream> {
        // Get the complete response first
        let response = self.generate(context, query).await?;
        
        // Create streaming chunks
        let chunks = self.simulate_streaming_tokens(&response);
        
        let stream = futures::stream::iter(
            chunks.into_iter().map(Ok::<_, mofa_kernel::rag::PipelineError>)
        );
        
        Ok(Box::pin(stream))
    }

    fn supports_streaming(&self) -> bool {
        true
    }
}

/// Demonstrates streaming generation with the RAG pipeline.
async fn streaming_generation_demo() -> Result<()> {
    println!("--- Streaming Generation Demo ---\n");
    
    // Create a mock generator (in production, use a real LLM provider)
    let generator = MockGenerator::new();
    
    // Wrap it with streaming support using the adapter
    let streaming_generator = generator.clone().with_streaming();
    
    // Sample context from retrieval
    let context = "MoFA is a modular framework for building AI agents in Rust. It uses a microkernel architecture.";
    let query = "What is MoFA?";
    
    // Demonstrate blocking generation
    println!("1. Blocking generation:");
    let blocking_result = generator.generate(context, query).await?;
    println!("   Result: {}\n", truncate_text(&blocking_result, 80));
    
    // Demonstrate streaming generation
    println!("2. Streaming generation:");
    print!("   Streaming output: \"");
    
    let stream = generator.stream(context, query).await?;
    use futures::stream::StreamExt;
    let mut stream = stream;
    
    while let Some(chunk_result) = stream.next().await {
        match chunk_result? {
            GeneratorChunk::Text(text) => {
                print!("{}", text);
            }
            GeneratorChunk::End => {
                println!("\" (end)");
            }
        }
    }
    
    // Demonstrate the passthrough adapter
    println!("\n3. Using PassthroughStreamingGenerator adapter:");
    print!("   Adapter output: \"");
    
    let stream = streaming_generator.stream(context, query).await?;
    let mut stream = stream;
    
    while let Some(chunk_result) = stream.next().await {
        match chunk_result? {
            GeneratorChunk::Text(text) => {
                print!("{}", text);
            }
            GeneratorChunk::End => {
                println!("\" (end)");
            }
        }
    }
    
    println!("\n4. Generator supports streaming: {}", generator.supports_streaming());
    println!();
    
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    let args: Vec<String> = std::env::args().collect();
    let mode = args.get(1).map(|s| s.as_str()).unwrap_or("memory");

    println!("=== MoFA RAG Pipeline Example ===\n");

    // Always run in-memory demos
    basic_rag_pipeline().await?;
    document_ingestion_demo().await?;
    
    // Run streaming generation demo
    streaming_generation_demo().await?;

    // Run Qdrant demo if requested
    if mode == "qdrant" {
        let url = std::env::var("QDRANT_URL").unwrap_or_else(|_| "http://localhost:6334".into());
        qdrant_rag_pipeline(&url).await?;
    } else {
        println!("--- Qdrant Demo Skipped ---");
        println!("To run with Qdrant, start a Qdrant instance and run:");
        println!("  QDRANT_URL=http://localhost:6334 cargo run -p rag_pipeline -- qdrant\n");
    }

    println!("=== Done ===");
    Ok(())
}
