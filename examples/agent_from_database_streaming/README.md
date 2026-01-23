# 从数据库加载 Agent 配置 - 流式对话示例

本示例展示了如何从 PostgreSQL 数据库加载 Agent 配置，并使用流式对话功能与用户交互。

## 功能特点

- 从数据库读取 Agent 配置（包括 provider、模型、系统提示词等）
- 从数据库读取 Provider 配置（API 地址、密钥等）
- 流式输出 LLM 响应
- 持久化会话和消息到数据库
- 上下文窗口管理（滑动窗口）
- 多租户支持

## 前置准备

### 1. 初始化数据库

```bash
psql -d your-database -f ../../scripts/sql/migrations/postgres_init.sql
```

### 2. 插入测试数据（可选）

```sql
-- 插入 Provider
INSERT INTO entity_provider (id, tenant_id, provider_name, provider_type, api_base, api_key, enabled, create_time, update_time)
VALUES (
    '550e8400-e29b-41d4-a716-446655440001',
    '00000000-0000-0000-0000-000000000000',
    'openai-provider',
    'openai',
    'https://api.openai.com/v1',
    'your-api-key-here',
    true,
    NOW(),
    NOW()
);

-- 插入 Agent
INSERT INTO entity_agent (
    id, tenant_id, agent_code, agent_name, agent_order, agent_status,
    model_name, provider_id, system_prompt, temperature, stream,
    context_limit, create_time, update_time
) VALUES (
    '550e8400-e29b-41d4-a716-446655440002',
    '00000000-0000-0000-0000-000000000000',
    'chat-assistant',
    '聊天助手',
    1,
    true,
    'gpt-4o-mini',
    '550e8400-e29b-41d4-a716-446655440001',
    '你是一个友好且专业的 AI 助手，能够帮助用户解答问题和完成任务。',
    0.7,
    true,
    10,
    NOW(),
    NOW()
);
```

## 环境变量

| 环境变量 | 必需 | 说明 | 示例 |
|---------|------|------|------|
| `DATABASE_URL` | 是 | PostgreSQL 数据库连接字符串 | `postgres://postgres:password@localhost:5432/mofa` |
| `AGENT_CODE` | 否 | Agent 代码（默认: chat-assistant） | `chat-assistant` |
| `USER_ID` | 是 | 用户 ID（UUID 格式） | `550e8400-e29b-41d4-a716-446655440003` |
| `TENANT_ID` | 否 | 租户 ID（默认: 00000000-0000-0000-0000-000000000000） | `00000000-0000-0000-0000-000000000000` |
| `OPENAI_API_KEY` | 条件 | OpenAI API 密钥（如果数据库中 provider 已配置则不需要） | `sk-xxx` |

## 运行示例

```bash
# 设置环境变量
export DATABASE_URL="postgres://postgres:password@localhost:5432/mofa"
export AGENT_CODE="chat-assistant"
export USER_ID="550e8400-e29b-41d4-a716-446655440003"
export OPENAI_API_KEY="sk-xxx"

# 运行示例
cargo run --release
```

## 使用方法

1. 程序会连接到数据库并加载指定 Agent 的配置
2. 显示 Agent 的配置信息（ID、名称、系统提示词等）
3. 进入交互式对话模式
4. 输入消息后会流式输出 AI 的响应
5. 每轮对话后会显示当前上下文状态
6. 输入 `quit` 或 `exit` 退出程序

## 代码说明

### 核心步骤

1. **连接数据库**: 使用 `PostgresStore::connect()` 建立数据库连接
2. **加载 Agent 配置**: 使用 `LLMAgentBuilder::from_database_with_tenant()` 从数据库加载
3. **配置持久化**: 设置消息存储和会话存储
4. **构建 Agent**: 调用 `build_async()` 异步构建 Agent
5. **流式对话**: 使用 `chat_stream()` 进行流式对话

### 关键 API

```rust
// 从数据库加载 Agent 配置
let mut agent_builder = LLMAgentBuilder::from_database_with_tenant(
    &store,
    tenant_id,
    &agent_code
).await?;

// 设置持久化存储
agent_builder = agent_builder
    .with_persistence_stores(store.clone(), store, user_id, tenant_id, agent_id)
    .with_persistence_handler(persistence);

// 构建Agent
let agent = agent_builder.build_async().await;

// 流式对话
let mut stream = agent.chat_stream(&user_input).await?;
while let Some(result) = stream.next().await {
    match result {
        Ok(text) => print!("{}", text),
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

## 数据库表结构

### entity_provider (Provider 配置表)

| 字段 | 类型 | 说明 |
|------|------|------|
| id | uuid | 主键 |
| tenant_id | uuid | 租户 ID |
| provider_name | varchar | Provider 名称 |
| provider_type | varchar | Provider 类型 (openai, azure, ollama 等) |
| api_base | varchar | API 地址 |
| api_key | varchar | API 密钥 |
| enabled | boolean | 是否启用 |

### entity_agent (Agent 配置表)

| 字段 | 类型 | 说明 |
|------|------|------|
| id | uuid | 主键 |
| tenant_id | uuid | 租户 ID |
| agent_code | varchar | Agent 代码（唯一标识） |
| agent_name | varchar | Agent 名称 |
| agent_status | boolean | 是否启用 |
| model_name | varchar | 模型名称 |
| provider_id | uuid | 关联的 Provider ID |
| system_prompt | text | 系统提示词 |
| temperature | float | 温度参数 |
| stream | boolean | 是否流式输出 |
| context_limit | integer | 上下文窗口大小（轮数） |

### entity_chat_session (会话表)

存储会话信息。

### entity_llm_message (消息表)

存储对话消息。

## 与其他示例的区别

| 示例 | Agent 配置来源 | 用途 |
|------|---------------|------|
| `react_agent` | 代码硬编码 | 基础 ReAct Agent |
| `streaming_persistence` | 代码硬编码 + 持久化 | 流式对话 + 持久化 |
| **agent_from_database_streaming** | **数据库** | **从数据库加载配置 + 流式对话** |


附：
创建Agent配置

INSERT INTO entity_agent
(id, tenant_id, agent_code, agent_name, agent_order, agent_status, context_limit, custom_params, max_completion_tokens, model_name, provider_id, response_format, system_prompt, temperature, stream, thinking, create_time, update_time)
VALUES(uuidv7(), '019aaa58-532f-7db0-95a8-e3786da68762'::uuid, 'chat-assistant', '聊天助手', 0, true, 1, NULL, 16000, 'Qwen/Qwen3-8B', '019b8c0c-6181-7ca0-8f4a-e23397395907', 'text', '你是一个友好的智能助手', 0.7, true, NULL, now(), now());
