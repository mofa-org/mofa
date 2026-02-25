## 📋 Summary

Implements a Linux-native inference backend for mofa-local-llm with automatic hardware detection and support for CUDA, ROCm, and Vulkan compute backends.

## 🔗 Related Issues

Closes #294

## 🧠 Context

Currently mofa-local-llm lacks a dedicated inference backend optimized for Linux environments. This implementation provides:

- **Automatic hardware detection**: Detects NVIDIA GPUs (via nvidia-smi), AMD GPUs (via rocm-smi), and Vulkan-capable GPUs
- **Backend selection**: Automatically selects the best available backend (CUDA > ROCm > Vulkan > CPU)
- **Multiple model format support**: GGUF, GGML, ONNX, Safe Tensors, PyTorch

## 🛠️ Changes

- Added `linux_backend.rs` module with hardware detection, compute backend selection, and model loading
- Added configuration options for manual backend override, memory limits, thread pinning
- Added memory pressure handler for resource management
- Added comprehensive unit tests

## 🧪 How you Tested

1. `cargo check -p mofa-foundation` - Code compiles successfully
2. `cargo test -p mofa-foundation -- linux_backend` - Run backend tests
3. Manual testing on Linux with CUDA/ROCm/Vulkan GPUs

## ⚠️ Breaking Changes

- [x] No breaking changes

## 🧹 Checklist

### Code Quality
- [x] Code follows Rust idioms and project conventions
- [x] `cargo fmt` run
- [x] `cargo clippy` passes without warnings (dev profile)

### Testing
- [x] Tests added/updated
- [x] `cargo test` passes locally

### Documentation
- [x] Public APIs documented with doc comments

### PR Hygiene
- [x] PR is small and focused (one logical change)
- [x] Branch is up to date with `main`
- [x] No unrelated commits

## 🚀 Deployment Notes

No deployment changes required. This is a new feature module.

## 🧩 Additional Notes for Reviewers

This is a foundation implementation. The actual llama.cpp/ONNX Runtime bindings would be added in follow-up PRs to keep this PR focused on the infrastructure layer.
