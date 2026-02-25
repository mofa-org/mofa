## 📋 Summary

This PR introduces Phase 1 of the Execution Timeline Profiler - an opt-in profiling system for capturing workflow execution timing data. This enables developers to measure and analyze node and tool execution durations without affecting normal workflow behavior when disabled.

## 🔗 Related Issues

Closes #458 (part of Deterministic Workflow Execution Replay & Trace Engine)

---

## 🧠 Context

As MoFA workflows grow in complexity, understanding execution performance becomes increasingly important. Currently, there is no built-in mechanism to capture execution timing data for analysis and optimization.

This feature adds a lightweight, opt-in profiler that:
- Records node execution start/end times
- Records tool invocation start/end times
- Uses existing DebugEvent::now_ms() for timestamps
- Has zero overhead when disabled

---

## 🛠️ Changes

- **New file: `workflow/profiler.rs`**
  - `ProfilerMode` enum: `Disabled` or `Record(ExecutionTimeline)`
  - `NodeSpan`: struct capturing node execution timing (node_id, started_at_ms, ended_at_ms, duration_ms)
  - `ToolSpan`: struct capturing tool execution timing (tool_id, tool_name, started_at_ms, ended_at_ms, duration_ms)
  - `ExecutionTimeline`: complete workflow timing data with workflow_id, execution_id, node_spans
  - `ProfilerHandle`: accessor for retrieving recorded timeline
  - 4 unit tests covering disabled mode, workflow duration, node duration, and tool duration

- **Modified: `workflow/executor.rs`**
  - Added `profiler: ProfilerMode` field to `WorkflowExecutor` struct (default: `Disabled`)
  - Added `with_profiler(mode: ProfilerMode)` method to enable profiling
  - Added `profiler_timeline() -> Option<&ExecutionTimeline>` method to retrieve timeline

- **Modified: `workflow/mod.rs`**
  - Added `mod profiler;` and `pub use profiler::*;`

---

## 🧪 How you Tested

1. **Unit Tests**: All 4 new profiler tests pass
   ```bash
   cargo test --package mofa-foundation workflow::profiler::tests
   ```

2. **Full Test Suite**: All 241 tests in mofa-foundation pass
   ```bash
   cargo test --package mofa-foundation
   ```

3. **Compilation**: Code compiles without errors or warnings

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
- [x] Tests added/updated (4 new tests)
- [x] `cargo test` passes locally without any error

### Documentation
- [x] Public APIs documented (inline docs)
- [ ] README / docs updated (not required for Phase 1)

### PR Hygiene
- [x] PR is small and focused (one logical change)
- [x] Branch is up to date with `main`
- [x] No unrelated commits
- [x] Commit messages explain **why**, not only **what**

---

## 🚀 Deployment Notes

No deployment changes required. This is an opt-in feature with zero runtime cost when disabled.

---

## 🧩 Additional Notes for Reviewers

- The profiler uses existing `DebugEvent::now_ms()` from mofa-kernel for timestamps
- When `ProfilerMode::Disabled`, the executor behavior is identical to before
- The profiler can be enabled via `WorkflowExecutor::new().with_profiler(ProfilerMode::Record(ExecutionTimeline::new(...)))`
- This is Phase 1 of the Execution Timeline Profiler; future phases may add:
  - Async span support
  - Memory profiling
  - Export to various formats (JSON, Prometheus)
