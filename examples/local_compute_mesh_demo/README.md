# Local Compute Mesh Demo with Execution Trace Visualization

This demo showcases the compute mesh pipeline with execution trace visualization to improve observability of how requests flow through the system.

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        COMPUTE MESH ARCHITECTURE                            │
└─────────────────────────────────────────────────────────────────────────────┘

User Prompt
    │
    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                         Workflow Engine                                     │
│  - Orchestrates the execution flow                                         │
│  - Manages request lifecycle                                               │
│  - Records workflow.start / workflow.complete events                       │
└─────────────────────────────────────────────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                         Inference Router                                    │
│  - Evaluates routing policies                                               │
│  - Selects appropriate backend (Local/Cloud)                               │
│  - Records router.policy / router.backend_selection events                │
└─────────────────────────────────────────────────────────────────────────────┘
    │
    ├──► ┌─────────────────────────────────────────────────────────────────┐   │
│  Local  │                    Local Backend                              │   │
│  Path    │  - On-device inference                                      │   │
│          │  - Lower latency                                            │   │
│          │  - Privacy-focused                                          │   │
└─────────►└─────────────────────────────────────────────────────────────────┘   │
    │
    └──► ┌─────────────────────────────────────────────────────────────────┐   │
│  Cloud  │                    Cloud Backend                              │   │
│  Path    │  - Remote API inference                                     │   │
│          │  - Higher capacity                                          │   │
│          │  - More powerful models                                      │   │
└─────────►└─────────────────────────────────────────────────────────────────┘   │
    │
    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                       Streaming Tokens                                     │
│  - Real-time token-by-token response delivery                             │
│  - Records streaming.tokens events for each token                         │
│  - Provides immediate feedback to users                                   │
└─────────────────────────────────────────────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                      Performance Metrics                                    │
│  - Latency measurement (latency_ms)                                      │
│  - Token count tracking                                                   │
│  - Records metrics.latency_ms event                                      │
└─────────────────────────────────────────────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                       Execution Trace                                       │
│  - Complete timeline of all pipeline stages                               │
│  - JSON export for analysis                                              │
│  - Debugging and observability                                            │
└─────────────────────────────────────────────────────────────────────────────┘
    │
    ▼
                        Final Response
```

## Execution Lifecycle

The demo records trace events at each stage of the pipeline:

| Stage | Component | Description | Log Event |
|-------|-----------|-------------|-----------|
| 1 | Workflow Engine | Initializes request processing | `workflow.start` |
| 2 | Inference Router | Evaluates routing policy | `router.policy` |
| 3 | Inference Router | Selects backend | `router.backend_selection` |
| 4 | Inference Engine | Starts inference | `inference.start` |
| 5 | Token Stream | Streams response tokens | `streaming.tokens` |
| 6 | Metrics | Collects performance data | `metrics.latency_ms` |
| 7 | Workflow Engine | Completes request | `workflow.complete` |

### Stage Details

#### 1. workflow.start
- **Component**: Workflow Engine
- **What it does**: Initializes the request, creates execution context
- **Log output**: `[trace] workflow.start`

#### 2. router.policy
- **Component**: Inference Router
- **What it does**: Evaluates the configured routing policy
- **Log output**: `[trace] router.policy = LocalFirstWithCloudFallback`

#### 3. router.backend_selection
- **Component**: Inference Router
- **What it does**: Selects the backend based on policy evaluation
- **Log output**: `[trace] router.backend_selection = local`

#### 4. inference.start
- **Component**: Inference Engine
- **What it does**: Begins the inference process with selected backend
- **Log output**: `[trace] inference.start`

#### 5. streaming.tokens
- **Component**: Token Stream
- **What it does**: Streams each token of the response in real-time
- **Log output**: `[trace] streaming.tokens = token_1`, `token_2`, etc.

#### 6. metrics.latency_ms
- **Component**: Metrics Collector
- **What it does**: Records end-to-end latency
- **Log output**: `[trace] metrics.latency_ms = 820`

#### 7. workflow.complete
- **Component**: Workflow Engine
- **What it does**: Finalizes the request, outputs final response
- **Log output**: `[trace] workflow.complete`

## Routing Policies

The demo supports three routing policies:

### LocalFirstWithCloudFallback
- **Behavior**: Attempts local inference first
- **Fallback**: If local fails, switches to cloud backend
- **Use case**: Balance of speed and reliability

### LocalOnly
- **Behavior**: Uses only local inference backend
- **Use case**: Offline mode, privacy-sensitive applications

### CloudOnly
- **Behavior**: Uses only cloud inference backend
- **Use case**: Maximum model capacity, always-online scenarios

### Backend Selection Logic

```
┌─────────────────────────────────────────────────────────────────┐
│                    Backend Selection Flow                        │
└─────────────────────────────────────────────────────────────────┘

                    ┌──────────────────┐
                    │  Evaluate Policy │
                    └────────┬─────────┘
                             │
              ┌──────────────┼──────────────┐
              │              │              │
              ▼              ▼              ▼
       ┌────────────┐ ┌────────────┐ ┌────────────┐
       │LocalOnly   │ │CloudOnly   │ │LocalFirst  │
       └─────┬──────┘ └─────┬──────┘ └─────┬──────┘
             │              │              │
             ▼              ▼              ▼
       ┌────────────┐ ┌────────────┐ ┌────────────┐
       │   Local    │ │   Cloud    │ │   Try Local│
       │   Backend  │ │   Backend  │ │   First    │
       └────────────┘ └────────────┘ └─────┬──────┘
                                           │
                                    ┌──────┴──────┐
                                    │             │
                                    ▼             ▼
                               Success       Failed
                                    │             │
                                    ▼             ▼
                               Response    ┌───────────┐
                                           │   Cloud   │
                                           │   Backend │
                                           └───────────┘
```

## Execution Walkthrough

Here's a step-by-step example of running the demo:

```bash
# Run the demo with a custom prompt
cargo run -p local_compute_mesh_demo -- "Explain quantum computing"
```

### Expected Output:

```
=== Local Compute Mesh Demo with Execution Trace ===

User prompt: Explain quantum computing

[workflow] executing step: generate_response
[router] policy: LocalFirstWithCloudFallback
[router] selected backend: local
[inference] sending request to orchestrator...
[stream] This
[stream] is
[stream] a
[stream] simulated
[stream] response
[stream] from
[stream] the
[stream] compute
[stream] mesh
[metrics] latency_ms = 820

==== Compute Mesh Execution Trace ====

[trace] workflow.start
[trace] router.policy = LocalFirstWithCloudFallback
[trace] router.backend_selection = local
[trace] inference.start
[trace] streaming.tokens = token_1
[trace] streaming.tokens = token_2
[trace] streaming.tokens = token_3
[trace] streaming.tokens = token_4
[trace] streaming.tokens = token_5
[trace] streaming.tokens = token_6
[trace] streaming.tokens = token_7
[trace] streaming.tokens = token_8
[trace] streaming.tokens = token_9
[trace] metrics.latency_ms = 820
[trace] workflow.complete

--- JSON Trace Export ---

{
  "request_id": "550e8400-e29b-41d4-a716-446655440000",
  "stages": [
    {"stage": "workflow.start", "detail": null, "timestamp_ms": 1699999999000},
    {"stage": "router.policy", "detail": "LocalFirstWithCloudFallback", "timestamp_ms": 1699999999010},
    {"stage": "router.backend_selection", "detail": "local", "timestamp_ms": 1699999999015},
    {"stage": "inference.start", "detail": null, "timestamp_ms": 1699999999020},
    {"stage": "streaming.tokens", "detail": "token_1", "timestamp_ms": 1699999999050},
    {"stage": "streaming.tokens", "detail": "token_2", "timestamp_ms": 1699999999100},
    ...
    {"stage": "metrics.latency_ms", "detail": "820", "timestamp_ms": 1699999999820},
    {"stage": "workflow.complete", "detail": null, "timestamp_ms": 1699999999820}
  ]
}

Result: Processed 'Explain quantum computing' with 9 tokens (latency: 820ms)
```

## ASCII Pipeline Visualization

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         USER PROMPT                                         │
│                    "Explain quantum computing"                              │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                     ┌─────────────────────┐                                 │
│                     │   Workflow Engine   │                                 │
│                     │  workflow.start ──► │                                 │
│                     └─────────────────────┘                                 │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                     ┌─────────────────────┐                                 │
│                     │  Inference Router   │                                 │
│                     │  policy=LocalFirst  │                                 │
│                     │  backend=local      │                                 │
│                     └─────────────────────┘                                 │
└─────────────────────────────────────────────────────────────────────────────┘
                    │                       │
           ┌────────┴────────┐      ┌────────┴────────┐
           │   Local Backend │      │  Cloud Backend  │
           │  (selected ✓)   │      │    (fallback)   │
           └─────────────────┘      └─────────────────┘
                    │
                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                     ┌─────────────────────┐                                 │
│                     │  Streaming Tokens   │                                 │
│                     │  token_1 → token_9  │                                 │
│                     └─────────────────────┘                                 │
└─────────────────────────────────────────────────────────────────────────────┘
                    │
                    ▼
┌────────────────┐     ┌─────────────────────┐
│   Metrics      │     │   Execution Trace   │
│ latency=820ms │     │   Complete Timeline  │
└────────────────┘     └─────────────────────┘
                                    │
                                    ▼
                        ┌─────────────────────┐
                        │   Final Response     │
                        │  "Processed 'Explain │
                        │  quantum computing'  │
                        │  with 9 tokens"      │
                        └─────────────────────┘
```

## Running the Demo

### Prerequisites

- Rust 1.75+
- Cargo

### Build and Run

```bash
# Navigate to examples directory
cd examples

# Run with default prompt
cargo run -p local_compute_mesh_demo

# Run with custom prompt
cargo run -p local_compute_mesh_demo -- "Your prompt here"

# Run with specific routing policy (modify source code)
```

### Features Demonstrated

1. **Pipeline Orchestration**: Complete request flow from user input to final response
2. **Routing Policies**: Flexible backend selection strategies
3. **Token Streaming**: Real-time response streaming
4. **Metrics Collection**: Performance tracking
5. **Execution Tracing**: Full observability of pipeline stages
6. **JSON Export**: Machine-readable trace output for analysis

## Architecture Benefits

| Benefit | Description |
|---------|-------------|
| **Observability** | Complete visibility into each pipeline stage |
| **Flexibility** | Multiple routing policies for different use cases |
| **Performance** | Local inference for low latency |
| **Reliability** | Cloud fallback for fault tolerance |
| **Debugging** | JSON trace export for troubleshooting |
