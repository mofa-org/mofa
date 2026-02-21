# CLI Agent Commands - Quick Reference

## Quick Start

```bash
# Build
cargo build -p mofa-cli

# List all agents
cargo run -p mofa-cli -- agent list

# Create & start agent
cargo run -p mofa-cli -- agent start my-agent

# Stop agent
cargo run -p mofa-cli -- agent stop my-agent
```

## Common Commands

| Command | Purpose | Example |
|---------|---------|---------|
| `agent list` | Show all agents | `cargo run -p mofa-cli -- agent list` |
| `agent list --running` | Show running agents only | `cargo run -p mofa-cli -- agent list --running` |
| `agent start <id>` | Create & start agent | `cargo run -p mofa-cli -- agent start my-agent` |
| `agent start <id> --daemon` | Start as daemon | `cargo run -p mofa-cli -- agent start my-agent --daemon` |
| `agent stop <id>` | Stop agent | `cargo run -p mofa-cli -- agent stop my-agent` |
| `agent status <id>` | Check agent status | `cargo run -p mofa-cli -- agent status my-agent` |
| `agent --help` | Show help | `cargo run -p mofa-cli -- agent --help` |

## Database

| Task | Command |
|------|---------|
| Database location | `~/.mofa/agents.db` |
| Custom location | `$env:MOFA_STATE_DIR = "C:\path"` |
| View schema | `sqlite3 ~/.mofa/agents.db ".schema agents"` |
| Clean slate | `rm ~/.mofa/agents.db` |

## Test Scenarios

**Empty database:**
```bash
rm ~/.mofa/agents.db
cargo run -p mofa-cli -- agent list
```

**Create multiple agents:**
```bash
cargo run -p mofa-cli -- agent start agent-1
cargo run -p mofa-cli -- agent start agent-2
cargo run -p mofa-cli -- agent start agent-3
cargo run -p mofa-cli -- agent list
```

**Mix running & stopped:**
```bash
cargo run -p mofa-cli -- agent stop agent-1
cargo run -p mofa-cli -- agent list
cargo run -p mofa-cli -- agent list --running
```

**Error testing:**
```bash
# Already running
cargo run -p mofa-cli -- agent start agent-1

# Not found
cargo run -p mofa-cli -- agent stop nonexistent
```

## Exit Codes

| Code | Meaning |
|------|---------|
| `0` | Success |
| `1` | Error (agent not found, already running, etc.) |

## Status Values

| Status | Meaning |
|--------|---------|
| `running` | Agent is currently active |
| `stopped` | Agent is inactive |
| `paused` | Agent is paused |
| `error` | Agent encountered an error |

## Environment Variables

```powershell
# PowerShell
$env:MOFA_STATE_DIR = "C:\tmp\mofa-test"

# Bash
export MOFA_STATE_DIR=/tmp/mofa-test
```

## Output Format

```
→ Listing agents
  Showing running agents only

 ID              │ Name             │ Status  │ Uptime     │ Provider │ Model
─────────────────┼──────────────────┼─────────┼────────────┼──────────┼────────
 my-agent        │ my-agent         │ running │ 5m 23s     │ -        │ -
```

## Troubleshooting

| Issue | Solution |
|-------|----------|
| `Agent 'x' not found` | Agent doesn't exist in DB or wasn't created |
| Database locked | Wait for other commands or restart |
| Changes not persisting | Check `MOFA_STATE_DIR` is consistent |
| Schema errors | Delete DB: `rm ~/.mofa/agents.db` |

See [CLI_AGENT_COMMANDS_TESTING.md](./CLI_AGENT_COMMANDS_TESTING.md) for detailed guide.
