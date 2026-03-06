//! Unit tests for proxy functionality.

use mofa_gateway::proxy::{LocalLLMBackend, ProxyBackend, ProxyHandler};
use std::time::Duration;

#[test]
fn test_proxy_backend_new() {
    let backend = ProxyBackend::new("test-backend", "http://localhost:8000");
    assert_eq!(backend.name, "test-backend");
    assert_eq!(backend.base_url, "http://localhost:8000");
    assert_eq!(backend.timeout, Duration::from_secs(60));
    assert_eq!(backend.retries, 3);
}

#[test]
fn test_proxy_backend_with_options() {
    let backend = ProxyBackend::new("test", "http://localhost:8000")
        .with_health_check("/health")
        .with_timeout(Duration::from_secs(30))
        .with_retries(5);

    assert_eq!(backend.health_check_endpoint, Some("/health".to_string()));
    assert_eq!(backend.timeout, Duration::from_secs(30));
    assert_eq!(backend.retries, 5);
}

#[test]
fn test_local_llm_backend_default() {
    let backend = LocalLLMBackend::default();
    assert_eq!(backend.base_url, "http://localhost:8000");
    assert_eq!(backend.health_endpoint, "/health");
    assert_eq!(backend.timeout, Duration::from_secs(60));
}

#[test]
fn test_local_llm_backend_new() {
    let backend = LocalLLMBackend::new("http://example.com:9000");
    assert_eq!(backend.base_url, "http://example.com:9000");
}

#[test]
fn test_local_llm_backend_url_for() {
    let backend = LocalLLMBackend::default();
    assert_eq!(backend.url_for("v1/models"), "http://localhost:8000/v1/models");
    assert_eq!(backend.url_for("/v1/models"), "http://localhost:8000/v1/models");
}

#[test]
fn test_local_llm_backend_health_url() {
    let backend = LocalLLMBackend::default();
    assert_eq!(backend.health_url(), "http://localhost:8000/health");
}

#[test]
fn test_local_llm_backend_to_proxy_backend() {
    let backend = LocalLLMBackend::default();
    let proxy_backend = backend.to_proxy_backend();
    assert_eq!(proxy_backend.name, "mofa-local-llm");
    assert_eq!(proxy_backend.base_url, "http://localhost:8000");
    assert_eq!(proxy_backend.health_check_endpoint, Some("/health".to_string()));
}

#[test]
fn test_proxy_handler_new() {
    let backend = ProxyBackend::new("test", "http://localhost:8000");
    let handler = ProxyHandler::new(backend);
    assert_eq!(handler.backend_name(), "test");
    assert_eq!(handler.backend_url(), "http://localhost:8000");
}

#[test]
fn test_proxy_handler_clone() {
    let backend = ProxyBackend::new("test", "http://localhost:8000");
    let handler1 = ProxyHandler::new(backend.clone());
    let handler2 = handler1.clone();
    assert_eq!(handler1.backend_name(), handler2.backend_name());
    assert_eq!(handler1.backend_url(), handler2.backend_url());
}
