# mofa-extra

Additional utilities and extensions for MoFA.

## Purpose

`mofa-extra` provides:
- Utility functions
- Extension traits
- Helper types
- Experimental features

## Contents

| Module | Description |
|--------|-------------|
| `utils` | General utilities |
| `extensions` | Extension traits |
| `experimental` | Experimental features |

## Usage

```rust
use mofa_extra::utils::json::parse_safe;
use mofa_extra::extensions::AgentExt;

let parsed = parse_safe(&json_string);
agent.execute_with_retry(input, 3).await?;
```

## Feature Flags

| Flag | Description |
|------|-------------|
| `all` | Enable all utilities |

## See Also

- [API Reference](../api-reference/kernel/README.md) — Core API
