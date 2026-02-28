//! customer support agent example - semantic compression for repetitive queries

use mofa_foundation::agent::{ContextCompressor, SlidingWindowCompressor, TokenCounter};
use crate::helpers::{make_msg, print_compression_stats};

pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    println!("\nscenario: customer support agent");
    println!("use case: repetitive queries, semantic compression merges similar questions\n");

    let conversation = vec![
        make_msg("system", "You are a helpful customer support agent for an e-commerce platform. Answer questions clearly and concisely."),
        make_msg("user", "I want to return my order"),
        make_msg("assistant", "I can help you with that. What's your order number?"),
        make_msg("user", "How do I return my order?"),
        make_msg("assistant", "You can return your order by logging into your account and selecting the return option."),
        make_msg("user", "I need to return something I bought"),
        make_msg("assistant", "Sure! Please provide your order number and I'll process the return."),
        make_msg("user", "What's your refund policy?"),
        make_msg("assistant", "We offer full refunds within 30 days of purchase for unused items."),
        make_msg("user", "How long does a refund take?"),
        make_msg("assistant", "Refunds are processed within 5-7 business days after we receive your return."),
        make_msg("user", "Can I return my order?"),
        make_msg("assistant", "Yes, you can return your order within 30 days. Would you like me to start the process?"),
    ];

    println!("original: {} messages, {} tokens", conversation.len(), TokenCounter::count(&conversation));

    let sliding_window = SlidingWindowCompressor::new(4);
    let compressed_sw = sliding_window.compress(conversation.clone(), 200).await?;
    print_compression_stats(&conversation, &compressed_sw, "sliding window");

    println!("\nsemantic compression would merge repetitive return questions");
    println!("into a single representative message, preserving unique info");

    Ok(())
}
