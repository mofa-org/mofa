//! MoFA FFI Bindings - Cross-language bindings for the MoFA framework
//!
//! This crate provides Foreign Function Interface (FFI) bindings for MoFA,
//! enabling the framework to be used from Python, Kotlin, Swift, Java, and other languages.
//!
//! # Architecture
//!
//! ```text
//! mofa-ffi (Adapter Layer)
//!     |
//!     +-- UniFFI bindings (Python, Kotlin, Swift, Java)
//!     |
//!     +-- PyO3 bindings (Native Python)
//!     |
//!     v
//! mofa-sdk (Standard API)
//! ```
//!
//! # Features
//!
//! - `uniffi` - Enable UniFFI cross-language bindings (Python, Kotlin, Swift, Java)
//! - `python` - Enable PyO3 native Python extension
//! - `all` - Enable all FFI bindings
//!
//! # Usage
//!
//! ## For Python Users
//!
//! ```bash
//! # Build with UniFFI support
//! cargo build --release --features uniffi -p mofa-ffi
//!
//! # Generate Python bindings
//! cd crates/mofa-ffi && ./generate-bindings.sh python
//! ```
//!
//! ## For Kotlin/Swift/Java Users
//!
//! ```bash
//! cargo build --release --features uniffi -p mofa-ffi
//! cd crates/mofa-ffi && ./generate-bindings.sh kotlin  # or swift, java
//! ```

// Re-export everything from mofa-ai for convenience
pub use mofa_ai::*;

// =============================================================================
// UniFFI bindings (enabled with `uniffi` feature)
// =============================================================================

#[cfg(feature = "uniffi")]
mod uniffi_bindings;

#[cfg(feature = "uniffi")]
pub use uniffi_bindings::*;

// Include generated UniFFI scaffolding
#[cfg(feature = "uniffi")]
uniffi::include_scaffolding!("mofa");

// =============================================================================
// PyO3 Python bindings (enabled with `python` feature)
// =============================================================================

#[cfg(feature = "python")]
mod python_bindings;

#[cfg(feature = "python")]
pub use python_bindings::mofa;
