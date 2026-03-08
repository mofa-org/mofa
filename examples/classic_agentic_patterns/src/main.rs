//! Showcase for classic agentic patterns in MoFA.

use mofa_sdk::llm::{MockLLMProvider, simple_llm_agent};
use mofa_sdk::patterns::{ChainOfThought, Router, RouterConfig};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    println!("==================================================");
    println!("MoFA Classic Agentic Patterns Showcase");
    println!("==================================================");
    println!();

    run_chain_of_thought_demo().await?;
    println!();
    run_router_demo().await?;

    Ok(())
}

async fn run_chain_of_thought_demo() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- Chain-of-Thought Demo ---");

    let provider = Arc::new(MockLLMProvider::new("cot"));
    provider
        .add_response("First, identify the operational constraints and what can fail under load.")
        .await;
    provider
        .add_response("Next, compare the cost of immediate retries versus staggered retries.")
        .await;
    provider
        .add_response("Finally, connect retry timing to service recovery and queue stability.")
        .await;
    provider
        .add_response(
            "Backoff matters because it reduces synchronized retry storms, gives dependencies time to recover, and improves success probability under contention.",
        )
        .await;

    let chain = ChainOfThought::builder()
        .with_llm(Arc::new(simple_llm_agent(
            "cot-thinker",
            provider,
            "You reason in crisp operational steps.",
        )))
        .with_steps(3)
        .with_verbose(false)
        .build()?;

    let result = chain
        .run("Explain why retry backoff improves resilience in distributed systems.")
        .await?;

    println!("{}", result.to_markdown_trace());
    Ok(())
}

async fn run_router_demo() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- Router Demo ---");

    let classifier_provider = Arc::new(MockLLMProvider::new("classifier"));
    classifier_provider
        .add_response(
            r#"{"route":"technical","reason":"the task asks about implementation strategy","confidence":0.94}"#,
        )
        .await;
    let technical_provider = Arc::new(MockLLMProvider::new("technical"));
    technical_provider
        .add_response(
            "Technical expert: build the prototype around the smallest stable API, then add typed traces and tests before expanding scope.",
        )
        .await;
    let billing_provider = Arc::new(MockLLMProvider::new("billing"));
    billing_provider
        .add_response("Billing expert: check invoice history and duplicate payment events.")
        .await;
    let general_provider = Arc::new(MockLLMProvider::new("general"));
    general_provider
        .add_response("General expert: clarify the request and route it manually.")
        .await;

    let router = Router::builder()
        .with_classifier(Arc::new(simple_llm_agent(
            "classifier",
            classifier_provider,
            "You classify tasks to the best expert.",
        )))
        .with_route_llm(
            "technical",
            Arc::new(simple_llm_agent(
                "technical",
                technical_provider,
                "You answer technical design questions.",
            )),
        )
        .describe_route(
            "technical",
            "Engineering strategy, implementation design, and tradeoffs",
        )
        .with_route_llm(
            "billing",
            Arc::new(simple_llm_agent(
                "billing",
                billing_provider,
                "You answer invoice and payment questions.",
            )),
        )
        .describe_route(
            "billing",
            "Invoices, billing corrections, and payment questions",
        )
        .with_default_llm(Arc::new(simple_llm_agent(
            "general",
            general_provider,
            "You provide fallback help when no expert matches.",
        )))
        .with_config(RouterConfig::default().with_verbose(false))
        .build()?;

    let result = router
        .run("How should we scope a new idea-8 pattern contribution so it is distinct and reviewable?")
        .await?;

    println!("{}", result.to_markdown_trace());
    Ok(())
}
