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
└─────────────────────────────────────────────────────────────┘
```

## Use Cases

- **Edge Deployment**: Run models locally on edge devices
- **Cost Optimization**: Minimize cloud API costs
- **Latency Sensitive**: Prioritize fast response times
- **Hybrid Mesh**: Combine local and cloud for best experience

## License

This demo is part of the MoFA project and follows the same license terms.
