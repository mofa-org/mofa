//! UniFFI Bindgen CLI
//!
//! This binary provides the `uniffi-bindgen` command for generating language bindings.
//!
//! # Usage
//!
//! ```bash
//! # Generate Python bindings
//! cargo run --features uniffi --bin uniffi-bindgen generate \
//!     --library target/release/libaimos.dylib \
//!     --language python \
//!     --out-dir bindings/python
//!
//! # Generate Kotlin bindings
//! cargo run --features uniffi --bin uniffi-bindgen generate \
//!     --library target/release/libaimos.dylib \
//!     --language kotlin \
//!     --out-dir bindings/kotlin
//! ```

fn main() {
    uniffi::uniffi_bindgen_main()
}
