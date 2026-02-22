# mofa-ffi

MoFA FFI (Foreign Function Interface) bindings for cross-language support.

This crate provides language bindings for the MoFA framework using:

- **UniFFI**: Python, Kotlin, Swift, Java bindings
- **PyO3**: Native Python extension module

## Features

- `uniffi` - Enable UniFFI cross-language bindings
- `python` - Enable PyO3 native Python extension
- `all` - Enable all FFI bindings

## Usage

### For Rust Users (No FFI)

If you're using MoFA from Rust, you should use `mofa-sdk` instead:

```toml
[dependencies]
mofa-sdk = "0.1"
```

### For Python Users

```toml
[dependencies]
mofa-ffi = { version = "0.1", features = ["uniffi"] }
```

Generate Python bindings:

```bash
cd crates/mofa-ffi
./generate-bindings.sh python
```

### For Kotlin/Swift/Java Users

```toml
[dependencies]
mofa-ffi = { version = "0.1", features = ["uniffi"] }
```

Generate bindings:

```bash
cd crates/mofa-ffi
./generate-bindings.sh kotlin   # or swift, java
```

## Building

```bash
# Build with UniFFI support
cargo build --features uniffi -p mofa-ffi

# Build with all FFI features
cargo build --features all -p mofa-ffi
```

## Architecture

```
mofa-ffi (Adapter Layer)
    |
    +-- UniFFI bindings (Python, Kotlin, Swift, Java)
    |
    +-- PyO3 bindings (Native Python)
    |
    v
mofa-sdk (Standard API)
    |
    v
mofa-runtime, mofa-foundation, mofa-kernel
```

## License

Apache-2.0
