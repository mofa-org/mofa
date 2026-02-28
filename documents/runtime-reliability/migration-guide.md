# Flow Schema Migration Guide (v1 -> v2)

The runtime parser now migrates legacy descriptors to schema v2 automatically.

## Version Fields

- Preferred field: `schema_version`
- Legacy field supported: `version`
- Missing version defaults to v1 and is migrated.

## Automatic Migrations

- `version` -> `schema_version`
- Node `kind` -> `type`
- Node `environment` -> `env`
- Node `output: out` -> `outputs: [out]`
- Input `source:out` -> `source/out`
- Input mapping `{node, output, queue}` -> `{source, queue_size}`

## Node Type Contracts

Supported types:

- `source`
- `agent`
- `transformer`
- `router`
- `sink`

Behavioral constraints:

- `source` nodes must not define inputs.
- `agent`/`transformer`/`router` nodes require non-empty `build` and `path`.
- `sink` outputs are allowed but warned.

## Env Constraints

- Env keys must match `^[A-Z_][A-Z0-9_]*$`.
- Values must be scalar literals (`str`, `int`, `float`, `bool`, `null`).
- Placeholder values must match `$NAME`.

## Retry Policy

Optional node field:

```yaml
retry:
  max_attempts: 4
  initial_delay_seconds: 0.1
  backoff_multiplier: 2.0
  max_delay_seconds: 2.0
  jitter_ratio: 0.2
```

Validation constraints:

- `max_attempts >= 1`
- `backoff_multiplier >= 1.0`
- `jitter_ratio` in `[0, 1]`

Execution planner attaches retry schedule metadata per node.
