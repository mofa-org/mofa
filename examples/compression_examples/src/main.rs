//! codex-style context compression practical examples

mod customer_support;
mod code_review;
mod research_assistant;
mod ecommerce;
mod hybrid_strategy;
mod integration;
mod performance_comparison;
mod helpers;
#[cfg(feature = "compression-cache")]
mod caching_example;

use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    info!("starting codex-style compression practical examples");

    let args: Vec<String> = std::env::args().collect();
    let scenario_arg = args.iter().find(|a| a.starts_with("--scenario"));

    println!("\nmofa codex-style context compression - practical examples");

    if let Some(scenario) = scenario_arg {
        let name = scenario.split('=').nth(1).unwrap_or("");
        match name {
            "customer-support" => customer_support::run().await?,
            "code-review" => code_review::run().await?,
            "research" => research_assistant::run().await?,
            "ecommerce" => ecommerce::run().await?,
            "hybrid" => hybrid_strategy::run().await?,
            "integration" => integration::run().await?,
            "comparison" => performance_comparison::run().await?,
            #[cfg(feature = "compression-cache")]
            "caching" => caching_example::run().await?,
            _ => {
                println!("unknown scenario: {name}");
                println!("available: customer-support, code-review, research, ecommerce, hybrid, integration, comparison");
            }
        }
    } else {
        customer_support::run().await?;
        code_review::run().await?;
        research_assistant::run().await?;
        ecommerce::run().await?;
        hybrid_strategy::run().await?;
        integration::run().await?;
        performance_comparison::run().await?;
        #[cfg(feature = "compression-cache")]
        caching_example::run().await?;
    }

    println!("\nexamples complete!");

    Ok(())
}
