# Runtime Reliability Troubleshooting Guide

This guide maps common runtime validation failures to concrete diagnosis and recovery steps.
It is designed for engineers iterating on flow descriptors during PR1 reliability rollout.

## Quick Triage

When `mofa run-flow` fails at validation:

1. Copy the full validation exception output.
2. Identify `stage`, `severity`, and `rule_id` in the first failing line.
3. Jump to the matching section below.
4. Re-run targeted runtime tests after edits.

Primary test command:

```bash
python3 -m unittest discover -s tests/runtime -p 'test_*.py' -v
```

## Stage: `syntax`

### Symptom: Invalid YAML parse errors

Typical message:

- `Invalid YAML in <file>: ...`

Root causes:

- Broken indentation in nested `inputs` or `retry` blocks.
- Inline lists/maps with missing commas or brackets.
- Multi-line scalars with accidental unescaped characters.

Recovery steps:

1. Validate indentation with two-space nesting.
2. Expand one-line maps to block format while debugging.
3. Re-parse with an isolated minimal fixture.

### Symptom: Literal-only violation

Typical message:

- `Only literal YAML values are allowed. Expression-like values found: ...`

Root causes:

- String resembles executable expression.
- Value copied from Python examples into YAML directly.

Recovery steps:

1. Replace expression-like strings with explicit constants.
2. Move computed-value construction to pre-processing scripts.
3. Keep YAML as declarative configuration only.

## Stage: `type`

### Symptom: Node identifier mismatch

Typical message:

- `Field 'id' must match '^[A-Za-z][A-Za-z0-9_-]{0,63}$'`

Root causes:

- Identifier starts with a digit.
- Illegal characters such as `.` or spaces.

Recovery steps:

1. Rename nodes to alphanumeric plus `_`/`-` only.
2. Update all dependent `inputs` references.

### Symptom: Unsupported node type

Typical message:

- `field 'type' must be one of: agent, router, sink, source, transformer`

Root causes:

- Typo or use of deprecated custom type string.

Recovery steps:

1. Select the nearest supported type.
2. Encode additional behavior in node extras rather than type name.

### Symptom: Invalid `env` shape

Typical message:

- Invalid env key pattern.
- Env value is non-scalar.
- Placeholder mismatch for `$NAME` format.

Recovery steps:

1. Use env keys like `OPENAI_API_KEY`.
2. Restrict values to scalar literal types.
3. Fix placeholders to `$VARNAME`.

### Symptom: Retry policy type/range errors

Typical messages:

- `retry.max_attempts must be an integer >= 1`
- `retry.backoff_multiplier must be >= 1.0`
- `retry.jitter_ratio must be in [0, 1]`

Recovery steps:

1. Make every retry numeric field explicit.
2. Keep jitter in inclusive `[0, 1]`.
3. Start with defaults, then tune incrementally.

## Stage: `semantic`

### Symptom: Duplicate node IDs

Typical message:

- `Duplicate node id`

Recovery steps:

1. Rename duplicate IDs.
2. Re-run tests to catch any stale references.

### Symptom: Duplicate outputs on one node

Typical message:

- `Duplicate output '<name>'`

Recovery steps:

1. Rename outputs to distinct names.
2. Ensure downstream references match renamed output.

### Symptom: Source node declares inputs

Typical message:

- `Source nodes must not declare inputs`

Recovery steps:

1. Move the node to `agent`/`transformer` if transformation is intended.
2. Keep `source` nodes as ingress-only producers.

### Symptom: Sink node declares outputs (warning)

Typical message:

- `Sink nodes should not declare outputs`

Recovery steps:

1. Remove sink outputs when possible.
2. If retained for compatibility, track with explicit warning acceptance.

### Symptom: Oversized queue warning

Typical message:

- `queue_size <n> exceeds max 100000; this can create memory pressure`

Recovery steps:

1. Reduce queue size.
2. Add intermediate buffering nodes.
3. Tune concurrency and downstream throughput.

### Symptom: High retry attempts warning

Typical message:

- `retry.max_attempts <n> exceeds limit 20`

Recovery steps:

1. Lower attempt count.
2. Add failure classification to avoid retrying permanent errors.
3. Log retry outcomes for empirical tuning.

## Stage: `dependency`

### Symptom: Invalid source format

Typical message:

- `Invalid source reference 'x'. Expected '<node-id>/<output-name>'`

Recovery steps:

1. Use slash separator.
2. Ensure both node and output segments are non-empty.

### Symptom: Unknown source node

Typical message:

- `Input references unknown node '<id>'`

Recovery steps:

1. Fix typo in `inputs.<name>.source`.
2. Add missing source node.

### Symptom: Unknown source output

Typical message:

- `Input references unknown output '<name>' from node '<id>'`

Recovery steps:

1. Update output name in source node.
2. Update downstream binding to match.

### Symptom: Dependency cycle

Typical message:

- `Dependency cycle detected: a -> b -> a`

Recovery steps:

1. Split cyclic logic into separate flows.
2. Introduce source boundaries or persisted handoff between stages.
3. Re-check graph with deterministic planner order.

## Snapshot Drift Troubleshooting

If runtime tests fail due to snapshot mismatch:

1. Confirm whether behavior changed intentionally.
2. If intentional, regenerate snapshots and review diff.
3. If unintentional, inspect the first changed field in JSON snapshots.
4. Determine if drift came from rule order, severity change, message text, or planner metadata.

Most common drift categories:

- Changed diagnostic sort order.
- New rule IDs introduced.
- Message wording update without behavior change.
- Retry schedule change due to policy defaults.

## Migration-Specific Failures

### Symptom: Legacy descriptor fails after migration

Checks:

1. Verify source file actually contains v1-style fields (`kind`, `environment`, `output`).
2. Confirm migrated output includes `schema_version: 2`.
3. Validate migrated input bindings (`source`, `queue_size`).

Recovery:

- Run targeted migration tests in `tests/runtime/test_schema_migration.py`.
- Add or update legacy fixture and snapshot for the discovered shape.

## Debugging Workflow

Recommended debugging sequence:

1. Reproduce with a single fixture from `tests/runtime/fixtures`.
2. Reduce fixture to minimal failing case.
3. Identify whether failure originates in parser, rule engine, or planner.
4. Add/adjust fixture catalog entry to prevent regression.
5. Update snapshot only if behavior change is deliberate.

## Escalation Heuristics

Escalate to maintainers when:

- Validation fails nondeterministically across repeated runs.
- Equivalent descriptors produce different planner order.
- Rule diagnostics contradict schema parser errors.
- Migration output changes unexpectedly without code changes.

Include in escalation report:

1. Offending fixture path.
2. Full validation exception.
3. Expected behavior.
4. Snapshot diff excerpt.
5. Commit hash or branch reference.

## Preventive Practices

1. Keep flow descriptors small and composable.
2. Prefer explicit node types and minimal extras.
3. Treat warnings as debt, not noise.
4. Add fixture + snapshot for each newly discovered failure mode.
5. Run runtime tests before opening review-ready PRs.
