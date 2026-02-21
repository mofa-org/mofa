# Agent CLI Commands - Implementation Guide

## Overview

This document explains the architecture, implementation details, and design decisions for the agent management CLI commands (`agent list`, `agent start`, `agent stop`).

**Issue:** #127 - Implement mofa agent list/start/stop CLI commands (previously returned hardcoded mock data)

---

## Architecture

### Layer Stack

```
┌─────────────────────────────────────────┐
│  CLI Command Handler Layer              │
│  (list.rs, start.rs, stop.rs)           │
│  - Parse CLI arguments                  │
│  - Call async functions                 │
│  - Format & display output              │
└────────────────┬────────────────────────┘
                 │
┌────────────────▼────────────────────────┐
│  State Management Layer                 │
│  (mod.rs, store.rs)                     │
│  - AgentStateStore trait                │
│  - AgentRecord data structure           │
│  - SQL query building                   │
└────────────────┬────────────────────────┘
                 │
┌────────────────▼────────────────────────┐
│  Persistence Layer                      │
│  (sqlite.rs)                            │
│  - SqliteAgentStateStore impl           │
│  - Database initialization              │
│  - CRUD operations                      │
└────────────────┬────────────────────────┘
                 │
┌────────────────▼────────────────────────┐
│  SQLite Database                        │
│  (~/.mofa/agents.db)                    │
│  - agents table                         │
└─────────────────────────────────────────┘
```

### Design Patterns

1. **Trait-Based Abstraction**: `AgentStateStore` trait allows future swappable storage backends (PostgreSQL, MySQL, in-memory, etc.)
2. **Async/Await**: All database operations are async for non-blocking CLI behavior
3. **Error Propagation**: Uses `anyhow::Result` for ergonomic error handling
4. **Table Format**: Structured table output for easy parsing and readability

---

## File Breakdown

### Command Handlers

#### `crates/mofa-cli/src/commands/agent/list.rs`

**Responsibility:** List agents from database with filtering

```rust
pub async fn run_async(running_only: bool, show_all: bool) -> anyhow::Result<()>
```

**Flow:**
1. Get agent store from state management
2. Call `store.list()` to fetch all agents
3. Filter by status if `--running` or `--all` flags provided
4. Convert to `AgentInfo` struct for display
5. Render as JSON table
6. Print formatted output

**Key Fix:** Capture `uptime()` before moving fields into struct to prevent borrow conflicts

#### `crates/mofa-cli/src/commands/agent/start.rs`

**Responsibility:** Create new agent or start existing one

```rust
pub async fn run_async(
    agent_id: &str,
    _config: Option<&std::path::Path>,
    daemon: bool
) -> anyhow::Result<()>
```

**Flow:**
1. Get agent store
2. Try to fetch existing agent by ID
3. If exists and running: return early (idempotent)
4. If exists but stopped: update status
5. If doesn't exist: create new record
6. Set status to `Running` with Unix timestamp
7. Save to database (create vs update)
8. Print success message

**Key Fix:** Use `store.create()` for new agents, `store.update()` for existing ones

**Special Handling:**
```rust
if store.exists(agent_id).await? {
    store.update(record).await?;
} else {
    store.create(record).await?;
}
```

#### `crates/mofa-cli/src/commands/agent/stop.rs`

**Responsibility:** Stop a running agent

```rust
pub async fn run_async(agent_id: &str) -> anyhow::Result<()>
```

**Flow:**
1. Get agent store
2. Fetch agent by ID
3. If not found: return error
4. If already stopped: return early (idempotent)
5. Update status to `Stopped`: clear `started_at`
6. Save to database
7. Print success message

### State Management Layer

#### `crates/mofa-cli/src/state/mod.rs`

**Responsibility:** Provide state store access and initialization

```rust
pub async fn get_agent_store() -> Result<SqliteAgentStateStore>
```

**Key Functions:**
- `get_default_state_dir()` - Get or create `~/.mofa` directory
- `get_default_state_db_path()` - Compute database path
- `get_agent_store()` - Factory function for store

**Environment Variable Support:**
```rust
let mofa_dir = if let Ok(custom_dir) = std::env::var("MOFA_STATE_DIR") {
    PathBuf::from(custom_dir)
} else {
    // Use default: ~/.mofa
}
```

#### `crates/mofa-cli/src/state/store.rs`

**Responsibility:** Define storage contract via trait

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

**AgentRecord Structure:**
```rust
pub struct AgentRecord {
    pub id: String,              // Primary key
    pub name: String,            // Display name
    pub status: AgentStatus,     // Running/Stopped/Paused/Error(String)
    pub started_at: Option<u64>, // Unix timestamp (when started)
    pub provider: Option<String>,// LLM provider
    pub model: Option<String>,   // LLM model
}

impl AgentRecord {
    pub fn uptime(&self) -> Option<String> {
        // Calculate human-readable uptime from started_at
    }
}
```

### Persistence Layer

#### `crates/mofa-cli/src/state/sqlite.rs`

**Responsibility:** Implement AgentStateStore using SQLite

```rust
pub struct SqliteAgentStateStore {
    pool: SqlitePool,
}

impl SqliteAgentStateStore {
    pub async fn new(db_path: PathBuf) -> Result<Self> {
        // Initialize connection pool
        // Create tables if needed
    }
}
```

**Database Schema:**
```sql
CREATE TABLE IF NOT EXISTS agents (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    status TEXT NOT NULL,
    started_at INTEGER,
    provider TEXT,
    model TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
)
```

**Implementation Details:**

- Uses `sqlx` for async SQLite access
- Connection pooling via `SqlitePool`
- Automatic schema creation on initialization
- JSON serialization for status enum

---

## Key Fixes Applied

### Fix 1: Partial Move in List Command

**Problem:**
```rust
AgentInfo {
    id: r.id,           // Moves owned String
    status: r.status.to_string(),
    uptime: r.uptime(), // Error: r partially moved!
}
```

**Solution:** Capture value before move
```rust
let uptime = r.uptime();
AgentInfo {
    id: r.id,
    name: r.name,
    status: r.status.to_string(),
    uptime,  // Use captured value
}
```

### Fix 2: Wrong Method on Agent Creation

**Problem:**
```rust
match store.get(agent_id).await? {
    None => AgentRecord::new(...),  // Create new
}
store.update(record).await?;  // Error: expects existing!
```

**Solution:** Use correct method for new records
```rust
if store.exists(agent_id).await? {
    store.update(record).await?;
} else {
    store.create(record).await?;  // Use create for new
}
```

### Fix 3: Missing Trait Imports

**Problem:**
```rust
let record = store.list().await?;  // Error: method not found!
```

**Cause:** `AgentStateStore` trait not in scope

**Solution:** Add trait import
```rust
use crate::state::{self, AgentStateStore, AgentRecord};
```

---

## Design Decisions

### 1. Trait-Based Storage

**Decision:** Use `AgentStateStore` trait for backend abstraction

**Rationale:**
- Easy to swap SQLite for PostgreSQL/MySQL later
- Enables in-memory implementation for testing
- Follows microkernel architecture from CLAUDE.md
- Decouples CLI from specific database

### 2. Async/Await Throughout

**Decision:** All database operations are async

**Rationale:**
- Non-blocking CLI feels more responsive
- Future support for concurrent agent operations
- Aligns with tokio runtime already in use
- Matches mofa-foundation async patterns

### 3. Idempotent Operations

**Decision:** Starting/stopping already-running/stopped agents succeeds

**Rationale:**
- Safer CLI - repeated commands don't fail
- Natural for automation/scripting
- Aligns with Unix philosophy (idempotent commands)
- Prevents user confusion on reruns

### 4. Status as String in Database

**Decision:** Store AgentStatus enum as TEXT in SQLite

**Rationale:**
- Human-readable database inspection
- Easier schema migrations
- Supports future status variants
- Standard practice for enum storage

---

## Testing Strategy

### Unit Tests
- Mock `AgentStateStore` for command logic
- Test filter logic independently
- Verify error handling paths

### Integration Tests
- Use temporary SQLite database
- Test full command flow with real storage
- Verify state persistence
- Test concurrent operations

### Manual Testing (See CLI_AGENT_COMMANDS_TESTING.md)
- Empty database scenario
- Multiple agent creation
- Status transitions
- Error conditions
- Filtering

---

## Performance Characteristics

### Database Operations
| Operation | Complexity | Estimated Time |
|-----------|-----------|-----------------|
| List agents | O(n) | ~100ms for 100 agents |
| Get agent | O(1) | ~10ms |
| Create agent | O(1) | ~50ms |
| Update agent | O(1) | ~50ms |
| Delete agent | O(1) | ~50ms |

### Optimization Opportunities
1. Add database indexes on `id`, `status` fields
2. Cache agent list (with invalidation)
3. Batch operations for multiple agents
4. Connection pool tuning

---

## Error Handling

### Execution Paths

```
Command Input
  ↓
Validation (agent_id format, flags)
  ↓
Database Operation
  ├─ Success → Format Output → Print → Exit 0
  │
  └─ Error
      ├─ Agent Not Found → Print Error → Exit 1
      ├─ Database Error → Propagate via anyhow → Exit 1
      └─ Permissions Error → Handle gracefully → Exit 1
```

### Error Types (anyhow::Result)
- SQLx errors (database connection, corruption)
- Agent not found (404-style)
- Permission denied (when creating ~/.mofa)
- Invalid parameters (from CLI parser)

---

## Future Extensions

### Planned Enhancements
1. **Session Management** - Link agents to sessions
2. **Agent Metadata** - Store custom fields
3. **Audit Logging** - Track all changes
4. **Metrics** - Uptime, restart count, etc.
5. **Filtering** - By provider, model, creation date
6. **Sorting** - By status, name, uptime
7. **Export** - CSV, JSON output formats
8. **Bulk Operations** - Start/stop multiple agents

### Backend Migrations
1. PostgreSQL support (use `sqlx` generic)
2. MySQL support
3. In-memory store for testing
4. Distributed store (Redis)

---

## Dependencies

```toml
[dependencies]
sqlx = { version = "0.7", features = ["sqlite", "runtime-tokio-native-tls"] }
tokio = { version = "1", features = ["full"] }
anyhow = "1.0"
async-trait = "0.1"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
colored = "2.0"  # Terminal colors
clap = { version = "4.0", features = ["derive"] }  # CLI parsing
```

---

## References

- **CLAUDE.md** - Microkernel architecture guidelines
- **#127** - GitHub issue tracking implementation
- [CLI_AGENT_COMMANDS_TESTING.md](./CLI_AGENT_COMMANDS_TESTING.md) - Comprehensive testing guide
- [CLI_QUICK_REFERENCE.md](./CLI_QUICK_REFERENCE.md) - Quick command reference
