# Local Compute Mesh Demo

This example demonstrates the **LocalFirstWithCloudFallback** routing behavior in the MoFA Compute Mesh architecture.

## Overview

The MoFA Compute Mesh architecture supports intelligent routing between local and cloud inference backends. The `LocalFirstWithCloudFallback` policy is the recommended default for most deployments, as it:

- Prioritizes local inference for privacy, cost savings, and low latency
- Automatically falls back to cloud providers when local resources are unavailable

## Routing Policy Demonstration

This demo illustrates two execution scenarios:

### Scenario A: Local Backend Available

When Ollama is running and the local model is accessible:

```bash
# Start Ollama server
ollama serve

# Pull a model (if not already available)
ollama pull llama3

# Run the demo
cargo run --example local_compute_mesh_demo -- "Explain photosynthesis"
```

**Expected behavior:**
- Router detects Ollama server is available
- Router selects local backend
- Log output shows:
  ```
  [router] policy: LocalFirstWithCloudFallback
  [router] attempting local backend
  [router] selected backend: local
  ```

### Scenario B: Local Backend Unavailable

When Ollama is not running or the local backend is unavailable:

```bash
# Stop Ollama (if running)
# On macOS: pkill -f ollama
# On Linux: sudo systemctl stop ollama

# Run the demo
cargo run --example local_compute_mesh_demo -- "Explain photosynthesis"
```

**Expected behavior:**
- Router detects local backend is unavailable
- Router falls back to cloud provider
- Log output shows:
  ```
  [router] policy: LocalFirstWithCloudFallback
  [router] local backend unavailable
  [router] falling back to cloud provider: openai
  ```

## Architecture

The demo uses the following MoFA components:

- **Inference Orchestrator**: Central control plane for routing decisions
- **Routing Policy Engine**: Implements LocalFirstWithCloudFallback logic
- **Hardware Detection**: Determines available compute resources
- **Memory Scheduler**: Manages admission control for local inference

## Customization

### Changing the Cloud Provider

Modify the `cloud_provider` field in the `OrchestratorConfig`:

```rust
let config = OrchestratorConfig {
    routing_policy: RoutingPolicy::LocalFirstWithCloudFallback,
    cloud_provider: "anthropic".to_string(), // or "google", "azure", etc.
    ..
};
```

### Trying Different Routing Policies

The demo supports multiple routing policies:

- `LocalOnly`: Only use local backends
- `CloudOnly`: Only use cloud providers
- `LocalFirstWithCloudFallback`: Try local first, fall back to cloud
- `LatencyOptimized`: Prefer the fastest backend
- `CostOptimized`: Prefer the cheapest option

## Requirements

- Rust 1.75+
- Cargo
- (Optional) Ollama for Scenario A

## Dependencies

The demo depends on:
- `mofa-foundation`: Core MoFA framework
- `reqwest`: HTTP client for Ollama health checks
- `tracing`: Structured logging
