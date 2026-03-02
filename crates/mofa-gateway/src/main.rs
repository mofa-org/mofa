//! MoFA Cognitive Gateway — entry point.
//!
//! Reads configuration from environment variables and starts the axum-based
//! HTTP gateway service.
//!
//! # Environment variables
//!
//! | Variable | Default | Description |
//! |----------|---------|-------------|
//! | `GATEWAY_PORT` | `3000` | TCP port to listen on. |
//! | `OPENAI_API_KEY` | *(none)* | OpenAI API key forwarded to the backend. |
//! | `OPENAI_BASE_URL` | `https://api.openai.com` | OpenAI-compatible base URL. |
//! | `GATEWAY_API_KEYS` | *(none)* | Comma-separated list of valid gateway API keys. |
//! | `RATE_PER_SECOND` | `100` | Sustained request rate per caller. |
//! | `BURST_CAPACITY` | `200` | Burst capacity per caller. |

use mofa_gateway::server::{GatewayServer, GatewayServerConfig};
use mofa_kernel::gateway::{
    BackendKind, CapabilityDescriptor, GatewayConfig, RouteConfig,
};
use tracing::info;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    // Initialise structured logging.
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("mofa_gateway=info".parse().unwrap()))
        .init();

    let port: u16 = std::env::var("GATEWAY_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3000);

    let openai_api_key = std::env::var("OPENAI_API_KEY").ok();
    let openai_base_url = std::env::var("OPENAI_BASE_URL")
        .unwrap_or_else(|_| "https://api.openai.com".to_string());

    let api_keys: Vec<String> = std::env::var("GATEWAY_API_KEYS")
        .unwrap_or_default()
        .split(',')
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.trim().to_string())
        .collect();

    let rate_per_second: u32 = std::env::var("RATE_PER_SECOND")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(100);

    let burst_capacity: u32 = std::env::var("BURST_CAPACITY")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(200);

    if api_keys.is_empty() {
        tracing::warn!(
            "GATEWAY_API_KEYS is not set — authentication is DISABLED. \
             Do not use this configuration in production."
        );
    }

    // Build the gateway configuration.
    let gateway_config = GatewayConfig::new("mofa-cognitive-gateway")
        .with_backend(
            CapabilityDescriptor::new("openai", BackendKind::LlmOpenAI, &openai_base_url)
                .with_health_check("/v1/models"),
        )
        .with_route(
            RouteConfig::new("chat-completions", "/v1/chat/completions", "openai")
                .with_methods(vec![mofa_kernel::gateway::HttpMethod::Post])
                .with_timeout_ms(120_000)
                .with_priority(10),
        )
        .with_route(
            RouteConfig::new("list-models", "/v1/models", "openai")
                .with_methods(vec![mofa_kernel::gateway::HttpMethod::Get])
                .with_priority(5),
        )
        .with_route(
            RouteConfig::new("embeddings", "/v1/embeddings", "openai")
                .with_methods(vec![mofa_kernel::gateway::HttpMethod::Post])
                .with_timeout_ms(60_000)
                .with_priority(5),
        );

    info!(
        port = port,
        openai_base_url = %openai_base_url,
        auth_enabled = !api_keys.is_empty(),
        "MoFA Cognitive Gateway configuration loaded"
    );

    let server = GatewayServer::new(GatewayServerConfig {
        port,
        api_keys,
        openai_api_key,
        openai_base_url,
        rate_per_second,
        burst_capacity,
    });

    if let Err(e) = server.start(gateway_config).await {
        eprintln!("Gateway error: {e}");
        std::process::exit(1);
    }
}

