# RAG Pipeline Example

This example demonstrates **practical RAG workflows** on top of MoFA's current RAG abstractions:

- In-memory RAG pipeline
- Document ingestion with metadata
- Real-world customer support retrieval scenario
- Optional Qdrant backend run
- Deterministic practical validation suite

## Run

```bash
cargo run -p rag_pipeline
```

## Practical validation mode

This mode runs deterministic checks useful for local verification and CI-style confidence:

```bash
cargo run -p rag_pipeline -- validate
```

Checks performed:

1. Ranking sanity check for a known query/document pair
2. Embedding-dimension validation error path

## Qdrant mode

```bash
docker run -p 6333:6333 -p 6334:6334 qdrant/qdrant
QDRANT_URL=http://localhost:6334 cargo run -p rag_pipeline -- qdrant
```

## Why this matters

These scenarios provide practical, reviewer-friendly evidence that:

- retrieval behavior is not only unit-tested but exerciseable end-to-end,
- metadata survives ingestion and retrieval,
- validation paths are enforced for malformed embeddings,
- the same high-level flow works with both in-memory and external vector backends.
