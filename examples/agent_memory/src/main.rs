//! Agent long-term memory example
//!
//! Demonstrates the two memory backends added in this PR:
//!
//! - **EpisodicMemory**: stores conversation turns as timestamped episodes and
//!   supports cross-session recall (retrieve recent turns from any past session).
//!
//! - **SemanticMemory** with **HashEmbedder**: stores memory entries as
//!   embedding vectors and retrieves the most semantically similar ones given a
//!   natural-language query — no external API required.
//!
//! Run:
//! ```
//! cargo run -p agent_memory
//! ```

use anyhow::Result;
use mofa_foundation::agent::{
    Embedder,
    EpisodicMemory, HashEmbedder, MemoryValue, Message, SemanticMemory,
};
use mofa_kernel::agent::components::memory::Memory;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .init();

    demo_episodic_memory().await?;
    demo_semantic_memory().await?;
    demo_hash_embedder_similarity().await?;

    Ok(())
}

// ============================================================================
// Demo 1: EpisodicMemory — cross-session conversation recall
// ============================================================================

async fn demo_episodic_memory() -> Result<()> {
    println!("\n=== Demo 1: EpisodicMemory ===\n");

    let mut memory = EpisodicMemory::new();

    // Simulate a conversation from session 1 (yesterday)
    memory
        .add_to_history("session-2026-02-24", Message::user("what is rust?"))
        .await?;
    memory
        .add_to_history(
            "session-2026-02-24",
            Message::assistant(
                "Rust is a systems programming language focused on safety and performance.",
            ),
        )
        .await?;
    memory
        .add_to_history(
            "session-2026-02-24",
            Message::user("does rust have a garbage collector?"),
        )
        .await?;
    memory
        .add_to_history(
            "session-2026-02-24",
            Message::assistant("No, Rust uses an ownership model instead of a GC."),
        )
        .await?;

    // Simulate a conversation from session 2 (this morning)
    memory
        .add_to_history(
            "session-2026-02-25",
            Message::user("remind me what we discussed about rust"),
        )
        .await?;
    memory
        .add_to_history(
            "session-2026-02-25",
            Message::assistant("We covered Rust's ownership model and lack of GC."),
        )
        .await?;

    println!(
        "Total episodes stored across all sessions: {}",
        memory.total_episodes()
    );
    println!("Sessions: {:?}", memory.session_ids());

    // Cross-session recall: fetch the 3 most recent episodes regardless of session
    println!("\nMost recent 3 episodes (cross-session):");
    for ep in memory.get_recent_episodes(3) {
        println!(
            "  [{}] {}: {}",
            ep.session_id, ep.message.role, ep.message.content
        );
    }

    // Keyword search across all sessions
    println!("\nSearch results for 'garbage collector':");
    let results = memory.search("garbage collector", 5).await?;
    for item in &results {
        println!(
            "  key={} text={}",
            item.key,
            item.value.as_text().unwrap_or("")
        );
    }
    println!("Found {} result(s)", results.len());

    // Per-session history
    println!("\nFull history for session-2026-02-24:");
    for msg in memory.get_history("session-2026-02-24").await? {
        println!("  {}: {}", msg.role, msg.content);
    }

    let stats = memory.stats().await?;
    println!(
        "\nStats: {} session(s), {} total message(s)",
        stats.total_sessions, stats.total_messages
    );

    Ok(())
}

// ============================================================================
// Demo 2: SemanticMemory — vector-similarity retrieval with HashEmbedder
// ============================================================================

async fn demo_semantic_memory() -> Result<()> {
    println!("\n=== Demo 2: SemanticMemory with HashEmbedder ===\n");

    let mut memory = SemanticMemory::with_hash_embedder();

    // Store a variety of memory entries
    let entries = [
        ("rust-ownership", "Rust uses an ownership model to guarantee memory safety without a garbage collector"),
        ("rust-traits", "Traits in Rust are similar to interfaces in other languages and enable polymorphism"),
        ("python-gc", "Python uses reference counting combined with a cyclic garbage collector"),
        ("mofa-arch", "MoFA follows a microkernel architecture with a dual-layer plugin system"),
        ("mofa-memory", "MoFA agents can use EpisodicMemory for cross-session recall and SemanticMemory for similarity search"),
        ("cooking-pasta", "To make pasta carbonara you need eggs, pecorino cheese, guanciale, and black pepper"),
    ];

    for (key, text) in &entries {
        memory.store(key, MemoryValue::text(*text)).await?;
    }

    println!("Stored {} memory entries.\n", entries.len());

    // Query 1: Rust memory model
    let query = "how does rust handle memory without gc";
    println!("Query: \"{query}\"");
    let results = memory.search(query, 3).await?;
    for (i, item) in results.iter().enumerate() {
        let sim = item.metadata.get("similarity").map(|s| s.as_str()).unwrap_or("?");
        println!(
            "  {}. [sim={}] {}: {}",
            i + 1,
            sim,
            item.key,
            item.value.as_text().unwrap_or("")
        );
    }

    println!();

    // Query 2: MoFA framework
    let query2 = "mofa agent framework architecture";
    println!("Query: \"{query2}\"");
    let results2 = memory.search(query2, 3).await?;
    for (i, item) in results2.iter().enumerate() {
        let sim = item.metadata.get("similarity").map(|s| s.as_str()).unwrap_or("?");
        println!(
            "  {}. [sim={}] {}: {}",
            i + 1,
            sim,
            item.key,
            item.value.as_text().unwrap_or("")
        );
    }

    let stats = memory.stats().await?;
    println!(
        "\nStats: {} vector entries indexed",
        stats.total_items
    );

    Ok(())
}

// ============================================================================
// Demo 3: HashEmbedder similarity intuition
// ============================================================================

async fn demo_hash_embedder_similarity() -> Result<()> {
    println!("\n=== Demo 3: HashEmbedder similarity intuition ===\n");

    let embedder = Arc::new(HashEmbedder::new(128));

    let texts = [
        "rust programming language systems",
        "rust ownership borrowing lifetime",
        "python machine learning data science",
        "bake a chocolate cake recipe",
    ];

    let anchor = "rust systems programming";
    println!("Anchor: \"{anchor}\"");
    println!("Embedding dimensions: {}\n", embedder.dimensions());

    let anchor_vec: Vec<f32> = embedder.embed(anchor).await?;

    for text in &texts {
        let vec: Vec<f32> = embedder.embed(text).await?;
        let cosine: f32 = anchor_vec.iter().zip(vec.iter()).map(|(a, b)| a * b).sum();
        println!("  sim({anchor:?}, {text:?}) = {cosine:.4}");
    }

    println!("\nTexts about Rust score higher than unrelated topics.");

    Ok(())
}
