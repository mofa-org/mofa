# Crates

MoFA workspace crate documentation.

## Overview

- **mofa-kernel** — Microkernel core with trait definitions
- **mofa-foundation** — Concrete implementations and business logic
- **mofa-runtime** — Agent lifecycle and message bus
- **mofa-plugins** — Plugin system (Rhai, WASM, Rust)
- **mofa-sdk** — High-level user-facing API
- **mofa-ffi** — Foreign function interface bindings
- **mofa-cli** — Command-line interface tool
- **mofa-macros** — Procedural macros
- **mofa-monitoring** — Observability and metrics
- **mofa-extra** — Additional utilities

## Architecture

```
mofa-sdk (User API)
    ├── mofa-runtime (Execution)
    ├── mofa-foundation (Implementations)
    ├── mofa-kernel (Traits)
    └── mofa-plugins (Extensions)
```

## Next Steps

Explore individual crate documentation.
