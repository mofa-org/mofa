//! real-world integration example - integrating compression into agent executor

pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    println!("\nscenario: real-world integration");
    println!("\nexample: customer support agent with semantic compression");
    println!("{}", r#"
use mofa_foundation::agent::{AgentExecutor, AgentExecutorConfig, SemanticCompressor};
use mofa_foundation::llm::openai::{OpenAIConfig, OpenAIProvider};
use std::sync::Arc;

let llm_provider = Arc::new(OpenAIProvider::with_config(
    OpenAIConfig::new(api_key).with_model("gpt-4o-mini")
));

let compressor = Arc::new(
    SemanticCompressor::new(llm_provider.clone())
        .with_similarity_threshold(0.80)
        .with_keep_recent(5)
);

let executor = AgentExecutor::with_config(
    llm_provider,
    workspace_path,
    AgentExecutorConfig::new()
        .with_max_context_tokens(4000)
        .with_model("gpt-4o-mini"),
)
.await?
.with_compressor(compressor);

let response = executor
    .process_message("session-123", "I want to return my order")
    .await?;
"#);
    println!("\nbenefits: automatic compression, no code changes, configurable");

    Ok(())
}
