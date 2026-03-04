# RAG 与知识库

使用向量存储的检索增强生成（RAG）示例。

## 基础 RAG 流水线

使用内存向量存储的文档分块和语义搜索。

**位置：** `examples/rag_pipeline/`

```rust
use mofa_foundation::rag::{
    ChunkConfig, DocumentChunk, InMemoryVectorStore,
    TextChunker, VectorStore,
};

async fn basic_rag_pipeline() -> Result<()> {
    // 创建余弦相似度向量存储
    let mut store = InMemoryVectorStore::cosine();
    let dimensions = 64;

    // 知识库文档
    let documents = vec![
        "MoFA 是用 Rust 构建模块化 AI 智能体的框架...",
        "双层插件系统支持 Rust/WASM 和 Rhai 脚本...",
        "MoFA 支持七种多智能体协调模式...",
    ];

    // 分块文档
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

    // 索引分块
    store.upsert_batch(all_chunks).await?;

    // 搜索
    let query = "MoFA 如何处理多个智能体？";
    let query_embedding = generate_embedding(query, dimensions);
    let results = store.search(&query_embedding, 3, None).await?;

    // 构建 LLM 上下文
    let context: String = results.iter()
        .map(|r| r.text.clone())
        .collect::<Vec<_>>()
        .join("\n\n");

    println!("LLM 上下文:\n{}", context);
    Ok(())
}
```

## 文档摄入

带元数据跟踪的多文档摄入。

```rust
async fn document_ingestion_demo() -> Result<()> {
    let mut store = InMemoryVectorStore::cosine();

    // 模拟摄入多个文件
    let files = vec![
        ("architecture.md", "微内核模式保持核心小巧可扩展..."),
        ("plugins.md", "编译时插件使用 Rust trait 实现零成本抽象..."),
        ("deployment.md", "MoFA 智能体可部署为容器..."),
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

    println!("存储包含 {} 个分块", store.count().await?);
    Ok(())
}
```

## Qdrant 集成

使用 Qdrant 的生产级向量存储。

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

    // 摄入文档
    let chunks = vec![
        DocumentChunk::new("intro", "MoFA 代表模块化智能体框架...", embedding)
            .with_metadata("source", "intro"),
        // 更多分块...
    ];
    store.upsert_batch(chunks).await?;

    // 搜索
    let results = store.search(&query_embedding, 5, None).await?;

    // 删除和清空
    store.delete("intro").await?;
    store.clear().await?;

    Ok(())
}
```

## 分块策略

`TextChunker` 支持多种分块方法：

```rust
let chunker = TextChunker::new(ChunkConfig {
    chunk_size: 200,      // 目标分块大小
    chunk_overlap: 30,    // 分块重叠
});

// 按字符（快速、简单）
let chunks = chunker.chunk_by_chars(text);

// 按句子（更好的语义边界）
let chunks = chunker.chunk_by_sentences(text);

// 按段落（保留结构）
let chunks = chunker.chunk_by_paragraphs(text);
```

## 运行示例

```bash
# 内存模式（无外部依赖）
cargo run -p rag_pipeline

# 使用 Qdrant
docker run -p 6333:6333 -p 6334:6334 qdrant/qdrant
QDRANT_URL=http://localhost:6334 cargo run -p rag_pipeline -- qdrant
```

## 可用示例

| 示例 | 描述 |
|------|------|
| `rag_pipeline` | 内存和 Qdrant 后端的 RAG |

## 相关链接

- [LLM 提供商](../guides/llm-providers.md) — 嵌入模型配置
- [API 参考：RAG](../api-reference/foundation/rag.md) — RAG API
