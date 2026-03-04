# Persistence

MoFA provides built-in persistence for saving agent state, conversation history, and session data.

## Overview

Persistence enables:
- **Session continuity** across restarts
- **Conversation history** for context
- **Agent state recovery** after failures

## Supported Backends

| Backend | Feature Flag | Use Case |
|---------|-------------|----------|
| PostgreSQL | `persistence-postgres` | Production |
| MySQL | `persistence-mysql` | Production |
| SQLite | `persistence-sqlite` | Development/Small scale |
| In-Memory | (default) | Testing |

## Configuration

### PostgreSQL

```toml
[dependencies]
mofa-sdk = { version = "0.1", features = ["persistence-postgres"] }
```

```rust
use mofa_sdk::persistence::PostgresStore;

let store = PostgresStore::connect("postgres://user:pass@localhost/mofa").await?;
```

### SQLite

```toml
[dependencies]
mofa-sdk = { version = "0.1", features = ["persistence-sqlite"] }
```

```rust
use mofa_sdk::persistence::SqliteStore;

let store = SqliteStore::connect("sqlite://mofa.db").await?;
```

## Using Persistence

### With LLMAgent

```rust
use mofa_sdk::persistence::PersistencePlugin;

let persistence = PersistencePlugin::new(
    "persistence",
    store,
    user_id,
    tenant_id,
    agent_id,
    session_id,
);

let agent = LLMAgentBuilder::from_env()?
    .with_persistence_plugin(persistence)
    .with_session_id(session_id.to_string())
    .build_async()
    .await;
```

### Session Management

```rust
// Create new session
let session_id = agent.create_session().await;

// Switch to existing session
agent.switch_session(&session_id).await?;

// List sessions
let sessions = agent.list_sessions().await;

// Delete session
agent.delete_session(&session_id).await?;
```

## Storage Schema

MoFA creates the following tables automatically:

```sql
CREATE TABLE sessions (
    id UUID PRIMARY KEY,
    user_id UUID,
    tenant_id UUID,
    agent_id UUID,
    created_at TIMESTAMP,
    updated_at TIMESTAMP
);

CREATE TABLE messages (
    id UUID PRIMARY KEY,
    session_id UUID REFERENCES sessions(id),
    role VARCHAR(20),
    content TEXT,
    metadata JSONB,
    created_at TIMESTAMP
);

CREATE TABLE agent_state (
    id UUID PRIMARY KEY,
    session_id UUID REFERENCES sessions(id),
    state JSONB,
    created_at TIMESTAMP
);
```

## See Also

- [Feature Flags](../appendix/feature-flags.md) — Persistence features
- [Configuration](../appendix/configuration.md) — Persistence config
