## 📋 Summary

Implements **ModelPool** — a runtime component for managing local model lifecycle with LRU caching, on-demand loading, and idle timeout-based unloading. Addresses GSoC 2026 Issue #358 for the Unified Inference Orchestrator.

## 🔗 Related Issues

Closes #358

---

## 🧠 Context

Without a model pool layer, MoFA risks hardcoded backend routing, fragile model loading across formats, and poor resource management. This PR introduces a reusable ModelPool that:

- Manages multiple model backends with automatic lifecycle handling
- Uses LRU eviction when capacity is reached
- Supports idle timeout-based automatic unloading
- Provides graceful shutdown with state preservation

This is foundational infrastructure that plugs into the existing backend abstraction and hardware discovery services.

---

## 🛠️ Changes

- Added `ModelPool` module to `mofa-foundation` crate
- Implemented `ModelBackend` trait for pluggable model backends
- Added `ModelPoolConfig` for configurable pool behavior (max models, idle timeout, memory limits)
- Implemented LRU-based cache with async operations using `tokio::sync::RwLock`
- Added background task for periodic idle model eviction
- Added Apple Silicon memory pressure monitoring placeholder
- Added unit tests for basic operations, LRU eviction, and model retrieval

---

## 🧪 How you Tested

```bash
# Run tests
cargo test -p mofa-foundation model_pool

# Build to verify compilation
cargo build -p mofa-foundation
```

All 3 tests pass:
- `test_model_pool_basic` — Load/unload operations
- `test_model_pool_lru_eviction` — LRU eviction when capacity exceeded  
- `test_model_pool_get_model` — Model retrieval from pool

---

## ⚠️ Breaking Changes

- [x] No breaking changes

---

## 🧹 Checklist

### Code Quality
- [x] Code follows Rust idioms and project conventions
- [x] `cargo fmt` run
- [x] `cargo clippy` passes without warnings

### Testing
- [x] Tests added/updated
- [x] `cargo test` passes locally without any error

### Documentation
- [x] Public APIs documented (inline docs in code)
- [ ] README / docs updated (if needed)

### PR Hygiene
- [x] PR is small and focused (one logical change)
- [x] Branch is up to date with `main`
- [x] No unrelated commits
- [x] Commit messages explain **why**, not only **what**

---

## 🚀 Deployment Notes

No deployment changes required. This is a new module that can be integrated into the inference pipeline.

---

## 🧩 Additional Notes for Reviewers

- Uses `tokio::sync::RwLock` for thread-safe async operations
- `MockModelBackend` provided for testing; real backends would implement the `ModelBackend` trait
- Memory pressure monitoring is a placeholder — actual Apple Silicon implementation would use system APIs
- The pool can be extended with weighted scoring for candidate adapters in Phase 2 (as per issue)
