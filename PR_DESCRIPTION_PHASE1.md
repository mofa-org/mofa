# Execution Event Schema Definition (Phase 1)

## Summary

Defines the canonical `ExecutionEvent` enum and `ExecutionEventEnvelope` wrapper for versioned workflow execution tracing, replay, and monitoring.

This is Phase 1 of implementing the Execution Event Contract for WorkflowTrace, Replay & Monitoring.

## Context

As MoFA workflows grow in complexity, there is a need for a standardized event schema that can be used across:
- Workflow execution tracing
- Deterministic replay
- Monitoring and observability

This schema provides a versioned contract that ensures compatibility between different phases of the system and enables future evolution.

## Changes

- Created `execution_event.rs` module with:
  - `ExecutionEvent` enum with 13 canonical event types (WorkflowStarted, NodeStarted, NodeCompleted, NodeFailed, ToolInvoked, ToolCompleted, ToolFailed, StateUpdated, WorkflowCompleted, WorkflowFailed, NodeRetrying, BranchDecision, ParallelGroupStarted, ParallelGroupCompleted)
  - `ExecutionEventEnvelope` wrapper with `schema_version` field
  - `SCHEMA_VERSION` constant (currently v1)
  - Serde derives for JSON serialization/deserialization
  - `is_compatible()` method for schema validation
- Added module declaration to `workflow/mod.rs`

## How You Tested

1. `cargo test --package mofa-foundation` - All 243 tests pass
2. Unit tests validate:
   - Serialize → deserialize consistency
   - Schema version stored correctly in JSON
   - Event variants serialize correctly
   - Compatibility check works

## Breaking Changes

- [x] No breaking changes - completely additive, isolated schema definition

## Checklist

### Code Quality
- [x] Code follows Rust idioms and project conventions
- [x] `cargo fmt` run
- [x] `cargo clippy` passes without warnings

### Testing
- [x] Tests added/updated (6 new tests)
- [x] `cargo test` passes locally without any error

### Documentation
- [x] Public APIs documented with doc comments
- [ ] README / docs updated (N/A - internal module)

### PR Hygiene
- [x] PR is small and focused (one logical change)
- [x] Branch is up to date with `main`
- [x] No unrelated commits
- [x] Commit messages explain **why**, not only **what**

## Additional Notes

- This is an isolated Phase 1 - NO integration changes
- NO changes to replay engine in this phase
- NO changes to monitoring in this phase
- Follow-up: Phase 2 will integrate with WorkflowTrace and replay

### Phase Completion Notes
- Phase implemented: 1
- Breaking changes: None
- Replay compatibility: Preserved
- Monitoring compatibility: Unchanged
