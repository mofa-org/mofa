//! Context window management demo
//!
//! Shows how token-budget-aware context window management prevents
//! silent failures when conversation history exceeds model limits.
//!
//! Scenario: A customer support chatbot with long conversation history.
//! Without management, the 50th message would silently exceed GPT-4's
//! 8K context window. With SlidingWindow policy, old messages are
//! automatically trimmed while preserving the system prompt and recent context.
//!
//! Run: `cargo run -p context_window_demo`

use mofa_foundation::llm::token_budget::{
    CharBasedEstimator, ContextWindowManager, ContextWindowPolicy,
};
use mofa_foundation::llm::types::ChatMessage;

fn main() {
    println!("=== Context Window Management Demo ===\n");

    // ── Simulate a long customer support conversation ──────────────
    let mut messages = Vec::new();

    // System prompt (always preserved)
    messages.push(ChatMessage::system(
        "You are a helpful customer support agent for AcmeCorp. \
         Be polite, concise, and resolve issues efficiently.",
    ));

    // 30 turns of conversation history
    let exchanges = [
        ("I can't log into my account", "I'd be happy to help. What email did you use to register?"),
        ("john@example.com", "Let me look that up. I see your account was created on Jan 15."),
        ("Yes that's correct", "Your account appears to be locked due to multiple failed attempts."),
        ("I didn't try to log in multiple times", "This sometimes happens with automated bots. I'll unlock it."),
        ("Thank you", "Done! You should be able to log in now. Try clearing your browser cache too."),
        ("It still doesn't work", "Can you try an incognito/private window?"),
        ("Still failing", "Let me reset your password. You'll receive an email shortly."),
        ("I got the email", "Great! Use the link to set a new password."),
        ("Done, I'm in now", "Excellent! Is there anything else I can help with?"),
        ("Yes, I want to upgrade my plan", "Sure! We have Basic ($10/mo), Pro ($25/mo), and Enterprise ($99/mo)."),
        ("What's the difference between Pro and Enterprise?", "Pro includes 100GB storage. Enterprise adds priority support and custom integrations."),
        ("I'll go with Pro", "Great choice! I've upgraded your account to Pro. You'll see the change on your next bill."),
        ("When is my next bill?", "Your next billing date is March 1st, 2026."),
        ("Can I get a discount?", "I can offer 20% off for the first 3 months with code WELCOME20."),
        ("Applied, thanks!", "You're welcome! The discount is now active on your account."),
    ];

    for (user_msg, assistant_msg) in &exchanges {
        messages.push(ChatMessage::user(*user_msg));
        messages.push(ChatMessage::assistant(*assistant_msg));
    }

    // Current user message (always preserved)
    messages.push(ChatMessage::user(
        "One more thing - how do I enable two-factor authentication?",
    ));

    println!(
        "Total messages in conversation: {}\n",
        messages.len()
    );

    // ── Policy: None (current default behavior) ───────────────────
    println!("--- Policy: None (no management) ---");
    let manager = ContextWindowManager::new(2048);
    let result = manager.apply(&messages);
    println!(
        "  Messages sent: {} (all {} kept)",
        result.messages.len(),
        messages.len()
    );
    println!("  Estimated tokens: {}", result.estimated_tokens);
    println!(
        "  PROBLEM: {} tokens exceeds 2048 budget!\n",
        result.estimated_tokens
    );

    // ── Policy: SlidingWindow (recommended) ───────────────────────
    println!("--- Policy: SlidingWindow (keep last 6 messages) ---");
    let manager = ContextWindowManager::new(2048)
        .with_policy(ContextWindowPolicy::SlidingWindow { keep_last_n: 6 });
    let result = manager.apply(&messages);
    println!("  Messages sent: {} (trimmed from {})", result.messages.len(), messages.len());
    println!("  Messages dropped: {}", result.dropped_count);
    println!("  Estimated tokens: {} (budget: 2048)", result.estimated_tokens);
    println!("  Was trimmed: {}", result.was_trimmed);
    println!("  First msg role: {:?} (system prompt preserved)", result.messages[0].role);
    println!(
        "  Last msg role: {:?} (current question preserved)\n",
        result.messages[result.messages.len() - 1].role
    );

    // ── Policy: Strict (for RAG pipelines) ────────────────────────
    println!("--- Policy: Strict (validation only) ---");
    let manager = ContextWindowManager::new(2048)
        .with_policy(ContextWindowPolicy::Strict);
    let result = manager.apply(&messages);
    println!("  Messages sent: {} (no trimming)", result.messages.len());
    println!(
        "  Estimated tokens: {} (exceeds budget: {})",
        result.estimated_tokens,
        result.estimated_tokens > 2048
    );
    println!("  Use case: catch oversized contexts early in RAG pipelines\n");

    // ── Custom estimator ──────────────────────────────────────────
    println!("--- Custom Estimator (2 chars/token for CJK text) ---");
    let manager = ContextWindowManager::new(4096)
        .with_policy(ContextWindowPolicy::SlidingWindow { keep_last_n: 4 })
        .with_estimator(Box::new(CharBasedEstimator::new(2)));
    let result = manager.apply(&messages);
    println!("  Messages sent: {} (CJK needs ~2 chars/token)", result.messages.len());
    println!("  Estimated tokens: {}\n", result.estimated_tokens);

    // ── Model-specific budgets ────────────────────────────────────
    println!("--- Model Context Windows ---");
    let models = [
        ("GPT-4", 8_192),
        ("GPT-4-Turbo", 128_000),
        ("Claude-3-Opus", 200_000),
        ("Llama-3-8B", 8_192),
        ("Ollama local", 4_096),
    ];

    for (name, budget) in &models {
        let mgr = ContextWindowManager::new(*budget)
            .with_policy(ContextWindowPolicy::SlidingWindow { keep_last_n: 4 });
        let result = mgr.apply(&messages);
        println!(
            "  {:<15} ({:>7} tokens): {} msgs kept, {} dropped",
            name, budget, result.messages.len(), result.dropped_count
        );
    }

    println!("\n=== Demo Complete ===");
}
