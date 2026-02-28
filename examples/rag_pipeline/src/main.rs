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

use futures::StreamExt;
use mofa_foundation::rag::{
    ChunkConfig, Document, DocumentChunk, IdentityReranker, InMemoryVectorStore, PassthroughStreamingGenerator,
    QdrantConfig, QdrantVectorStore, RagPipeline, ScoredDocument, SimilarityMetric,
    TextChunker, VectorStore,
};
use mofa_kernel::rag::pipeline::{Generator, Retriever};
use mofa_kernel::rag::types::GenerateInput;
use std::sync::Arc;

/// Generate a simple topic label for a document
fn get_document_topic(doc_idx: usize) -> String {
    match doc_idx {
        0 => "architecture".to_string(),
        1 => "performance".to_string(),
        2 => "extensibility".to_string(),
        3 => "deployment".to_string(),
        4 => "examples".to_string(),
        _ => "general".to_string(),
    }
}

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

/// Simple retriever that uses vector similarity search
struct SimpleRetriever {
    store: InMemoryVectorStore,
    dimensions: usize,
}

impl SimpleRetriever {
    fn new(store: InMemoryVectorStore, dimensions: usize) -> Self {
        Self { store, dimensions }
    }
}

#[async_trait::async_trait]
impl Retriever for SimpleRetriever {
    async fn retrieve(&self, query: &str, top_k: usize) -> mofa_kernel::agent::error::AgentResult<Vec<ScoredDocument>> {
        let query_embedding = simple_embedding(query, self.dimensions);
        let results = self.store.search(&query_embedding, top_k, None).await?;
        Ok(results
            .into_iter()
            .map(|r| ScoredDocument {
                document: Document::new(r.id, r.text),
                score: r.score,
                source: Some("vector_search".to_string()),
            })
            .collect())
    }
}

/// Simple generator that creates a response from documents
struct SimpleGenerator;

#[async_trait::async_trait]
impl Generator for SimpleGenerator {
    async fn generate(&self, input: &GenerateInput) -> mofa_kernel::agent::error::AgentResult<String> {
        let context = input
            .context
            .iter()
            .map(|d| d.text.clone())
            .collect::<Vec<_>>()
            .join("\n\n");

        Ok(format!(
            "Based on the following context:\n\n{}\n\nAnswer to '{}': This is a generated response.",
            context, input.query
        ))
    }
}

/// Demonstrates a basic RAG pipeline using the in-memory vector store.
///
/// Steps: chunk documents, embed, store, search, build context for LLM.
async fn basic_rag_pipeline() -> Result<(), Box<dyn std::error::Error>> {
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

/// Demonstrates the new RAG pipeline with streaming generation and real-world testing.
async fn streaming_rag_pipeline() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- Streaming RAG Pipeline with Real-World Testing ---\n");

    let mut store = InMemoryVectorStore::cosine();
    let dimensions = 64;

    // More comprehensive and realistic knowledge base
    let documents = vec![
        "MoFA (Modular Framework for Agents) is a production-grade AI agent framework built in Rust, \
         designed for extreme performance, unlimited extensibility, and runtime programmability. \
         It implements a microkernel + dual-layer plugin system architecture that allows developers \
         to build sophisticated multi-agent systems with minimal overhead.",

        "The MoFA microkernel provides core abstractions for agent lifecycle management, message passing, \
         and plugin coordination. It defines trait interfaces for tools, memory systems, reasoning engines, \
         and communication protocols, ensuring consistent behavior across different implementations.",

        "MoFA's dual plugin system consists of compile-time plugins (Rust/WASM) for performance-critical \
         operations like LLM inference and data processing, and runtime plugins (Rhai scripts) for \
         dynamic business logic, workflow orchestration, and hot-reloadable rules.",

        "Multi-agent coordination in MoFA supports seven distinct patterns: Request-Response for \
         deterministic one-to-one tasks, Publish-Subscribe for one-to-many broadcast scenarios, \
         Consensus for multi-round negotiation, Debate for quality improvement through discussion, \
         Parallel for simultaneous execution, Sequential for pipeline workflows, and Custom modes \
         for domain-specific coordination strategies.",

        "The Secretary Agent pattern implements human-in-the-loop workflows with five phases: \
         receiving ideas and recording todos, clarifying requirements through interactive sessions, \
         scheduling and dispatching execution agents, monitoring progress and collecting feedback, \
         and generating acceptance reports for human review and approval.",

        "MoFA provides cross-language bindings through UniFFI, enabling Rust agents to be called \
         from Python, Java, Swift, Kotlin, and Go applications. This allows teams to leverage \
         MoFA's performance benefits while integrating with existing codebases in other languages.",

        "Distributed dataflow support in MoFA is provided through optional Dora-rs integration, \
         enabling agents to participate in distributed processing pipelines with automatic \
         serialization, routing, and fault tolerance across multiple nodes.",

        "MoFA's persistence layer supports PostgreSQL, MySQL, and SQLite backends with async \
         database drivers, providing ACID transactions, connection pooling, and migration support \
         for storing agent state, conversation history, and application data.",

        "The actor-based concurrency model in MoFA uses Ractor for lightweight, isolated execution \
         contexts that prevent data races and provide natural fault isolation between agents. \
         Each agent runs in its own actor with dedicated message queues and state management.",

        "Plugin development in MoFA follows strict architectural boundaries: the kernel layer defines \
         all trait interfaces, the foundation layer provides concrete implementations, and plugins \
         can extend functionality at both compile-time and runtime without modifying core code.",
    ];

    // Chunk and embed documents with more realistic chunking
    let chunker = TextChunker::new(ChunkConfig {
        chunk_size: 300,  // Larger chunks for more context
        chunk_overlap: 50, // More overlap for better continuity
    });

    let mut all_chunks = Vec::new();
    for (doc_idx, document) in documents.iter().enumerate() {
        let text_chunks = chunker.chunk_by_chars(document);
        for (chunk_idx, text) in text_chunks.iter().enumerate() {
            let id = format!("doc-{doc_idx}-chunk-{chunk_idx}");
            let embedding = simple_embedding(text, dimensions);
            let chunk = DocumentChunk::new(&id, text.as_str(), embedding)
                .with_metadata("source", &format!("document_{doc_idx}"))
                .with_metadata("chunk_index", &chunk_idx.to_string())
                .with_metadata("word_count", &text.split_whitespace().count().to_string())
                .with_metadata("topic", get_document_topic(doc_idx));
            all_chunks.push(chunk);
        }
    }

    println!("Indexed {} chunks from {} comprehensive documents\n", all_chunks.len(), documents.len());

    store.upsert_batch(all_chunks).await?;

    // Create pipeline components
    let retriever = Arc::new(SimpleRetriever::new(store, dimensions));
    let reranker = Arc::new(IdentityReranker);
    let generator = Arc::new(PassthroughStreamingGenerator::new(SimpleGenerator));
    let pipeline = RagPipeline::new(retriever, reranker, generator);

    // Test multiple realistic queries
    let test_queries = vec![
        "How does MoFA achieve both performance and extensibility?",
        "What are the different ways agents can coordinate in MoFA?",
        "How does the Secretary Agent pattern work?",
        "What database backends does MoFA support?",
        "How does MoFA handle cross-language integration?",
    ];

    for (query_idx, query) in test_queries.iter().enumerate() {
        println!("\n--- Test Query {}: \"{}\" ---", query_idx + 1, query);

        let start_time = std::time::Instant::now();

        let (documents, mut stream) = pipeline.run_streaming(query, 3).await?;

        let retrieval_time = start_time.elapsed();
        println!("Retrieval completed in {:.2}ms, found {} documents",
                retrieval_time.as_millis(), documents.len());

        for (i, doc) in documents.iter().enumerate() {
            println!("  {}. [score: {:.4}] {} ({} words)",
                    i + 1, doc.score,
                    truncate_text(&doc.document.text, 60),
                    doc.document.metadata.get("word_count").unwrap_or(&"0".to_string()));
        }

        println!("\n--- Streaming Generation ---");
        print!("Response: ");

        let mut full_response = String::new();
        let mut chunk_count = 0;
        let mut total_chars = 0;

        while let Some(chunk_result) = stream.next().await {
            match chunk_result? {
                mofa_kernel::rag::pipeline::GeneratorChunk::Text(text) => {
                    print!("{}", text);
                    full_response.push_str(&text);
                    chunk_count += 1;
                    total_chars += text.len();

                    // Simulate realistic streaming delays for testing
                    if chunk_count % 3 == 0 {
                        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                    }
                }
                mofa_kernel::rag::pipeline::GeneratorChunk::Done => break,
            }
        }

        let total_time = start_time.elapsed();
        println!("\n\nStreaming stats: {} chunks, {} chars, {:.2}ms total",
                chunk_count, total_chars, total_time.as_millis());
        println!("Generation throughput: {:.1} chars/sec",
                total_chars as f64 / total_time.as_secs_f64());
    }

    // Test error handling scenarios
    println!("\n--- Error Handling Tests ---");

    // Test with empty query
    match pipeline.run_streaming("", 3).await {
        Ok(_) => println!("✓ Empty query handled gracefully"),
        Err(e) => println!("✗ Empty query failed: {}", e),
    }

    // Test with very long query
    let long_query = "What is the relationship between ".repeat(50) + "?";
    match pipeline.run_streaming(&long_query, 3).await {
        Ok((docs, _)) => println!("✓ Long query handled ({} docs retrieved)", docs.len()),
        Err(e) => println!("✗ Long query failed: {}", e),
    }

    Ok(())
}

/// Test edge cases and error conditions in streaming
async fn streaming_edge_cases_test() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- Streaming Edge Cases Test ---\n");

    let mut store = InMemoryVectorStore::cosine();
    let dimensions = 64;

    // Minimal knowledge base for edge case testing
    let documents = vec![
        "MoFA is a framework for AI agents.",
        "It supports streaming responses.",
        "Error handling is important.",
    ];

    let chunker = TextChunker::new(ChunkConfig {
        chunk_size: 100,
        chunk_overlap: 20,
    });

    let mut all_chunks = Vec::new();
    for (doc_idx, document) in documents.iter().enumerate() {
        let text_chunks = chunker.chunk_by_chars(document);
        for (chunk_idx, text) in text_chunks.iter().enumerate() {
            let id = format!("edge-doc-{doc_idx}-chunk-{chunk_idx}");
            let embedding = simple_embedding(text, dimensions);
            let chunk = DocumentChunk::new(&id, text.as_str(), embedding);
            all_chunks.push(chunk);
        }
    }

    store.upsert_batch(all_chunks).await?;

    let retriever = Arc::new(SimpleRetriever::new(store, dimensions));
    let reranker = Arc::new(IdentityReranker);
    let generator = Arc::new(PassthroughStreamingGenerator::new(SimpleGenerator));
    let pipeline = RagPipeline::new(retriever, reranker.clone(), generator.clone());

    // Test cases
    let long_query = "What is the meaning of life? ".repeat(100);
    let test_cases = vec![
        ("Empty query", "", 3, "Should handle empty queries gracefully"),
        ("Very long query", &long_query, 3, "Should handle very long queries"),
        ("Special characters", "Query with @#$%^&*() symbols!", 3, "Should handle special characters"),
        ("Unicode content", "Query with émojis 🚀 and 中文 characters", 3, "Should handle Unicode content"),
        ("Single character", "A", 1, "Should work with minimal input"),
        ("Maximum results", "framework agents", 10, "Should handle requesting more results than available"),
    ];

    for (test_name, query, top_k, description) in test_cases {
        println!("Testing: {} - {}", test_name, description);

        match pipeline.run_streaming(query, top_k).await {
            Ok((documents, mut stream)) => {
                println!("  ✓ Query succeeded, retrieved {} documents", documents.len());

                let mut chunk_count = 0;
                let mut total_chars = 0;
                let mut stream_success = true;

                while let Some(chunk_result) = stream.next().await {
                    match chunk_result {
                        Ok(chunk) => match chunk {
                            mofa_kernel::rag::pipeline::GeneratorChunk::Text(text) => {
                                chunk_count += 1;
                                total_chars += text.len();
                            }
                            mofa_kernel::rag::pipeline::GeneratorChunk::Done => break,
                        },
                        Err(e) => {
                            println!("  ✗ Stream error: {}", e);
                            stream_success = false;
                            break;
                        }
                    }
                }

                if stream_success {
                    println!("  ✓ Streaming succeeded: {} chunks, {} characters", chunk_count, total_chars);
                }
            }
            Err(e) => {
                println!("  ✗ Query failed: {}", e);
            }
        }
        println!();
    }

    // Test pipeline with no retriever (should fail gracefully)
    println!("Testing pipeline with missing retriever...");
    let broken_retriever = Arc::new(SimpleRetriever::new(InMemoryVectorStore::cosine(), dimensions));
    let broken_pipeline = RagPipeline::new(broken_retriever, reranker.clone(), generator.clone());

    match broken_pipeline.run_streaming("test query", 3).await {
        Ok(_) => println!("  ✗ Expected error but succeeded"),
        Err(e) => println!("  ✓ Correctly failed with error: {}", e),
    }

    println!("\n✓ Edge cases test completed");

    Ok(())
}

async fn streaming_memory_test() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- Streaming Memory Usage Test ---\n");

    // Note: This is a basic memory test. In production, you'd use proper memory profiling tools.
    println!("Note: For accurate memory profiling, use tools like heaptrack or valgrind.\n");

    let mut store = InMemoryVectorStore::cosine();
    let dimensions = 64;

    // Create a substantial knowledge base
    let mut documents = Vec::new();
    for i in 0..50 {
        documents.push(format!(
            "Document {}: MoFA is a comprehensive framework for AI agent development. \
             It provides extensive capabilities for building distributed systems, \
             including advanced coordination patterns, persistence layers, and cross-language \
             interoperability. The framework supports various deployment scenarios from \
             embedded systems to cloud-scale applications. Key features include the \
             microkernel architecture, dual plugin system, and actor-based concurrency model. \
             Performance optimizations include zero-copy message passing, async I/O operations, \
             and efficient memory management techniques. The framework has been designed \
             with scalability, reliability, and maintainability as core principles.",
            i
        ));
    }

    // Chunk documents
    let chunker = TextChunker::new(ChunkConfig {
        chunk_size: 400,
        chunk_overlap: 100,
    });

    let mut all_chunks = Vec::new();
    for (doc_idx, document) in documents.iter().enumerate() {
        let text_chunks = chunker.chunk_by_chars(document);
        for (chunk_idx, text) in text_chunks.iter().enumerate() {
            let id = format!("mem-doc-{doc_idx}-chunk-{chunk_idx}");
            let embedding = simple_embedding(text, dimensions);
            let chunk = DocumentChunk::new(&id, text.as_str(), embedding);
            all_chunks.push(chunk);
        }
    }

    println!("Memory test setup: {} chunks from {} documents", all_chunks.len(), documents.len());

    // Test memory usage during indexing
    let index_start = std::time::Instant::now();
    store.upsert_batch(all_chunks).await?;
    let index_time = index_start.elapsed();

    println!("Indexing completed in {:.2}ms", index_time.as_millis());

    let retriever = Arc::new(SimpleRetriever::new(store, dimensions));
    let reranker = Arc::new(IdentityReranker);
    let generator = Arc::new(PassthroughStreamingGenerator::new(SimpleGenerator));
    let pipeline = RagPipeline::new(retriever, reranker, generator);

    // Test concurrent streaming operations
    println!("\nTesting concurrent streaming operations...");

    let mut handles = Vec::new();
    for i in 0..5 {
        let pipeline_clone = pipeline.clone();
        let handle = tokio::spawn(async move {
            let query = format!("What are the key features of document {}?", i);
            let start = std::time::Instant::now();

            let (docs, mut stream) = pipeline_clone.run_streaming(&query, 3).await.map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { format!("{}", e).into() })?;
            let mut char_count = 0;

            while let Some(chunk_result) = stream.next().await {
                match chunk_result.map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { format!("{}", e).into() })? {
                    mofa_kernel::rag::pipeline::GeneratorChunk::Text(text) => {
                        char_count += text.len();
                        // Simulate processing time
                        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
                    }
                    mofa_kernel::rag::pipeline::GeneratorChunk::Done => break,
                }
            }

            let duration = start.elapsed();
            Ok::<_, Box<dyn std::error::Error + Send + Sync>>((docs.len(), char_count, duration))
        });
        handles.push(handle);
    }

    // Wait for all concurrent operations
    let mut total_docs = 0;
    let mut total_chars = 0;
    let mut max_duration = std::time::Duration::new(0, 0);

    for handle in handles {
        let (docs, chars, duration) = match handle.await {
            Ok(Ok(val)) => val,
            Ok(Err(e)) => return Err(format!("Task error: {}", e).into()),
            Err(e) => return Err(format!("Join error: {}", e).into()),
        };
        total_docs += docs;
        total_chars += chars;
        if duration > max_duration {
            max_duration = duration;
        }
        println!("  Concurrent operation: {} docs, {} chars, {:.2}ms",
                docs, chars, duration.as_millis());
    }

    println!("\nConcurrent test results:");
    println!("  Total documents retrieved: {}", total_docs);
    println!("  Total characters streamed: {}", total_chars);
    println!("  Max operation time: {:.2}ms", max_duration.as_millis());
    println!("  Average throughput: {:.1} chars/sec",
            total_chars as f64 / max_duration.as_secs_f64());

    // Test cleanup and memory release
    println!("\nTesting memory cleanup...");
    drop(pipeline); // Explicit cleanup
    tokio::time::sleep(std::time::Duration::from_millis(100)).await; // Allow cleanup

    println!("✓ Memory test completed successfully");

    Ok(())
}

async fn streaming_performance_benchmark() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- Streaming RAG Performance Benchmark ---\n");

    let mut store = InMemoryVectorStore::cosine();
    let dimensions = 64;

    // Create a larger knowledge base for benchmarking
    let base_docs = vec![
        "MoFA is a modular framework for building AI agents in Rust with microkernel architecture.",
        "The dual plugin system supports both compile-time Rust/WASM and runtime Rhai scripts.",
        "Multi-agent coordination patterns include request-response, publish-subscribe, and consensus.",
        "The Secretary Agent pattern provides human-in-the-loop workflow management.",
        "UniFFI enables cross-language bindings for Python, Java, Swift, Kotlin, and Go.",
    ];

    // Generate multiple variations to create a larger corpus
    let mut documents = Vec::new();
    for i in 0..20 {
        for base_doc in &base_docs {
            documents.push(format!("{} (variation {})", base_doc, i));
        }
    }

    // Chunk documents
    let chunker = TextChunker::new(ChunkConfig {
        chunk_size: 200,
        chunk_overlap: 30,
    });

    let mut all_chunks = Vec::new();
    for (doc_idx, document) in documents.iter().enumerate() {
        let text_chunks = chunker.chunk_by_chars(document);
        for (chunk_idx, text) in text_chunks.iter().enumerate() {
            let id = format!("bench-doc-{doc_idx}-chunk-{chunk_idx}");
            let embedding = simple_embedding(text, dimensions);
            let chunk = DocumentChunk::new(&id, text.as_str(), embedding);
            all_chunks.push(chunk);
        }
    }

    println!("Benchmark setup: {} chunks from {} documents", all_chunks.len(), documents.len());
    store.upsert_batch(all_chunks).await?;

    let retriever = Arc::new(SimpleRetriever::new(store, dimensions));
    let reranker = Arc::new(IdentityReranker);
    let generator = Arc::new(PassthroughStreamingGenerator::new(SimpleGenerator));
    let pipeline = RagPipeline::new(retriever, reranker, generator);

    // Benchmark different scenarios
    let scenarios = vec![
        ("Short query", "What is MoFA?", 3),
        ("Medium query", "How does MoFA handle multiple agents coordination?", 5),
        ("Long query", "Can you explain the relationship between MoFA's microkernel architecture and its dual plugin system?", 5),
    ];

    println!("\nRunning benchmarks...\n");

    for (scenario_name, query, top_k) in scenarios {
        println!("Scenario: {}", scenario_name);

        let mut total_retrieval_time = std::time::Duration::new(0, 0);
        let mut total_streaming_time = std::time::Duration::new(0, 0);
        let mut total_chars = 0;
        let runs = 5; // Run each scenario multiple times

        for run in 0..runs {
            let start_time = std::time::Instant::now();

            let (documents, mut stream) = pipeline.run_streaming(query, top_k).await?;
            let retrieval_time = start_time.elapsed();
            total_retrieval_time += retrieval_time;

            let mut run_chars = 0;
            let streaming_start = std::time::Instant::now();

            while let Some(chunk_result) = stream.next().await {
                match chunk_result? {
                    mofa_kernel::rag::pipeline::GeneratorChunk::Text(text) => {
                        run_chars += text.len();
                        // Small delay to simulate realistic streaming
                        tokio::time::sleep(std::time::Duration::from_millis(1)).await;
                    }
                    mofa_kernel::rag::pipeline::GeneratorChunk::Done => break,
                }
            }

            let streaming_time = streaming_start.elapsed();
            total_streaming_time += streaming_time;
            total_chars += run_chars;

            println!("  Run {}: {} docs, {} chars, retrieval: {:.2}ms, streaming: {:.2}ms",
                    run + 1, documents.len(), run_chars,
                    retrieval_time.as_millis(), streaming_time.as_millis());
        }

        let avg_retrieval = total_retrieval_time / runs as u32;
        let avg_streaming = total_streaming_time / runs as u32;
        let avg_chars = total_chars / runs;

        println!("  Average: retrieval {:.2}ms, streaming {:.2}ms, {} chars",
                avg_retrieval.as_millis(), avg_streaming.as_millis(), avg_chars);
        println!("  Throughput: {:.1} chars/sec\n",
                avg_chars as f64 / avg_streaming.as_secs_f64());
    }

    Ok(())
}

/// Demonstrates multi-document ingestion with metadata tracking.
async fn document_ingestion_demo() -> Result<(), Box<dyn std::error::Error>> {
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
async fn qdrant_rag_pipeline(qdrant_url: &str) -> Result<(), Box<dyn std::error::Error>> {
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    let args: Vec<String> = std::env::args().collect();
    let mode = args.get(1).map(|s| s.as_str()).unwrap_or("memory");

    println!("=== MoFA RAG Pipeline Example ===\n");

    // Always run in-memory demos
    basic_rag_pipeline().await?;
    document_ingestion_demo().await?;
    streaming_rag_pipeline().await?;
    streaming_performance_benchmark().await?;
    streaming_memory_test().await?;
    streaming_edge_cases_test().await?;

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
