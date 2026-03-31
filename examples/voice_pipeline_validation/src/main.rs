//! Command-line entry point for the voice pipeline validation demo.

use anyhow::Context;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_target(false)
        .without_time()
        .init();

    let events = voice_pipeline_validation::run_validation_demo(
        voice_pipeline_validation::DemoConfig::default(),
    )
    .await
    .context("validation demo failed")?;

    for event in events {
        println!("[{}] {}", event.kind(), event.message());
    }

    Ok(())
}
