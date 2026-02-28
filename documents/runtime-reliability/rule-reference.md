# Runtime Validation Rule Reference

This document is the operator-focused reference for PR1 runtime reliability validation.
Each rule entry includes trigger conditions, representative examples, and remediation notes.

## Rule Taxonomy

Validation currently executes in this sequence:

1. YAML syntax and literal-only checks.
2. Schema migration and type normalization.
3. Semantic/rule engine checks.
4. Dependency planning and cycle detection.

Diagnostics include:

- `stage`: `syntax`, `type`, `semantic`, or `dependency`.
- `severity`: `error` or `warning`.
- `rule_id`: stable identifier for automation and snapshotting.
- `node_id` and `field`: optional location hints.
- `hint`: optional remediation direction.

## Syntax and Type Guards

### `syntax.literal_only`

Purpose:

- Blocks expression-like scalar strings that look executable.
- Preserves deterministic YAML semantics.

Typical offender patterns:

- Quoted string with arithmetic operator, such as `'"x" * 4'`.
- Numeric arithmetic strings like `'1 + 2'`.
- Strings containing `eval(`, `exec(`, `__import__(`, `lambda `.

Examples:

```yaml
nodes:
  - id: source
    type: source
    outputs: [out]
    env:
      BAD: '"token" * 3'
```

Remediation:

- Replace expression-style strings with concrete literals.
- Generate expanded values outside the YAML descriptor.

### `type.node_shape`

Purpose:

- Enforces shape of node-level fields.
- Prevents partial coercion and hidden type surprises.

Representative checks:

- `id` must be non-empty and match the identifier regex.
- `type` must be one of supported types.
- `outputs` must be list of valid output identifiers.
- `inputs` must be a mapping.
- `env` values must be scalar literals only.

Remediation:

- Keep each node field explicit and typed.
- Avoid mixed map/list forms for `inputs` and `outputs`.

## Semantic Rules

### `flow.non_empty`

Purpose:

- Ensures a descriptor has at least one node.

Failure signature:

- `Dataflow must contain at least one node`.

Remediation:

- Add at least one source/sink pair before runtime execution.

### `flow.unique_node_ids`

Purpose:

- Prevents ambiguous dependency references.

Failure signature:

- `Duplicate node id` with `node=<id>, field=id`.

Remediation:

- Rename duplicates and update all dependent `inputs` references.

### `flow.unique_outputs`

Purpose:

- Prevents ambiguous output bindings from a single node.

Failure signature:

- `Duplicate output '<name>'` with `field=outputs`.

Remediation:

- Use unique output names per node.

### `semantic.node_type_contract`

Purpose:

- Enforces role-specific contracts by node type.

Contract summary:

- `source` nodes must not define `inputs`.
- `sink` nodes should not define `outputs`.
- `agent`, `transformer`, `router` require non-empty `build` and `path`.

Severity behavior:

- Source-input violation is an `error`.
- Sink-output is currently a `warning` for compatibility.

Remediation:

- Remove invalid fields for role-specific nodes.
- For workers/routers, always provide concrete build/path.

### `semantic.queue_size_bounds`

Purpose:

- Flags oversized queue configuration that can cause memory pressure.

Default threshold:

- `queue_size > 100000` generates a warning.

Remediation:

- Lower queue size and introduce backpressure handling.
- Split large fan-in workloads into staged flows.

### `semantic.retry_bounds`

Purpose:

- Flags excessive retry attempts and tail-latency risk.

Default threshold:

- `retry.max_attempts > 20` generates a warning.

Remediation:

- Reduce retry attempts.
- Use bounded backoff and sensible timeouts.

## Dependency Rules

### `dependency.source_format`

Purpose:

- Validates source references conform to `<node-id>/<output-name>`.

Failure signature:

- `Invalid source reference`.

Remediation:

- Replace separators like `:` with `/`.
- Ensure both node and output segments are non-empty.

### `dependency.target_exists`

Purpose:

- Ensures every input references a known source node.

Failure signature:

- `Input references unknown node '<id>'`.

Remediation:

- Add the missing node or correct the reference.

### `dependency.output_exists`

Purpose:

- Ensures referenced outputs exist on the source node.

Failure signature:

- `Input references unknown output '<name>' from node '<id>'`.

Remediation:

- Correct output names to match source declaration.

### `dependency.cycle`

Purpose:

- Blocks cyclic dependency graphs during planning.

Failure signature:

- `Dependency cycle detected: ...`.

Remediation:

- Break feedback loops with explicit boundary nodes or staged execution.
- Ensure source roots remain cycle-free.

## Severity and CI Policy

Recommended CI defaults:

- Fail on any `error` severity diagnostic.
- Preserve warnings in artifacts and snapshots.
- Gate warning budget by project policy if needed.

Suggested local command:

```bash
python3 -m unittest discover -s tests/runtime -p 'test_*.py' -v
```

## Snapshot Strategy

Fixture snapshots in `tests/runtime/fixtures/snapshots` lock current behavior.
When changing rules:

1. Update rule implementation and tests.
2. Regenerate snapshots intentionally.
3. Review diff for expected severity/stage/rule movement.

Snapshot files are split into:

- `planner/*.json` for valid/edge flows.
- `errors/*.json` for invalid flows.

## Compatibility Notes

Legacy schema compatibility currently includes:

- `version` promoted to `schema_version`.
- `kind` migrated to `type`.
- `environment` migrated to `env`.
- `output` migrated to `outputs`.
- legacy input separator `node:out` migrated to `node/out`.
- legacy input mappings (`node`, `output`, `queue`) migrated to (`source`, `queue_size`).

If a migration path is missing, parser raises a migration/type failure before rule execution.

## Operations Checklist

Before shipping a flow descriptor:

1. Confirm `schema_version` is present or migration intent is explicit.
2. Run runtime test corpus for regressions.
3. Review warnings and justify any retained warning state.
4. Validate dependency order snapshot against expected execution topology.
5. Archive flow and snapshot pair in project artifacts if reproducibility matters.
