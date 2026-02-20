# mofa-plugins

MoFA Plugins - Dual-layer plugin system with compile-time (Rust/WASM) and runtime (Rhai) support

## Installation

```toml
[dependencies]
mofa-plugins = "0.1"
```

## Features

- Compile-time plugin layer (Rust/WASM) for performance-critical paths
- Runtime plugin layer (Rhai scripts) for dynamic business logic
- Hot-reloadable plugin system
- Plugin adapters and tool integration
- TTS support (rodio, kokoro-tts)
- WASM runtime via Wasmtime
- Script execution engine

## Documentation

- [API Documentation](https://docs.rs/mofa-plugins)
- [Main Repository](https://github.com/mofa-org/mofa)

## License

Licensed under either of Apache License, Version 2.0 or MIT license at your option.
