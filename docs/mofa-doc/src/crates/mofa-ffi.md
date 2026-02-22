# mofa-ffi

Foreign Function Interface bindings for multiple languages.

## Purpose

`mofa-ffi` provides:
- UniFFI bindings for Python, Java, Go, Swift, Kotlin
- PyO3 native Python bindings
- Cross-language type conversions

## Supported Languages

| Language | Method | Status |
|----------|--------|--------|
| Python | UniFFI / PyO3 | Stable |
| Java | UniFFI | Beta |
| Go | UniFFI | Beta |
| Swift | UniFFI | Beta |
| Kotlin | UniFFI | Beta |

## Usage

### Build Bindings

```bash
# Build all bindings
cargo build -p mofa-ffi --features uniffi

# Build Python only
cargo build -p mofa-ffi --features python
```

### Generate Bindings

```bash
# Python
cargo run -p mofa-ffi --features uniffi -- generate python

# Java
cargo run -p mofa-ffi --features uniffi -- generate java
```

## Feature Flags

| Flag | Description |
|------|-------------|
| `uniffi` | Enable UniFFI bindings |
| `python` | Enable PyO3 Python bindings |

## See Also

- [Cross-Language Bindings](../ffi/README.md) — FFI overview
- [Python Bindings](../ffi/python.md) — Python usage
