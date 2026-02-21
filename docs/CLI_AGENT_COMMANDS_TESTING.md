# CLI Agent Commands Testing Guide

## Overview

This guide provides comprehensive testing instructions for the MoFA CLI agent management commands (`agent list`, `agent start`, `agent stop`). These commands manage agent state through a SQLite persistence layer.

**Implemented Commands:**
- `mofa agent list` - List all agents with status
- `mofa agent start` - Create and start an agent
- `mofa agent stop` - Stop a running agent

---

## Setup & Prerequisites

### Requirements
- Rust toolchain (1.70+)
- SQLite3 (optional, for manual database inspection)
- PowerShell or Bash terminal

### Build the Project

```bash
# Full build
cargo build

# Or build just the CLI crate
cargo build -p mofa-cli

# Release build (optimized)
cargo build -p mofa-cli --release
```

### Verify Build Success

```bash
# Check CLI help
cargo run -p mofa-cli -- agent --help
```

Expected output:
```
Agent management subcommands

Usage: mofa agent <COMMAND>

Commands:
  create    Create a new agent (interactive wizard)
  start     Start an agent
  stop      Stop a running agent
  restart   Restart an agent
  status    Show agent status
  list      List all agents
  destroy   Delete an agent
  help      Print this message or the help of a subcommand(s)
```

---

## Database Configuration

### Default Location
The SQLite database is stored at: `~/.mofa/agents.db`

- `~` = User home directory
  - **Windows**: `C:\Users\<username>\.mofa\agents.db`
  - **macOS/Linux**: `/home/<username>/.mofa/agents.db`

### Custom Location
Override the default location using the `MOFA_STATE_DIR` environment variable:

**PowerShell:**
```powershell
$env:MOFA_STATE_DIR = "C:\tmp\mofa-test"
cargo run -p mofa-cli -- agent list
```

**Bash:**
```bash
export MOFA_STATE_DIR=/tmp/mofa-test
cargo run -p mofa-cli -- agent list
```

### View Database Schema

```bash
# Using SQLite CLI
sqlite3 ~/.mofa/agents.db ".schema agents"

# Or inspect with SQLite GUI tools (DB Browser for SQLite, etc.)
```

Expected schema:
```sql
CREATE TABLE agents (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  status TEXT NOT NULL,
  started_at INTEGER,
  provider TEXT,
  model TEXT,
  created_at TEXT,
  updated_at TEXT
)
```

---

## Test Scenarios

### Scenario 1: Fresh Start (Empty Database)

**Goal:** Verify proper behavior when no agents exist

```bash
# 1. Clean slate - remove existing database
Remove-Item -Path $env:USERPROFILE\.mofa\agents.db -Force -ErrorAction SilentlyContinue

# 2. List agents (should show empty)
cargo run -p mofa-cli -- agent list
```

**Expected Output:**
```
→ Listing agents

  No agents found.
```

**Exit Code:** `0` (success)

---

### Scenario 2: Create First Agent

**Goal:** Create and start a new agent

```bash
# Start an agent
cargo run -p mofa-cli -- agent start my-first-agent
```

**Expected Output:**
```
→ Starting agent: my-first-agent
✓ Agent 'my-first-agent' started
```

**Exit Code:** `0`

**Verification:**
```bash
# List agents - should show 1
cargo run -p mofa-cli -- agent list
```

**Expected Output:**
```
→ Listing agents

 ID              │ Name             │ Status  │ Uptime     │ Provider │ Model
─────────────────┼──────────────────┼─────────┼────────────┼──────────┼────────
 my-first-agent  │ my-first-agent   │ running │ <duration> │ -        │ -
```

---

### Scenario 3: Multiple Agents

**Goal:** Create multiple agents and verify list display

```bash
# Create agents
cargo run -p mofa-cli -- agent start agent-alpha
cargo run -p mofa-cli -- agent start agent-beta
cargo run -p mofa-cli -- agent start agent-gamma

# List all agents
cargo run -p mofa-cli -- agent list
```

**Expected Output:**
```
→ Listing agents

 ID              │ Name             │ Status  │ Uptime     │ Provider │ Model
─────────────────┼──────────────────┼─────────┼────────────┼──────────┼────────
 agent-alpha     │ agent-alpha      │ running │ <duration> │ -        │ -
 agent-beta      │ agent-beta       │ running │ <duration> │ -        │ -
 agent-gamma     │ agent-gamma      │ running │ <duration> │ -        │ -
 my-first-agent  │ my-first-agent   │ running │ <duration> │ -        │ -
```

---

### Scenario 4: Stop an Agent

**Goal:** Stop a running agent and verify status change

```bash
# Stop one agent
cargo run -p mofa-cli -- agent stop agent-alpha

# List agents - should show stopped status
cargo run -p mofa-cli -- agent list
```

**Expected Output from `agent stop`:**
```
→ Stopping agent: agent-alpha
✓ Agent 'agent-alpha' stopped
```

**Expected Output from `agent list`:**
```
 ID              │ Name             │ Status   │ Uptime     │ Provider │ Model
─────────────────┼──────────────────┼──────────┼────────────┼──────────┼────────
 agent-alpha     │ agent-alpha      │ stopped  │ -          │ -        │ -
 agent-beta      │ agent-beta       │ running  │ <duration> │ -        │ -
 agent-gamma     │ agent-gamma      │ running  │ <duration> │ -        │ -
 my-first-agent  │ my-first-agent   │ running  │ <duration> │ -        │ -
```

---

### Scenario 5: Start Already-Running Agent (Idempotent)

**Goal:** Verify that starting an already-running agent is safe

```bash
# Try to start an agent that's already running
cargo run -p mofa-cli -- agent start agent-beta
```

**Expected Output:**
```
→ Starting agent: agent-beta
! Agent 'agent-beta' is already running
```

**Exit Code:** `0` (success - no error)

---

### Scenario 6: Stop Non-Existent Agent

**Goal:** Verify error handling for invalid agent ID

```bash
# Try to stop an agent that doesn't exist
cargo run -p mofa-cli -- agent stop nonexistent-agent
```

**Expected Output:**
```
→ Stopping agent: nonexistent-agent
✗ Agent 'nonexistent-agent' not found
Error: Agent not found: nonexistent-agent
```

**Exit Code:** `1` (error)

---

### Scenario 7: List Only Running Agents

**Goal:** Test filtering by status

```bash
# List only running agents (if you have mixed status)
cargo run -p mofa-cli -- agent list --running
```

**Expected Output:**
Shows only agents with `status = running`

```
→ Listing agents
  Showing running agents only

 ID              │ Name             │ Status  │ Uptime     │ Provider │ Model
─────────────────┼──────────────────┼─────────┼────────────┼──────────┼────────
 agent-beta      │ agent-beta       │ running │ <duration> │ -        │ -
 agent-gamma     │ agent-gamma      │ running │ <duration> │ -        │ -
 my-first-agent  │ my-first-agent   │ running │ <duration> │ -        │ -
```

---

### Scenario 8: Daemon Mode

**Goal:** Start an agent in daemon mode

```bash
# Start agent with daemon flag
cargo run -p mofa-cli -- agent start daemon-agent --daemon
```

**Expected Output:**
```
→ Starting agent: daemon-agent
  Mode: daemon
✓ Agent 'daemon-agent' started
```

**Verification:**
```bash
cargo run -p mofa-cli -- agent list
```

The agent should appear with `status = running`

---

## Complete Test Workflow

Run this complete test sequence to verify all functionality:

```powershell
# PowerShell script

Write-Host "=== MoFA Agent CLI Test Workflow ===" -ForegroundColor Green
Write-Host ""

# Clean start
Write-Host "1. Cleaning database..." -ForegroundColor Cyan
Remove-Item -Path "$env:USERPROFILE\.mofa\agents.db" -Force -ErrorAction SilentlyContinue

Write-Host "2. Test empty list..." -ForegroundColor Cyan
cargo run -p mofa-cli -- agent list
Write-Host ""

Write-Host "3. Create agent-01..." -ForegroundColor Cyan
cargo run -p mofa-cli -- agent start agent-01
Write-Host ""

Write-Host "4. List (should show 1)..." -ForegroundColor Cyan
cargo run -p mofa-cli -- agent list
Write-Host ""

Write-Host "5. Create agent-02 and agent-03..." -ForegroundColor Cyan
cargo run -p mofa-cli -- agent start agent-02
cargo run -p mofa-cli -- agent start agent-03
Write-Host ""

Write-Host "6. List (should show 3)..." -ForegroundColor Cyan
cargo run -p mofa-cli -- agent list
Write-Host ""

Write-Host "7. Stop agent-01..." -ForegroundColor Cyan
cargo run -p mofa-cli -- agent stop agent-01
Write-Host ""

Write-Host "8. List (mixed status)..." -ForegroundColor Cyan
cargo run -p mofa-cli -- agent list
Write-Host ""

Write-Host "9. List running only..." -ForegroundColor Cyan
cargo run -p mofa-cli -- agent list --running
Write-Host ""

Write-Host "10. Try to start running agent..." -ForegroundColor Cyan
cargo run -p mofa-cli -- agent start agent-02
Write-Host ""

Write-Host "11. Try to stop non-existent..." -ForegroundColor Cyan
cargo run -p mofa-cli -- agent stop nonexistent
Write-Host ""

Write-Host "=== All tests complete ===" -ForegroundColor Green
```

---

## Bash Version

```bash
#!/bin/bash

echo "=== MoFA Agent CLI Test Workflow ==="
echo ""

# Clean start
echo "1. Cleaning database..."
rm -f ~/.mofa/agents.db

echo "2. Test empty list..."
cargo run -p mofa-cli -- agent list
echo ""

echo "3. Create agent-01..."
cargo run -p mofa-cli -- agent start agent-01
echo ""

echo "4. List (should show 1)..."
cargo run -p mofa-cli -- agent list
echo ""

echo "5. Create agent-02 and agent-03..."
cargo run -p mofa-cli -- agent start agent-02
cargo run -p mofa-cli -- agent start agent-03
echo ""

echo "6. List (should show 3)..."
cargo run -p mofa-cli -- agent list
echo ""

echo "7. Stop agent-01..."
cargo run -p mofa-cli -- agent stop agent-01
echo ""

echo "8. List (mixed status)..."
cargo run -p mofa-cli -- agent list
echo ""

echo "9. List running only..."
cargo run -p mofa-cli -- agent list --running
echo ""

echo "10. Try to start running agent..."
cargo run -p mofa-cli -- agent start agent-02
echo ""

echo "11. Try to stop non-existent..."
cargo run -p mofa-cli -- agent stop nonexistent
echo ""

echo "=== All tests complete ==="
```

---

## Troubleshooting

### Issue: `Error: Agent not found` on First Start

**Cause:** Agent creation is failing

**Solution:**
1. Check database location: `ls -la ~/.mofa/` (Unix) or `dir $env:USERPROFILE\.mofa` (Windows)
2. Verify write permissions to `.mofa` directory
3. Check logs with verbose flag: `cargo run -p mofa-cli -- -v agent start test-agent`

### Issue: Database Locked Error

**Cause:** Multiple CLI instances accessing database simultaneously

**Solution:**
1. Wait for running commands to complete
2. Or use separate `MOFA_STATE_DIR` for concurrent testing
3. Check for stale processes: `ps aux | grep mofa` (Unix)

### Issue: Changes Not Persisting

**Cause:** Using different `MOFA_STATE_DIR` values between commands

**Solution:**
1. Verify environment variable: `echo $MOFA_STATE_DIR` or `$env:MOFA_STATE_DIR`
2. Unset if not needed: `unset MOFA_STATE_DIR` (Bash) or `Remove-Item Env:MOFA_STATE_DIR` (PowerShell)

### Issue: Table Not Found Error

**Cause:** Database schema not initialized

**Solution:**
1. Delete database: `rm ~/.mofa/agents.db`
2. Run any command to reinitialize: `cargo run -p mofa-cli -- agent list`

---

## Performance Expectations

| Operation | Expected Time |
|-----------|---------------|
| List 1-10 agents | < 100ms |
| Start new agent | 200-500ms |
| Stop running agent | 200-500ms |
| List 100 agents | < 500ms |

---

## Implementation Details

### Trait Requirements
The commands rely on the `AgentStateStore` trait:

```rust
pub trait AgentStateStore: Send + Sync {
    async fn list(&self) -> anyhow::Result<Vec<AgentRecord>>;
    async fn get(&self, agent_id: &str) -> anyhow::Result<Option<AgentRecord>>;
    async fn create(&self, record: AgentRecord) -> anyhow::Result<()>;
    async fn update(&self, record: AgentRecord) -> anyhow::Result<()>;
    async fn delete(&self, agent_id: &str) -> anyhow::Result<()>;
    async fn exists(&self, agent_id: &str) -> anyhow::Result<bool>;
}
```

**Implementation:** `SqliteAgentStateStore` in `crates/mofa-cli/src/state/sqlite.rs`

### Agent Record Fields

```rust
pub struct AgentRecord {
    pub id: String,              // Unique identifier
    pub name: String,            // Display name
    pub status: AgentStatus,     // Running/Stopped/Paused/Error
    pub started_at: Option<u64>, // Unix timestamp
    pub provider: Option<String>,
    pub model: Option<String>,
}
```

### Command Flow

```
CLI Input
  ↓
AgentCommands enum (from clap parser)
  ↓
Command handler (list.rs, start.rs, start.rs)
  ↓
get_agent_store() → SqliteAgentStateStore
  ↓
Database operations (create, get, list, update)
  ↓
Format output (Table display)
  ↓
Print to stdout
  ↓
Exit with status code (0 = success, 1 = error)
```

---

## Files Modified

- `crates/mofa-cli/src/commands/agent/list.rs` - List all agents
- `crates/mofa-cli/src/commands/agent/start.rs` - Create and start agents
- `crates/mofa-cli/src/commands/agent/stop.rs` - Stop running agents
- `crates/mofa-cli/src/state/mod.rs` - State management
- `crates/mofa-cli/src/state/store.rs` - Store trait definition
- `crates/mofa-cli/src/state/sqlite.rs` - SQLite implementation

---

## Related Documentation

- [MoFA Architecture](./architecture.md)
- [CLI Usage Guide](./usage.md)
- [Database Setup](./database_setup.md)

---

## Support

For issues or questions:
1. Check this guide's Troubleshooting section
2. Review build warnings/errors: `cargo build -p mofa-cli 2>&1 | grep -i error`
3. Enable verbose logging: `cargo run -p mofa-cli -- -v agent list`
4. Open an issue: https://github.com/mofa-org/mofa/issues
