//! code review assistant example - hierarchical compression preserves technical details

use mofa_foundation::agent::TokenCounter;
use crate::helpers::make_msg;

pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    println!("\nscenario: code review assistant");
    println!("use case: technical discussions, hierarchical compression preserves code\n");

    let conversation = vec![
        make_msg("system", "You are a code review assistant. Help developers improve their code quality and security."),
        make_msg("user", "Here's my function:\n```rust\nfn calculate(x: i32, y: i32) -> i32 {\n    x + y\n}\n```"),
        make_msg("assistant", "Consider adding error handling and documentation. Also, what if x + y overflows?"),
        make_msg("user", "Good point. How should I handle overflow?"),
        make_msg("assistant", "Use checked_add() or saturating_add() depending on your use case."),
        make_msg("user", "Thanks! Here's the updated version:\n```rust\nfn calculate(x: i32, y: i32) -> Option<i32> {\n    x.checked_add(y)\n}\n```"),
        make_msg("assistant", "Much better! Consider adding a doc comment explaining the function's purpose."),
        make_msg("user", "Got it. One more question - should I use i32 or i64?"),
        make_msg("assistant", "It depends on your use case. i32 is fine for most cases, but if you expect large numbers, use i64."),
        make_msg("user", "Makes sense. Thanks for the help!"),
        make_msg("assistant", "You're welcome! Happy coding!"),
    ];

    println!("original: {} messages, {} tokens", conversation.len(), TokenCounter::count(&conversation));
    println!("\nhierarchical compression scores by recency (40%), role (30%), density (30%)");
    println!("ensures code snippets and technical details are preserved");

    Ok(())
}
