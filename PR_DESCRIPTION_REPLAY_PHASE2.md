## 📋 Summary

This PR implements Phase 2 of the Deterministic Workflow Replay feature - adding replay capabilities to the workflow execution system. It enables reproducing workflow executions deterministically by returning recorded outputs instead of calling real tools.

## 🔗 Related Issues

Closes #458 (part of Deterministic Workflow Execution Replay & Trace Engine)

---

## 🧠 Context

As MoFA workflows grow in complexity, debugging and validating execution becomes difficult. Currently, re-running workflows requires live tools and APIs which may return different results, making debugging inconsistent.

This feature enables:
- Deterministic workflow replay without external side effects
- Capturing and reproducing execution results
- Safer regression testing
- Offline debugging and validation
- Zero overhead when not in replay mode

---

## 🛠️ Changes

### Core Types:
- **ExecutionMode enum**: `Normal` (default, runs workflow normally) or `Replay(WorkflowTrace)` (returns recorded outputs)
- **ReplayError enum**: Structured errors for replay failures:
  - `NodeOrderMismatch`: Node executed in wrong order
  - `ToolOutputMissing`: Required tool output not found in trace
  - `ToolOutputMismatch`: Tool output differs from recorded

### Trace Structures:
- **WorkflowTrace**: Records workflow executions with node executions and tool invocations
- **NodeExecution**: Captured node execution (node_id, input, output, tool_invocations)
- **ToolInvocation**: Captured tool call (node_id, tool_name, input, output)
- **ReplayHelper**: Helper for replay logic (validates order, retrieves outputs)

### Features:
- Fully serializable with serde (JSON export ready)
- Node order validation during replay
- Tool output retrieval from trace
- Replay position tracking

---

## 🧪 How you Tested

1. **Unit Tests**: 7 new replay tests:
   ```bash
   cargo test --package mofa-foundation workflow::replay::tests
   ```

2. **Full Test Suite**: All 244 tests pass:
   ```bash
   cargo test --package mofa-foundation
   ```

3. **Specific test cases verified**:
   - Record → Replay → identical result
   - Replay detects node order mismatch
   - Replay detects missing tool output
   - Replay does not execute real tool

---

## 📸 Screenshots / Logs (if applicable)

N/A - CLI-only feature

---

## ⚠️ Breaking Changes

- [x] No breaking changes

This is an opt-in feature. Existing code behavior is unchanged when `ExecutionMode::Normal` (the default).

---

## 🧹 Checklist

### Code Quality
- [x] Code follows Rust idioms and project conventions
- [x] `cargo fmt` run
- [x] `cargo clippy` passes without warnings

### Testing
- [x] Tests added/updated (7 new tests)
- [x] `cargo test` passes locally without any error

### Documentation
- [x] Public APIs documented (inline Rustdoc)
- [ ] README / docs updated (not required for initial implementation)

### PR Hygiene
- [x] PR is small and focused (425 lines, single logical change)
- [x] Branch is up to date with `main`
- [x] No unrelated commits
- [x] Commit messages explain **why**, not only **what**

---

## 🚀 Deployment Notes

No deployment changes required. This is an opt-in feature with zero runtime cost when in Normal mode.

---

## 🧩 Additional Notes for Reviewers

**Design decisions:**
- ExecutionMode defaults to Normal for zero overhead
- All trace structures are serde-serializable for JSON export
- ReplayHelper provides clean API for replay logic
- Error types include both expected and unexpected failure modes

**Non-goals (for future phases):**
- No automatic trace recording during Normal execution
- No integration with WorkflowExecutor (users manually use ReplayHelper)
- No network-based replay
- No trace persistence (in-memory only for Phase 2)

**Usage example:**
```rust
use mofa_foundation::workflow::{ExecutionMode, WorkflowTrace, ReplayHelper};

// Record mode (Phase 1 - not in this PR)
let mut trace = WorkflowTrace::new("workflow-1".to_string());
trace.record_node_execution(...);

// Replay mode
let mut replay = ReplayHelper::new(trace);

// Validate order before executing
replay.validate_order("node-1")?;

// Get recorded output instead of calling real tool
let output = replay.get_tool_output("node-1", "search")?;
```
