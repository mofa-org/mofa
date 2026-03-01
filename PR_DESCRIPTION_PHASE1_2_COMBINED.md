## 📋 Summary

This PR implements Phase 1 and Phase 2 of the Execution Timeline Profiler - an opt-in system for capturing and analyzing workflow execution timing data.

**Phase 1** adds timeline recording capabilities: profiling mode control, node and tool span tracking, and timeline collection.

**Phase 2** adds analysis capabilities: critical path identification, statistical analysis (min/max/mean), and percentile calculations (p50, p95).

## 🔗 Related Issues

Closes #458 (part of Deterministic Workflow Execution Replay & Trace Engine)

---

## 🧠 Context

As MoFA workflows grow in complexity, understanding execution performance becomes critical. Currently, there's no built-in mechanism to capture or analyze execution timing data.

This feature enables:
- Recording node and tool execution times with minimal overhead
- Identifying performance bottlenecks via critical path analysis
- Computing statistical summaries (min, max, mean, percentiles)
- Zero overhead when profiler is disabled

---

## 🛠️ Changes

### Phase 1 - Timeline Recording:
- **New `ProfilerMode` enum**: `Disabled` (default, zero overhead) or `Record(ExecutionTimeline)`
- **New `NodeSpan` struct**: captures node_id, started_at_ms, ended_at_ms, duration_ms, tool_spans
- **New `ToolSpan` struct**: captures tool_id, tool_name, started_at_ms, ended_at_ms, duration_ms
- **New `ExecutionTimeline` struct**: workflow_id, execution_id, node_spans with start_node/end_node/start_tool/end_tool methods
- Uses existing `DebugEvent::now_ms()` from mofa-kernel for timestamps

### Phase 2 - Analysis Capabilities:
- **`critical_path()`**: Returns node spans sorted by duration descending - identifies longest-running nodes
- **`node_stats()`**: Returns `TimelineStats` (min, max, mean) across all node durations
- **`percentile_stats()`**: Returns `PercentileStats` (p50, p95) using nearest-rank method
- **`percentile()`**: Private helper function for percentile calculation
- **`total_duration_ms()`**: Returns sum of all node durations

---

## 🧪 How you Tested

1. **Unit Tests**: 8 new tests covering all functionality:
   ```bash
   cargo test --package mofa-foundation workflow::profiler::tests
   ```

2. **Full Test Suite**: All 245 tests pass:
   ```bash
   cargo test --package mofa-foundation
   ```

3. **Doctests**: All pass

---

## 📸 Screenshots / Logs (if applicable)

N/A - CLI-only feature

---

## ⚠️ Breaking Changes

- [x] No breaking changes

This is an opt-in feature. Existing code behavior is unchanged when `ProfilerMode::Disabled` (the default).

---

## 🧹 Checklist

### Code Quality
- [x] Code follows Rust idioms and project conventions
- [x] `cargo fmt` run
- [x] `cargo clippy` passes without warnings

### Testing
- [x] Tests added/updated (8 new tests)
- [x] `cargo test` passes locally without any error

### Documentation
- [x] Public APIs documented (inline Rustdoc)
- [ ] README / docs updated (not required for initial implementation)

### PR Hygiene
- [x] PR is small and focused (438 lines, single logical change)
- [x] Branch is up to date with `main`
- [x] No unrelated commits
- [x] Commit messages explain **why**, not only **what**

---

## 🚀 Deployment Notes

No deployment changes required. This is an opt-in feature with zero runtime cost when disabled.

---

## 🧩 Additional Notes for Reviewers

**Design decisions:**
- Used existing `DebugEvent::now_ms()` to avoid adding new time sources
- Critical path uses simple duration-based sorting (not graph-based) for Phase 2
- Percentile uses nearest-rank method as specified
- All analysis methods return `Option` for empty timeline handling

**Non-goals (for future phases):**
- No OpenTelemetry integration
- No async instrumentation
- No JSON export
- No serialization logic
- No graph dependency analysis

**Usage example:**
```rust
use mofa_foundation::workflow::{ExecutionTimeline, ProfilerMode};

// Enable profiling
let timeline = ExecutionTimeline::new("my-workflow".to_string(), "exec-1".to_string());
let mode = ProfilerMode::Record(timeline);

// After execution...
let critical = timeline.critical_path();
let stats = timeline.node_stats();
let percentiles = timeline.percentile_stats();
```
