# 持久化

MoFA 提供内置的持久化功能，用于保存智能体状态、对话历史和会话数据。

## 概述

持久化功能支持：
- **会话连续性** — 跨重启保持会话
- **对话历史** — 保存上下文
- **智能体状态恢复** — 故障后恢复

## 支持的后端

| 后端 | 功能标志 | 使用场景 |
|------|----------|----------|
| PostgreSQL | `persistence-postgres` | 生产环境 |
| MySQL | `persistence-mysql` | 生产环境 |
| SQLite | `persistence-sqlite` | 开发/小规模 |
| 内存 | (默认) | 测试 |

## 配置

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

## 使用持久化

### 配合 LLMAgent

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

### 会话管理

```rust
// 创建新会话
let session_id = agent.create_session().await;

// 切换到现有会话
agent.switch_session(&session_id).await?;

// 列出会话
let sessions = agent.list_sessions().await;

// 删除会话
agent.delete_session(&session_id).await?;
```

## 存储架构

MoFA 自动创建以下表：

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

## 相关链接

- [功能标志](../appendix/feature-flags.md) — 持久化功能
- [配置](../appendix/configuration.md) — 持久化配置
