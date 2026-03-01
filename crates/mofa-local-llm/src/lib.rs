//! # mofa-local-llm
//!
//! Linux inference backend for MoFA with automatic hardware detection.
//!
//! Selects the best available compute backend in priority order:
//! **CUDA** (NVIDIA) → **ROCm** (AMD) → **Vulkan** (cross-vendor) → **CPU**
//!
//! ## Features
//!
//! | Feature  | Description                          |
//! |----------|--------------------------------------|
//! | `cuda`   | Enable CUDA backend (NVIDIA GPUs)    |
//! | `rocm`   | Enable ROCm backend (AMD GPUs)       |
//! | `vulkan` | Enable Vulkan compute backend        |
//!
//! No features are enabled by default — the CPU fallback is always available.
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use mofa_local_llm::{LinuxLocalProvider, LinuxInferenceConfig};
//! use mofa_foundation::orchestrator::traits::ModelProvider;
//!
//! #[tokio::main]
//! async fn main() {
//!     let config = LinuxInferenceConfig::new("llama-7b", "/models/llama-7b.gguf");
//!     let provider = LinuxLocalProvider::new(config).unwrap();
//!     println!("backend: {}", provider.get_metadata()["backend"]);
//! }
//! ```

pub mod config;
pub mod hardware;
pub mod provider;

pub use config::LinuxInferenceConfig;
pub use hardware::{ComputeBackend, HardwareInfo};
pub use provider::LinuxLocalProvider;
