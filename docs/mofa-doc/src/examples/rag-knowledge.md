# RAG & Knowledge

Examples demonstrating Retrieval-Augmented Generation (RAG) with vector stores.

## Basic RAG Pipeline

In-memory RAG with document chunking and semantic search.

**Location:** `examples/rag_pipeline/`

```rust
use mofa_foundation::rag::{
    ChunkConfig, DocumentChunk, InMemoryVectorStore,
    TextChunker, VectorStore,
};

async fn basic_rag_pipeline() -> Result<()> {
    // Create vector store with cosine similarity
    let mut store = InMemoryVectorStore::cosine();
    let dimensions = 64;

    // Knowledge base documents
    let documents = vec![
        "MoFA is a modular framework for building AI agents in Rust...",
        "The dual plugin system supports Rust/WASM and Rhai scripts...",
        "MoFA supports seven multi-agent coordination patterns...",
    ];

    // Chunk documents
    let chunker = TextChunker::new(ChunkConfig {
        chunk_size: 200,
        chunk_overlap: 30,
    });

    let mut all_chunks = Vec::new();
    for (doc_idx, document) in documents.iter().enumerate() {
        let text_chunks = chunker.chunk_by_chars(document);
        for (chunk_idx, text) in text_chunks.iter().enumerate() {
            let embedding = generate_embedding(text, dimensions);
            let chunk = DocumentChunk::new(&format!("doc-{doc_idx}-chunk-{chunk_idx}"), text, embedding)
                .with_metadata("source", &format!("document_{doc_idx}"));
            all_chunks.push(chunk);
        }
    }

    // Index chunks
    store.upsert_batch(all_chunks).await?;

    // Search
    let query = "How does MoFA handle multiple agents?";
    let query_embedding = generate_embedding(query, dimensions);
    let results = store.search(&query_embedding, 3, None).await?;

    // Build context for LLM
    let context: String = results.iter()
        .map(|r| r.text.clone())
        .collect::<Vec<_>>()
        .join("\n\n");

    println!("Context for LLM:\n{}", context);
    Ok(())
}
```

## Document Ingestion

Multi-document ingestion with metadata tracking.

```rust
async fn document_ingestion_demo() -> Result<()> {
    let mut store = InMemoryVectorStore::cosine();

    // Simulate ingesting multiple files
    let files = vec![
        ("architecture.md", "The microkernel pattern keeps the core small..."),
        ("plugins.md", "Compile-time plugins use Rust traits..."),
        ("deployment.md", "MoFA agents can be deployed as containers..."),
    ];

    let chunker = TextChunker::new(ChunkConfig::default());

    for (filename, content) in &files {
        let text_chunks = chunker.chunk_by_sentences(content);
        let chunks: Vec<_> = text_chunks.iter().enumerate()
            .map(|(i, text)| {
                let embedding = generate_embedding(text, dimensions);
                DocumentChunk::new(&format!("{filename}-{i}"), text, embedding)
                    .with_metadata("filename", filename)
                    .with_metadata("chunk_index", &i.to_string())
            })
            .collect();
        store.upsert_batch(chunks).await?;
    }

    println!("Store contains {} chunks", store.count().await?);
    Ok(())
}
```

## Qdrant Integration

Production vector store with Qdrant.

```rust
use mofa_foundation::rag::{QdrantConfig, QdrantVectorStore, SimilarityMetric};

async fn qdrant_rag_pipeline(qdrant_url: &str) -> Result<()> {
    let config = QdrantConfig {
        url: qdrant_url.into(),
        api_key: std::env::var("QDRANT_API_KEY").ok(),
        collection_name: "mofa_rag".into(),
        vector_dimensions: 64,
        metric: SimilarityMetric::Cosine,
        create_collection: true,
    };

    let mut store = QdrantVectorStore::new(config).await?;

    // Ingest documents
    let chunks = vec![
        DocumentChunk::new("intro", "MoFA stands for Modular Framework...", embedding)
            .with_metadata("source", "intro"),
        // More chunks...
    ];
    store.upsert_batch(chunks).await?;

    // Search
    let results = store.search(&query_embedding, 5, None).await?;

    // Delete and clear
    store.delete("intro").await?;
    store.clear().await?;

    Ok(())
}
```

## Chunking Strategies

The `TextChunker` supports multiple chunking methods:

```rust
let chunker = TextChunker::new(ChunkConfig {
    chunk_size: 200,      // Target chunk size
    chunk_overlap: 30,    // Overlap between chunks
});

// By characters (fast, simple)
let chunks = chunker.chunk_by_chars(text);

// By sentences (better semantic boundaries)
let chunks = chunker.chunk_by_sentences(text);

// By paragraphs (preserves structure)
let chunks = chunker.chunk_by_paragraphs(text);
```

## Running Examples

```bash
# In-memory mode (no external dependencies)
cargo run -p rag_pipeline

# With Qdrant
docker run -p 6333:6333 -p 6334:6334 qdrant/qdrant
QDRANT_URL=http://localhost:6334 cargo run -p rag_pipeline -- qdrant
```

## Available Examples

| Example | Description |
|---------|-------------|
| `rag_pipeline` | RAG with in-memory and Qdrant backends |

## See Also

- [LLM Providers](../guides/llm-providers.md) — Embedding model configuration
- [API Reference: RAG](../api-reference/foundation/rag.md) — RAG API
