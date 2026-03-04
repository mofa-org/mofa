# WASM Plugins

High-performance plugins using WebAssembly.

## Overview

WASM plugins provide:
- Cross-language compatibility
- Sandboxed execution
- Near-native performance

## Creating a WASM Plugin

### Setup

```toml
# Cargo.toml
[lib]
crate-type = ["cdylib"]

[dependencies]
wasm-bindgen = "0.2"
```

### Implementation

```rust
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn process(input: &str) -> String {
    // Your implementation
    format!("Processed: {}", input)
}

#[wasm_bindgen]
pub fn analyze(data: &[u8]) -> Vec<u8> {
    // Binary data processing
    data.to_vec()
}
```

### Build

```bash
cargo build --target wasm32-unknown-unknown --release
```

## Loading WASM Plugins

```rust
use mofa_plugins::WasmPlugin;

let plugin = WasmPlugin::load("./plugins/my_plugin.wasm").await?;

// Call exported function
let result = plugin.call("process", b"input data").await?;
println!("Result: {}", String::from_utf8_lossy(&result));
```

## Security

WASM plugins run in a sandboxed environment:
- No direct file system access
- No network access (unless explicitly granted)
- Memory isolation

## See Also

- [Plugins Concept](../../concepts/plugins.md) — Plugin architecture
