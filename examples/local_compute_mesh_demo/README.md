# Local Compute Mesh Demo

This demo showcases the MoFA compute mesh routing capabilities, demonstrating how inference requests can be routed between local and cloud backends based on configurable policies.

## Overview

The compute mesh enables intelligent routing of inference requests between:

- **Local backends**: Run inference on local hardware (CPUs/GPUs)
- **Cloud backends**: Route to cloud providers (OpenAI, Anthropic, etc.)

## Features

- **Multiple Routing Policies**: LocalFirstWithCloudFallback, LocalOnly, CloudOnly, LatencyOptimized, CostOptimized
- **Memory-Aware Scheduling**: Automatic memory management and model eviction
- **Performance Benchmarking**: Built-in metrics collection for latency and throughput
- **Execution Trace Visualization**: Full pipeline observability with trace events and JSON export

## Running the Demo

### Basic Usage

```bash
cargo run --example local_compute_mesh_demo -- "Explain photosynthesis"
```

### With Custom Prompt

```bash
cargo run --example local_compute_mesh_demo -- "What is quantum computing?"
```

## Performance Benchmark

The demo includes comprehensive performance benchmarking that measures:

### Metrics Collected

| Metric | Description |
|--------|-------------|
| `latency_ms` | Total time from request start to completion |
| `time_to_first_token_ms` | Time to receive the first token |
| `tokens_streamed` | Total number of tokens generated |
| `tokens_per_second` | Token generation throughput |
| `total_time_ms` | Total streaming duration |

### How Latency is Measured

1. **Request Start**: Timer begins when the inference request is created
2. **First Token**: Records the time when the first token is received/streamed
3. **Completion**: Timer ends when all tokens have been streamed

The latency is measured using `std::time::Instant` for high-resolution timing.

### How Tokens/Second is Calculated

```
tokens_per_second = tokens_streamed / (total_time_ms / 1000.0)
```

This gives you the actual streaming throughput during generation.

### Comparing Local vs Cloud Routing

The demo runs three scenarios to compare different routing policies:

1. **LocalFirstWithCloudFallback**: Tries local first, falls back to cloud if needed
2. **CloudOnly**: Always routes to cloud provider
3. **LocalOnly**: Always uses local backend

Example output:

```
[workflow] executing step: generate_response
[inference] sending request to orchestrator...

[router] policy: LocalFirstWithCloudFallback
[router] selected backend: local

[stream] This
[stream] is
[stream] a
...

[metrics]
backend: local
latency_ms: 820
time_to_first_token_ms: 45
tokens_streamed: 27
tokens_per_second: 32.9
total_time_ms: 910
```

## Execution Trace Visualization

The demo includes execution trace visualization to improve observability of how requests flow through the compute mesh pipeline.

### Trace Events

The pipeline records the following trace events:

| Event | Description |
|-------|-------------|
| `workflow.start` | Workflow execution begins |
| `router.policy` | Routing policy decision |
| `router.backend_selection` | Selected backend (local/cloud) |
| `inference.start` | Inference request initiated |
| `streaming.tokens` | Token streaming progress |
| `metrics.latency_ms` | Latency measurement |
| `workflow.complete` | Workflow execution completed |

### Trace Output

Traces are printed to console and can be exported as JSON:

```json
{
  "request_id": "uuid-here",
  "stages": [
    {"stage": "workflow.start", "timestamp_ms": 1700000000000},
    {"stage": "router.policy", "detail": "LocalFirstWithCloudFallback", "timestamp_ms": 1700000000005},
    {"stage": "router.backend_selection", "detail": "local", "timestamp_ms": 1700000000010},
    {"stage": "inference.start", "timestamp_ms": 1700000000015},
    {"stage": "streaming.tokens", "detail": "token_1", "timestamp_ms": 1700000000020},
    {"stage": "metrics.latency_ms", "detail": "350", "timestamp_ms": 1700000000350},
    {"stage": "workflow.complete", "timestamp_ms": 1700000000355}
  ]
}
```

### Pipeline Visualization

```
┌─────────────────────────────────────────────────────────────┐
│              Compute Mesh Pipeline Flow                      │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  User Input ──▶ [Workflow Start] ──▶ [Router Policy]       │
│                                              │              │
│                                              ▼              │
│                                    [Backend Selection]      │
│                                              │              │
│                                              ▼              │
│                                   [Inference Engine]        │
│                                              │              │
│                                              ▼              │
│                                 [Token Streaming] ──▶ Trace  │
│                                              │              │
│                                              ▼              │
│                                    [Metrics Collection]     │
│                                              │              │
│                                              ▼              │
│                                 [Workflow Complete]          │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

## Configuration

The demo can be configured via the `workflow.yaml` file:

- **routing_policy**: Choose the routing strategy
- **memory_capacity_mb**: Set local model memory limit
- **cloud_provider**: Configure cloud provider fallback

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                   Compute Mesh Demo                          │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────────────────────────────────────────────┐   │
│  │           InferenceOrchestrator                      │   │
│  │                                                      │   │
│  │  ┌─────────────┐    ┌─────────────┐                │   │
│  │  │   Routing   │    │   Model     │                │   │
│  │  │   Policy    │    │   Pool      │                │   │
│  │  └─────────────┘    └─────────────┘                │   │
│  │         │                  │                         │   │
│  │         ▼                  ▼                         │   │
│  │  ┌─────────────┐    ┌─────────────┐                │   │
│  │  │    Local    │    │    Cloud    │                │   │
│  │  │  Execution  │    │   Fallback  │                │   │
│  │  └─────────────┘    └─────────────┘                │   │
│  └─────────────────────────────────────────────────────┘   │
│                        │                                      │
│                        ▼                                      │
│              ┌──────────────────┐                          │
│              │  Observability    │                          │
│              │  - Metrics        │                          │
│              │  - Tracing        │                          │
│              │  - Benchmarking   │                          │
│              └──────────────────┘                          │
└─────────────────────────────────────────────────────────────┘
```

## Use Cases

- **Edge Deployment**: Run models locally on edge devices
- **Cost Optimization**: Minimize cloud API costs
- **Latency Sensitive**: Prioritize fast response times
- **Hybrid Mesh**: Combine local and cloud for best experience
- **Observability**: Debug and trace request flow with execution traces

## License

This demo is part of the MoFA project and follows the same license terms.
