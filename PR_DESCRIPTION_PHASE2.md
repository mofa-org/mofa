## 📋 Summary

This PR implements **Phase 2** of the Deterministic Workflow Execution Replay & Trace Engine feature (#458). It adds replay mode capabilities that enable deterministic workflow execution replay from previously recorded traces, enabling offline debugging, regression testing, and validation without invoking real tools.

## 🔗 Related Issues

Closes #458

---

## 🧠 Context

As MoFA workflows grow in complexity, debugging and validating execution becomes increasingly difficult. Currently, there's no mechanism to replay workflow execution deterministically. This PR introduces the replay infrastructure that:

- Enables replaying workflows from recorded traces without external side effects
- Validates execution order, inputs, outputs, and state transitions match the recorded trace
- Detects and reports mismatches during replay for debugging

---

## 🛠️ Changes

- Added `TraceMode::Replay` variant for configuring replay mode
- Added `ReplayMismatchType` enum with mismatch types: `NodeOrderMismatch`, `InputMismatch`, `OutputMismatch`, `StateMismatch`, `StatusMismatch`, `ExcessExecution`, `MissingExecution`
- Added `ReplayMismatch` struct for structured error reporting with fatal/non-fatal severity
- Added `ReplayState` state machine with validation methods:
  - `validate_node_start()` - Validates node execution order matches recorded trace
  - `validate_node_end()` - Validates outputs and status match recorded values
  - `get_replayed_output()` - Returns recorded output for replay
  - `finish()` - Checks for missing executions
  - `mismatches()` - Returns all detected mismatches
- Updated executor's `trace_handle()` to handle `Replay` variant
- Added 5 new tests for replay functionality

---

## 🧪 How you Tested

1. **All existing tests pass**: `cargo test --package mofa-foundation` - 310 tests pass
2. **New replay tests added**:
   - `test_replay_state_creation` - Tests ReplayState initialization
   - `test_replay_mismatch_types` - Tests mismatch type creation
   - `test_trace_mode_replay` - Tests TraceMode::Replay variant
   - `test_replay_state_validation` - Tests node validation
   - `test_replay_output_retrieval` - Tests output retrieval during replay

---

## ⚠️ Breaking Changes

- [x] No breaking changes

---

## 🧹 Checklist

### Code Quality
- [x] Code follows Rust idioms and project conventions
- [x] `cargo fmt` run (code is formatted)
- [x] `cargo clippy` passes without warnings

### Testing
- [x] Tests added/updated (5 new tests)
- [x] `cargo test` passes locally without any error (310 tests pass)

### Documentation
- [x] Public APIs documented (Rustdoc comments)
- [ ] README / docs updated (not required for this phase)

### PR Hygiene
- [x] PR is small and focused (one logical change)
- [x] Branch is up to date with `main`
- [x] No unrelated commits
- [x] Commit messages explain **why**, not only **what**

---

## 🚀 Deployment Notes (if applicable)

This is a feature addition with no deployment impact. Replay mode is fully opt-in and non-breaking.

---

## 🧩 Additional Notes for Reviewers

This PR builds on Phase 1 (Trace Capture) and provides the foundation for Phase 3 (Deterministic Validation). The replay functionality is fully optional and does not affect existing execution flows. All trace recording remains opt-in as specified in the issue.
