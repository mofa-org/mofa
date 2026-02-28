## 📋 Summary

Implements Phase 1 of the runtime model-adapter registry with capability/format negotiation as described in issue #333. This feature adds the missing runtime glue between the backend abstraction, model orchestration, and hardware discovery layers, enabling pluggable inference backends to be discovered, loaded, and managed dynamically at runtime.

## 🔗 Related Issues

- Closes #333

## 🧠 Context

Without an adapter registry + negotiation layer, MoFA risks:
- Hardcoded backend routing
- Fragile model loading across formats/modalities  
- Poor extensibility for new model families
- Inconsistent behavior across hardware backends

This implementation provides a clean abstraction for runtime adapter discovery, addressing the key unresolved questions from `ideas/mofa-agents/call-for-proposal.md`:
- "How do model adapters get discovered, loaded, and managed at runtime?"
- "How does MoFA handle model format differences (safetensors, GGUF, checkpoints)?"

## 🛠️ Changes

- **New module**: `crates/mofa-foundation/src/adapter/` with 6 new files
- **AdapterDescriptor**: Describes adapter capabilities (modalities, formats, quantization, hardware)
- **AdapterRegistry**: Runtime registration, discovery, and deterministic resolution
- **ModelConfig & HardwareProfile**: Configuration types for model selection
- **Error types**: Structured rejection reasons with severity levels (Hard/Soft)
- **Resolver**: Phase 2-ready weighted scoring with EWMA runtime stats

## 🧪 How you Tested

1. **Unit tests pass**: `cargo test -p mofa-foundation`
2. **Adapter resolution tested**:
   - No-compatible-adapter case returns proper error
   - Multi-candidate deterministic selection (priority → alphabetical)
   - Format mismatch rejection
   - Modality mismatch rejection
3. **Code quality**: `cargo fmt` and `cargo clippy` pass

```bash
# Run tests
cargo test -p mofa-foundation -- adapter

# Check formatting  
cargo fmt --all

# Lint
cargo clippy -p mofa-foundation -- -D warnings
```

## ⚠️ Breaking Changes

- [x] No breaking changes

## 🧹 Checklist

- [x] Code follows Rust idioms and project conventions
- [x] `cargo fmt` run
- [x] `cargo clippy` passes without warnings
- [x] Tests added/updated (included in each module)
- [x] Public APIs documented with doc comments
- [x] PR is small and focused (one logical change)
- [x] Branch is up to date with `main`

## 🚀 Deployment Notes

No special deployment requirements. This is a pure library addition to `mofa-foundation`.

## 🧩 Additional Notes for Reviewers

- Implements both Phase 1 (core registry) and Phase 2 (weighted scoring) components
- Designed to integrate with existing work: #296 (InferenceBackend trait), #147 (Core Trait for Model Orchestration), #221 (Hardware Discovery Service)
- Includes builder patterns for all public types for ergonomic API usage
