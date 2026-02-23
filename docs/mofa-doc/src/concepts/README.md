# Core Concepts

Understanding MoFA's architecture and key abstractions.

## Overview

- **Architecture Overview** — High-level system design
- **Microkernel Design** — Core kernel and plugin layers
- **Agents** — Agent traits and lifecycle
- **Tools** — Tool system and function calling
- **Plugins** — Compile-time and runtime plugins
- **Workflows** — Multi-agent coordination patterns

## Architecture Layers

```
┌─────────────────────────────────────────┐
│              mofa-sdk                   │
├─────────────────────────────────────────┤
│            mofa-runtime                 │
├─────────────────────────────────────────┤
│          mofa-foundation                │
├─────────────────────────────────────────┤
│            mofa-kernel                  │
├─────────────────────────────────────────┤
│           mofa-plugins                  │
└─────────────────────────────────────────┘
```

## Next Steps

Start with [Architecture Overview](architecture.md) to understand the big picture.
