//! research assistant example - summarizing compression for long research sessions

use mofa_foundation::agent::TokenCounter;
use crate::helpers::make_msg;

pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    println!("\nscenario: research assistant");
    println!("use case: long research sessions, summarizing compression condenses history\n");

    let conversation = vec![
        make_msg("system", "You are a research assistant. Help users find and synthesize information from academic sources."),
        make_msg("user", "I'm researching the impact of AI on software development"),
        make_msg("assistant", "Here are some key findings:\n1. AI tools can increase developer productivity by 30-50%\n2. Code generation tools like GitHub Copilot are widely adopted\n3. Concerns about code quality and security remain"),
        make_msg("user", "What about the security concerns specifically?"),
        make_msg("assistant", "Security concerns include:\n- AI-generated code may contain vulnerabilities\n- Developers may become over-reliant on AI\n- Need for better validation and testing"),
        make_msg("user", "Are there studies on developer productivity?"),
        make_msg("assistant", "Yes, several studies show:\n- GitHub Copilot users complete tasks 55% faster\n- Code quality metrics show mixed results\n- Developer satisfaction increases significantly"),
        make_msg("user", "What about code quality specifically?"),
        make_msg("assistant", "Code quality studies indicate:\n- AI-assisted code has similar bug rates\n- Code review time increases initially\n- Long-term quality depends on developer training"),
    ];

    println!("original: {} messages, {} tokens", conversation.len(), TokenCounter::count(&conversation));
    println!("\nsummarizing compression keeps recent questions, summarizes older findings");
    println!("preserves key facts and citations in compact summary");

    Ok(())
}
