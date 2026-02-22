# CLI Production Smoke Example

This example validates MoFA CLI behavior from a user perspective by invoking the
`mofa` binary as a subprocess and checking command outcomes.

## What it validates

- Top-level smoke commands: `info`, `config path`, `config list`
- Agent lifecycle: `start`, `status`, `list`, `restart`, `stop`
- Session lifecycle: `list`, `show`, `export`, `delete`
- Plugin lifecycle: `list`, `info`, `uninstall`
- Tool commands: `list`, `info`

## Prerequisites

Build the CLI binary from repo root:

```bash
cargo build -p mofa-cli
```

If needed, override binary path:

```bash
export MOFA_BIN=/absolute/path/to/mofa
```

## Run

From `examples/`:

```bash
cargo run -p cli_production_smoke
cargo test -p cli_production_smoke
```

## Notes

- The runner uses isolated `XDG_*` directories in a temporary folder.
- Assertions are based on stable output tokens and exit codes (not full snapshots).
- This is a manual smoke workflow; CI wiring can be added in a follow-up PR.
