//! Integration tests for proxy functionality.

use mofa_gateway::gateway::{Gateway, GatewayConfig};
use mofa_gateway::proxy::{LocalLLMBackend, ProxyHandler};
use std::time::Duration;
use tokio::time::sleep;

#[tokio::test]
async fn test_gateway_with_local_llm_proxy_enabled() {
    // Create gateway config with local-llm proxy enabled
    let mut config = GatewayConfig::default();
    config.enable_local_llm_proxy = true;
    config.local_llm_backend_url = Some("http://localhost:8000".to_string());

    // Create gateway
    let mut gateway = Gateway::new(config).await.unwrap();

    // Start gateway (this will register routes)
    let start_result = gateway.start().await;
    
    // Gateway should start successfully even if backend is not available
    assert!(start_result.is_ok());

    // Clean shutdown
    gateway.stop().await.unwrap();
}

#[tokio::test]
async fn test_gateway_with_local_llm_proxy_disabled() {
    // Create gateway config with local-llm proxy disabled
    let mut config = GatewayConfig::default();
    config.enable_local_llm_proxy = false;

    // Create gateway
    let mut gateway = Gateway::new(config).await.unwrap();

    // Start gateway
    let start_result = gateway.start().await;
    assert!(start_result.is_ok());

    // Clean shutdown
    gateway.stop().await.unwrap();
}

#[tokio::test]
async fn test_proxy_handler_health_check_no_backend() {
    // Test health check when backend is not available
    // Use a valid but non-listening port to simulate an unavailable backend
    let backend = LocalLLMBackend::new("http://localhost:9999");
    let handler = ProxyHandler::new(backend.to_proxy_backend());

    // Health check should return false (backend not available) without erroring
    let result = handler
        .health_check()
        .await
        .expect("health check should not error for unreachable backend");
    assert!(
        !result,
        "health check should report backend as unavailable when it cannot be reached"
    );
}

#[tokio::test]
async fn test_proxy_backend_configuration() {
    // Test various backend configurations
    let backend1 = LocalLLMBackend::default();
    assert_eq!(backend1.base_url, "http://localhost:8000");

    let backend2 = LocalLLMBackend::new("http://example.com:9000")
        .with_health_endpoint("/custom/health")
        .with_timeout(Duration::from_secs(120));
    
    assert_eq!(backend2.base_url, "http://example.com:9000");
    assert_eq!(backend2.health_endpoint, "/custom/health");
    assert_eq!(backend2.timeout, Duration::from_secs(120));
}
