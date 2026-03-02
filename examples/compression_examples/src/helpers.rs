//! shared helper functions

use mofa_kernel::agent::types::ChatMessage;

pub fn make_msg(role: &str, content: &str) -> ChatMessage {
    ChatMessage {
        role: role.to_string(),
        content: Some(content.to_string()),
        tool_call_id: None,
        tool_calls: None,
    }
}

pub fn print_compression_stats(
    original: &[ChatMessage],
    compressed: &[ChatMessage],
    compressor_name: &str,
) {
    use mofa_foundation::agent::TokenCounter;
    
    let tokens_before = TokenCounter::count(original);
    let tokens_after = TokenCounter::count(compressed);
    let reduction = if tokens_before > 0 {
        100.0 * (1.0 - tokens_after as f64 / tokens_before as f64)
    } else {
        0.0
    };

    println!("\ncompression results ({compressor_name}):");
    println!("  messages: {} → {} ({:.0}% reduction)", 
        original.len(), compressed.len(),
        100.0 * (1.0 - compressed.len() as f64 / original.len() as f64));
    println!("  tokens: {} → {} ({:.0}% reduction)", 
        tokens_before, tokens_after, reduction);
}
