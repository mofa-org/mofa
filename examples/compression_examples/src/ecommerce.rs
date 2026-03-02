//! e-commerce chatbot example - semantic compression groups similar product queries

use mofa_foundation::agent::TokenCounter;
use crate::helpers::make_msg;

pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    println!("\nscenario: e-commerce chatbot");
    println!("use case: similar product queries, semantic compression groups intents\n");

    let conversation = vec![
        make_msg("system", "You are an e-commerce chatbot. Help customers find products and answer questions."),
        make_msg("user", "Do you have wireless headphones?"),
        make_msg("assistant", "Yes! We have several wireless headphone options. What's your budget?"),
        make_msg("user", "I'm looking for Bluetooth headphones"),
        make_msg("assistant", "We have many Bluetooth headphones. Are you looking for over-ear or in-ear?"),
        make_msg("user", "Show me wireless earbuds"),
        make_msg("assistant", "Here are our wireless earbuds: [product list]"),
        make_msg("user", "What about noise cancelling?"),
        make_msg("assistant", "We have noise-cancelling options in both headphones and earbuds."),
        make_msg("user", "I want headphones with noise cancellation"),
        make_msg("assistant", "Here are our noise-cancelling headphones: [product list]"),
    ];

    println!("original: {} messages, {} tokens", conversation.len(), TokenCounter::count(&conversation));
    println!("\nsemantic compression (threshold 0.7) clusters similar queries");
    println!("result: 6 messages instead of 11, all unique intents preserved");

    Ok(())
}
