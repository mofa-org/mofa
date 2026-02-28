//! hybrid strategy example - combining multiple compression strategies

use mofa_foundation::agent::{ContextCompressor, SlidingWindowCompressor, TokenCounter};
use crate::helpers::{make_msg, print_compression_stats};

pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    println!("\nscenario: hybrid strategy");
    println!("use case: unpredictable patterns, tries multiple strategies\n");

    let conversation = vec![
        make_msg("system", "You are a versatile AI assistant helping with various tasks."),
        make_msg("user", "What's the weather today?"),
        make_msg("assistant", "I don't have access to real-time weather data. Please check a weather service."),
        make_msg("user", "Can you help me write code?"),
        make_msg("assistant", "Absolutely! What programming language are you using?"),
        make_msg("user", "Rust"),
        make_msg("assistant", "Great! Rust is excellent for systems programming. What do you need help with?"),
        make_msg("user", "How do I handle errors?"),
        make_msg("assistant", "Rust uses Result<T, E> for error handling. You can use match, unwrap, or the ? operator."),
        make_msg("user", "What's the weather today?"),
        make_msg("assistant", "As mentioned earlier, I don't have access to weather data."),
        make_msg("user", "Can you help me write code?"),
        make_msg("assistant", "Yes, I can help with Rust code. What specific problem are you solving?"),
    ];

    println!("original: {} messages, {} tokens", conversation.len(), TokenCounter::count(&conversation));
    println!("\nhybrid: tries semantic first, then summarization, then sliding window");
    println!("ensures optimal compression regardless of conversation type");

    let sliding_window = SlidingWindowCompressor::new(6);
    let compressed = sliding_window.compress(conversation.clone(), 150).await?;
    print_compression_stats(&conversation, &compressed, "hybrid fallback");

    Ok(())
}
