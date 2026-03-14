# MoFA Local Compute Mesh Demo

This example provides a scaffold for the MoFA Local Compute Mesh Demo, demonstrating the architecture for local inference pipelines.

## Overview

The Compute Mesh is a distributed inference architecture that enables efficient local model execution. This demo showcases the pipeline from user input to generated response, with components for workflow processing, intelligent routing, and local backend execution.

## What is Compute Mesh?

Compute Mesh is MoFA's approach to distributed local inference, featuring:

- **Workflow Engine**: Processes user prompts through configurable workflows
- **Inference Router**: Intelligently routes requests based on prompt characteristics, available resources, and policies
- **Local Backend**: Executes inference using local models (Ollama, candle, etc.)
- **Streaming Support**: Enables real-time response streaming for better UX

## Architecture

```
┌─────────────┐    ┌─────────────┐    ┌────────────────┐    ┌──────────────────┐    ┌───────────┐
│ User Prompt │───▶│    Workflow │───▶│ Inference      │───▶│ Local Inference │───▶│ Response  │
│             │    │   Engine    │    │    Router      │    │    Backend       │    │           │
└─────────────┘    └─────────────┘    └────────────────┘    └──────────────────┘    └───────────┘
```

### Pipeline Stages

1. **User Prompt**: Input from the user
2. **Workflow Engine**: Processes and validates the prompt through defined workflow steps
3. **Inference Router**: Determines routing policy (local vs cloud) based on various factors
4. **Local Inference Backend**: Executes inference using local models
5. **Response**: Generated text response returned to the user

## Current Status

This is **Phase 1 (Scaffold)** - a foundational skeleton demonstrating the pipeline architecture.

### What Works

- ✅ Loads and parses workflow.yaml configuration
- ✅ Displays pipeline visualization
- ✅ Simulates each pipeline stage with logging
- ✅ Provides clear architecture documentation

### What's Missing (Future Issues)

- ❌ Full workflow execution integration
- ❌ Actual inference routing logic
- ❌ Local backend implementation
- ❌ Streaming response support
- ❌ Performance benchmarking

## How to Run

```bash
# Run the demo
cargo run --example local_compute_mesh_demo
```

## Expected Output

```
INFO Starting MoFA local compute mesh demo...

========================================
  MoFA Local Compute Mesh Demo         
  (Scaffold - No Inference Yet)         
========================================

Pipeline Architecture:

    ┌─────────────┐    ┌─────────────┐    ┌────────────────┐    ┌──────────────────┐    ┌───────────┐
    │ User Prompt │───▶│    Workflow │───▶│ Inference      │───▶│ Local Inference  │───▶│ Response  │
    │             │    │   Engine    │    │    Router      │    │    Backend       │    │           │
    └─────────────┘    └─────────────┘    └────────────────┘    └──────────────────┘    └───────────┘

  (1)              (2)                 (3)                  (4)                    (5)

INFO Loading workflow definition from workflow.yaml...
INFO Loaded workflow: local_compute_mesh_demo
...

========================================
  Demo scaffold completed successfully!
========================================
```

## Files

- [`Cargo.toml`](Cargo.toml) - Package configuration
- [`src/main.rs`](src/main.rs) - Demo implementation (scaffold)
- [`workflow.yaml`](workflow.yaml) - Workflow definition (placeholder)
- [`README.md`](README.md) - This file

## Future Work

This demo will be extended in future issues to implement:

| Feature | Description | Issue |
|---------|-------------|-------|
| Workflow Integration | Full workflow DSL parsing and execution | TBD |
| Routing Policy Demo | Intelligent routing based on prompt characteristics | TBD |
| Local Backend | Integration with Ollama and other local backends | TBD |
| Streaming | Real-time streaming response support | TBD |
| Benchmarking | Performance metrics and benchmarking tools | TBD |

## Contributing

This scaffold is designed to be extended by future PRs. When adding new functionality:

1. Follow the existing architecture pattern
2. Add clear logging at each pipeline stage
3. Include documentation for new components
4. Maintain backwards compatibility

## Related Issues

- [Issue #952](https://github.com/mofa-org/mofa/issues/952) - Compute Mesh Demo Pipeline
- [Issue #955](https://github.com/mofa-org/mofa/issues/955) - Create Local Compute Mesh Demo Scaffold
