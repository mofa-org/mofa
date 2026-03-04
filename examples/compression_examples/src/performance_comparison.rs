//! performance comparison example - comparing compression strategies

use mofa_foundation::agent::{ContextCompressor, SlidingWindowCompressor, TokenCounter};
use crate::helpers::{make_msg, print_compression_stats};

pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    println!("\nscenario: performance comparison");
    println!("compare strategies on same conversation\n");

    let conversation = vec![
        make_msg("system", "You are a helpful assistant."),
        make_msg("user", "What is Rust?"),
        make_msg("assistant", "Rust is a systems programming language focused on safety and performance."),
        make_msg("user", "Tell me about Rust"),
        make_msg("assistant", "Rust provides memory safety without garbage collection through ownership."),
        make_msg("user", "What can you tell me about the Rust programming language?"),
        make_msg("assistant", "Rust has zero-cost abstractions and prevents data races at compile time."),
        make_msg("user", "How do I learn Rust?"),
        make_msg("assistant", "Start with 'The Rust Book' and practice with small projects."),
        make_msg("user", "Where can I learn Rust?"),
        make_msg("assistant", "The official Rust website has excellent learning resources."),
    ];

    let budget = 100;
    let tokens_before = TokenCounter::count(&conversation);

    println!("test: {} messages, {} tokens, budget: {}", conversation.len(), tokens_before, budget);

    let sw = SlidingWindowCompressor::new(4);
    let compressed_sw = sw.compress(conversation.clone(), budget).await?;
    print_compression_stats(&conversation, &compressed_sw, "sliding window");

    println!("\nstrategy comparison:");
    println!("  sliding window: <1ms, simple truncation");
    println!("  summarizing: 200-1s, preserve semantics");
    println!("  semantic: 100-500ms, remove redundancy");
    println!("  hierarchical: 200-1s, preserve important");
    println!("  hybrid: variable, adapt to context");

    Ok(())
}
