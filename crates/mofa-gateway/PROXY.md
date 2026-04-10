# MoFA Gateway - HTTP Proxy for mofa-local-llm

Complete guide for using the gateway's HTTP proxy feature to integrate with mofa-local-llm.

## Table of Contents

- [Overview](#overview)
- [Quick Start](#quick-start)
- [Configuration](#configuration)
- [API Endpoints](#api-endpoints)
- [Client Examples](#client-examples)
- [Features](#features)
- [Architecture](#architecture)
- [Testing](#testing)
- [Troubleshooting](#troubleshooting)

---

## Overview

The MoFA Gateway includes a production-ready HTTP proxy that forwards requests to the `mofa-local-llm` server. This provides:

- **OpenAI-compatible API** - Drop-in replacement for OpenAI endpoints
- **Health checking** - Automatic backend health monitoring
- **Circuit breakers** - Fault tolerance and automatic recovery
- **Rate limiting** - Request throttling and protection
- **Distributed tracing** - Full observability of proxy requests
- **Load balancing** - Support for multiple backend instances (future)

### Why Use the Proxy?

1. **Centralized Management** - Single entry point for all LLM requests
2. **Fault Tolerance** - Circuit breakers prevent cascading failures
3. **Observability** - Metrics, tracing, and logging built-in
4. **Security** - Add authentication, rate limiting, and access control
5. **Flexibility** - Easy to add caching, A/B testing, and more

---

## Quick Start

### 1. Start mofa-local-llm Server

```bash
# Start the mofa-local-llm HTTP server
cd mofa-local-llm
cargo run --release

# Server runs on http://localhost:8000 by default
```

### 2. Start Gateway with Proxy Enabled

```bash
# Run the example (from workspace root)
cd crates/mofa-gateway
cargo run --example gateway_local_llm_proxy

# Gateway runs on http://localhost:8080
```

### 3. Make Requests

```bash
# List available models
curl http://localhost:8080/v1/models

# Get specific model info
curl http://localhost:8080/v1/models/qwen2.5-0.5b-instruct

# Chat completion
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "qwen2.5-0.5b-instruct",
    "messages": [{"role": "user", "content": "Hello!"}],
    "max_tokens": 100
  }'
```

---

## Configuration

### Basic Configuration

```rust
use mofa_gateway::{Gateway, GatewayConfig};
use mofa_gateway::types::LoadBalancingAlgorithm;

let config = GatewayConfig {
    listen_addr: "0.0.0.0:8080".parse().unwrap(),
    load_balancing: LoadBalancingAlgorithm::RoundRobin,
    enable_rate_limiting: true,
    enable_circuit_breakers: true,
    enable_local_llm_proxy: true,
    local_llm_backend_url: Some("http://localhost:8000".to_string()),
};

let mut gateway = Gateway::new(config).await?;
gateway.start().await?;
```

### Environment Variables

```bash
# Enable/disable local LLM proxy (default: false)
export MOFA_LOCAL_LLM_ENABLED="true"

# Backend URL (default: http://localhost:8000)
export MOFA_LOCAL_LLM_URL="http://localhost:8000"

# Gateway listen address (default: 0.0.0.0:8080)
export GATEWAY_LISTEN_ADDR="0.0.0.0:8080"

# Enable/disable features
export ENABLE_RATE_LIMITING=true
export ENABLE_CIRCUIT_BREAKERS=true
```

### Advanced Configuration

```rust
use mofa_gateway::proxy::{LocalLLMBackend, ProxyHandler};
use std::time::Duration;

// Custom backend configuration
let backend = LocalLLMBackend::new("http://localhost:8000")
    .with_health_endpoint("/health")
    .with_timeout(Duration::from_secs(120));

// Create proxy handler
let proxy = ProxyHandler::new(backend.to_proxy_backend());
```

---

## API Endpoints

The gateway exposes OpenAI-compatible endpoints:

### 1. List Models

```http
GET /v1/models
```

**Response:**
```json
{
  "object": "list",
  "data": [
    {
      "id": "qwen2.5-0.5b-instruct",
      "object": "model",
      "created": 1234567890,
      "owned_by": "mofa"
    }
  ]
}
```

### 2. Get Model Info

```http
GET /v1/models/{model_id}
```

**Response:**
```json
{
  "id": "qwen2.5-0.5b-instruct",
  "object": "model",
  "created": 1234567890,
  "owned_by": "mofa"
}
```

### 3. Chat Completions

```http
POST /v1/chat/completions
Content-Type: application/json
```

**Request:**
```json
{
  "model": "qwen2.5-0.5b-instruct",
  "messages": [
    {"role": "system", "content": "You are a helpful assistant."},
    {"role": "user", "content": "Hello!"}
  ],
  "max_tokens": 100,
  "temperature": 0.7
}
```

**Response:**
```json
{
  "id": "chatcmpl-123",
  "object": "chat.completion",
  "created": 1234567890,
  "model": "qwen2.5-0.5b-instruct",
  "choices": [
    {
      "index": 0,
      "message": {
        "role": "assistant",
        "content": "Hello! How can I help you today?"
      },
      "finish_reason": "stop"
    }
  ],
  "usage": {
    "prompt_tokens": 10,
    "completion_tokens": 20,
    "total_tokens": 30
  }
}
```

### 4. Gateway Health Check

```http
GET /health
```

**Response:**
```json
{
  "status": "ok",
  "backends": {
    "mofa-local-llm": "healthy"
  }
}
```

---

## Client Examples

### Python (OpenAI SDK)

```python
from openai import OpenAI

# Point to gateway instead of OpenAI
client = OpenAI(
    base_url="http://localhost:8080/v1",
    api_key="not-needed"  # Gateway doesn't require auth yet
)

# List models
models = client.models.list()
for model in models.data:
    print(f"Model: {model.id}")

# Chat completion
response = client.chat.completions.create(
    model="qwen2.5-0.5b-instruct",
    messages=[
        {"role": "user", "content": "Hello!"}
    ],
    max_tokens=100
)

print(response.choices[0].message.content)
```

### JavaScript/TypeScript

```typescript
import OpenAI from 'openai';

const client = new OpenAI({
  baseURL: 'http://localhost:8080/v1',
  apiKey: 'not-needed'
});

// List models
const models = await client.models.list();
console.log(models.data);

// Chat completion
const completion = await client.chat.completions.create({
  model: 'qwen2.5-0.5b-instruct',
  messages: [{ role: 'user', content: 'Hello!' }],
  max_tokens: 100
});

console.log(completion.choices[0].message.content);
```

### Rust

```rust
use reqwest::Client;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new();
    
    // List models
    let models = client
        .get("http://localhost:8080/v1/models")
        .send()
        .await?
        .json::<serde_json::Value>()
        .await?;
    
    println!("Models: {}", models);
    
    // Chat completion
    let response = client
        .post("http://localhost:8080/v1/chat/completions")
        .json(&json!({
            "model": "qwen2.5-0.5b-instruct",
            "messages": [{"role": "user", "content": "Hello!"}],
            "max_tokens": 100
        }))
        .send()
        .await?
        .json::<serde_json::Value>()
        .await?;
    
    println!("Response: {}", response);
    
    Ok(())
}
```

### cURL

```bash
# List models
curl http://localhost:8080/v1/models | jq

# Get model info
curl http://localhost:8080/v1/models/qwen2.5-0.5b-instruct | jq

# Chat completion
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "qwen2.5-0.5b-instruct",
    "messages": [{"role": "user", "content": "Hello!"}],
    "max_tokens": 100
  }' | jq
```

---

## Features

### Health Checking

The gateway automatically monitors backend health:

```rust
// Health checks run every 30 seconds
// Unhealthy backends are temporarily removed from rotation
// Circuit breakers open after consecutive failures
```

**Check backend health:**
```bash
curl http://localhost:8080/health
```

### Circuit Breakers

Automatic fault tolerance:

- **Failure threshold**: 5 consecutive failures
- **Timeout**: 60 seconds
- **Half-open state**: Gradual recovery testing

When circuit breaker opens:
```json
{
  "error": "Circuit breaker open for backend: mofa-local-llm"
}
```

### Rate Limiting

Protect backend from overload:

```rust
// Configure in GatewayConfig
config.enable_rate_limiting = true;
```

Rate limit exceeded response:
```json
{
  "error": "Rate limit exceeded",
  "retry_after": 60
}
```

### Distributed Tracing

Full request tracing with spans:

```rust
// Tracing automatically enabled
// View logs with RUST_LOG=debug

// Example trace:
// proxy_forward{backend=mofa-local-llm method=POST url=/v1/chat/completions}
//   ├─ forward_request duration=150ms
//   └─ response status=200 body_size=1024
```

---

## Architecture

### Request Flow

```
Client Request
    ↓
Gateway (Port 8080)
    ↓
Health Check → Is backend healthy?
    ↓
Circuit Breaker → Is circuit open?
    ↓
Rate Limiter → Within limits?
    ↓
ProxyHandler → Forward request
    ↓
mofa-local-llm (Port 8000)
    ↓
Response ← Process request
    ↓
Gateway ← Forward response
    ↓
Client ← Return response
```

### Components

1. **ProxyHandler** (`src/proxy/handler.rs`)
   - HTTP forwarding with hyper
   - Request/response transformation
   - Header management

2. **LocalLLMBackend** (`src/proxy/local_llm.rs`)
   - Backend configuration
   - Health check endpoints
   - Timeout settings

3. **Gateway Integration** (`src/gateway/mod.rs`)
   - Route registration
   - State management
   - Middleware integration

4. **Handlers** (`src/handlers/local_llm.rs`)
   - Endpoint implementations
   - Error handling
   - Tracing integration

---

## Testing

### Unit Tests

```bash
# Run proxy unit tests
cargo test --package mofa-gateway --test proxy_tests

# 9 tests covering:
# - ProxyBackend configuration
# - LocalLLMBackend configuration
# - ProxyHandler creation
```

### Integration Tests

```bash
# Run integration tests (requires mofa-local-llm running)
cargo test --package mofa-gateway --test proxy_integration_full

# 8 tests covering:
# - Successful request proxying
# - Backend down scenarios
# - Circuit breaker behavior
# - Concurrent requests
# - Health check integration
```

### Mock Server for Testing

```bash
# Start mock server (no MLX dependencies)
./test_mock_local_llm.sh

# Mock server provides:
# - /health endpoint
# - /v1/models endpoint
# - /v1/models/{id} endpoint
# - /v1/chat/completions endpoint
```

### Performance Benchmarks

```bash
# Run benchmarks
cargo bench --package mofa-gateway --bench proxy_bench

# Benchmarks measure:
# - Proxy overhead (direct vs proxied)
# - Concurrent request handling
# - Different endpoint types
```

---

## Troubleshooting

### Backend Not Reachable

**Error:**
```json
{"error": "Unhealthy node: mofa-local-llm"}
```

**Solution:**
1. Check if mofa-local-llm is running: `curl http://localhost:8000/health`
2. Verify backend URL in config
3. Check network connectivity
4. Review gateway logs: `RUST_LOG=debug cargo run`

### Circuit Breaker Open

**Error:**
```json
{"error": "Circuit breaker open for backend: mofa-local-llm"}
```

**Solution:**
1. Wait for circuit breaker to enter half-open state (60 seconds)
2. Fix backend issues causing failures
3. Restart gateway to reset circuit breaker
4. Check backend health: `curl http://localhost:8000/health`

### 404 Not Found

**Error:**
```
404 Not Found
```

**Solution:**
1. Verify endpoint path: `/v1/models` not `/models`
2. Check route registration in gateway
3. Ensure proxy is enabled: `enable_local_llm_proxy: true`
4. Review gateway startup logs

### Timeout Errors

**Error:**
```json
{"error": "Request timeout"}
```

**Solution:**
1. Increase timeout in backend config:
   ```rust
   backend.with_timeout(Duration::from_secs(120))
   ```
2. Check backend performance
3. Reduce request complexity (max_tokens, etc.)
4. Monitor backend resource usage

### Port Already in Use

**Error:**
```
Address already in use (os error 48)
```

**Solution:**
```bash
# Find and kill process using port 8080
lsof -ti:8080 | xargs kill -9

# Or use a different port
export GATEWAY_LISTEN_ADDR="0.0.0.0:8081"
```

---

## Performance Tips

### 1. Connection Pooling

The gateway uses connection pooling by default:

```rust
// hyper client automatically pools connections
// No additional configuration needed
```

### 2. Timeout Tuning

Adjust timeouts based on your workload:

```rust
// For quick inference models
backend.with_timeout(Duration::from_secs(30));

// For larger models
backend.with_timeout(Duration::from_secs(120));
```

### 3. Concurrent Requests

The gateway handles concurrent requests efficiently:

```bash
# Test concurrent load
ab -n 1000 -c 10 http://localhost:8080/v1/models
```

### 4. Monitoring

Monitor proxy performance:

```bash
# View metrics
curl http://localhost:8080/metrics

# Key metrics:
# - gateway_proxy_requests_total
# - gateway_proxy_request_duration_seconds
# - gateway_proxy_errors_total
```

---

## Next Steps

### Add Authentication

See [Task 15](../../NEXT_TASKS.md#task-15-authentication--authorization) for adding API key authentication.

### Add Caching

See [Task 16](../../NEXT_TASKS.md#task-16-requestresponse-caching) for implementing response caching.

### Multi-Backend Load Balancing

See [Task 17](../../NEXT_TASKS.md#task-17-multi-backend-load-balancing) for supporting multiple backends.

### Monitoring Dashboard

See [Task 18](../../NEXT_TASKS.md#task-18-monitoring--observability-dashboard) for Grafana dashboards.

---

## Additional Resources

- [Gateway Architecture](../../docs/gateway.md)
- [Task 13 Implementation Plan](../../TASK13_PLAN.md)
- [Next Tasks](../../NEXT_TASKS.md)
- [mofa-local-llm Repository](https://github.com/mofa-org/mofa-local-llm)

---

**Questions or Issues?**

- Check the [troubleshooting section](#troubleshooting)
- Review [test files](tests/) for working examples
- See [examples directory](examples/) for practical code

**Status**: Production Ready
