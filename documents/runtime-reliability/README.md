# Runtime Reliability Core (PR-1 Groundwork)

This document describes the first reliability building blocks added for `mofa run-flow`.

## Scope

- Typed flow schema parsing in `mofa/schema`.
- Schema version migration in `mofa/schema/versioning.py` (legacy v1 -> v2).
- Validation pipeline in `mofa/runtime/validation` with four stages:
  - syntax
  - type
  - semantic
  - dependency
- Rule engine in `mofa/runtime/validation/rules.py` with pluggable rule sets and aggregated diagnostics.
- Deterministic execution planner skeleton in `mofa/runtime/execution`.
- Retry/backoff policy primitives in `mofa/runtime/execution/retry.py` and planner metadata hooks per node.
- Minimal fail-fast integration in `mofa/commands/run_flow.py`.

## Schema Layer

`mofa/schema/flow.py` adds dataclasses for:

- `FlowSpec`
- `FlowNode`
- `FlowInputBinding`
- `FlowRetryPolicySpec`

Parsing helpers:

- `parse_yaml_text`
- `parse_yaml_file`
- `parse_flow_dict`

Literal-only checks reject expression-like scalar strings such as:

- `"a" * 1000`
- `[1, 2] * 3`
- `1 + 2`

## Validation Pipeline

`validate_and_plan_dataflow_descriptor` and `validate_and_plan_dataflow_file` provide one entry path.

Failure mode guarantees:

- Stage-tagged issues (`syntax`, `type`, `semantic`, `dependency`).
- Early rejection for unknown nodes/outputs.
- Cycle detection via execution planner.
- Pluggable validation rules and warning/error diagnostics in a single report.

## Execution Planner

Planner in `mofa/runtime/execution/planner.py` provides:

- Source reference parsing (`node/output`).
- Dependency graph extraction.
- Deterministic topological ordering (lexicographic tie-breaks).
- Cycle detection with explicit cycle path.

## Tests

Unit coverage lives in `tests/runtime` for:

- schema parse and literal checks
- validation stage behavior
- planner ordering and cycle detection
- run_flow validation integration path
- schema migration compatibility and richer constraints
- retry/backoff policy behavior and planning metadata
- fixture-driven valid/invalid flow scenarios

## Additional Docs

- `documents/runtime-reliability/rule-catalog.md`
- `documents/runtime-reliability/rule-reference.md`
- `documents/runtime-reliability/troubleshooting.md`
- `documents/runtime-reliability/migration-guide.md`
