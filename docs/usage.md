## 使用说明

若启用持久化特性
当前优先适配的是postgre，建议使用18版本
uuid_v7特性

---
# 快速开始
## 持久化
examples/streaming_persistence
## 从数据库初始化Agent
examples/agent_from_database_streaming
## 使用TTS插件
examples/llm_tts_streaming

上述示例包含了企业智能体开发最通用的流程：创建会话-配置智能体-发起会话-统计入库


# 创建智能体

MoFA 提供了 `LLMAgentBuilder` 作为 LLM 智能体的建造者模式，支持链式调用配置智能体的各项属性。

## 基本用法

### 1. 最简单的 LLM Agent 创建

```rust
use mofa_sdk::llm::{LLMAgentBuilder, OpenAIProvider};

// 从环境变量读取配置并创建
let agent = LLMAgentBuilder::new()
    .with_provider(std::sync::Arc::new(OpenAIProvider::from_env()))
    .build();
```

### 2. 完整配置示例

```rust
use mofa_sdk::llm::{LLMAgentBuilder, OpenAIProvider};
use std::sync::Arc;
use uuid::Uuid;

let provider = OpenAIProvider::from_env();

let agent = LLMAgentBuilder::new()
    .with_id(Uuid::new_v4().to_string())  // 必须使用 UUID 格式，或省略自动生成
    .with_name("My LLM Agent".to_string())
    .with_provider(Arc::new(provider))
    .with_system_prompt("你是一个乐于助人的AI助手。".to_string())
    .with_temperature(0.7)
    .with_max_tokens(2048)
    .build();
```

### 3. 带工具调用的 Agent

```rust
use mofa_sdk::llm::{LLMAgentBuilder, OpenAIProvider};
use mofa_sdk::create_builtin_tool_plugin;
use mofa_sdk::ToolPluginAdapter;

// 创建内置工具插件（包含 HTTP、文件系统、Shell、计算器等工具）
let mut tool_plugin = create_builtin_tool_plugin("comprehensive_tools")?;
tool_plugin.init_plugin().await?;

// 创建适配器连接到 LLM
let adapter = ToolPluginAdapter::new(tool_plugin);
let tools = adapter.available_tools();
let executor: Arc<dyn LLMToolExecutor> = Arc::new(adapter);

// 构建带工具的 Agent
let agent = LLMAgentBuilder::new()
    .with_name("工具调用助手".to_string())
    .with_provider(Arc::new(OpenAIProvider::from_env()))
    .with_system_prompt("你是一个可以使用工具的AI助手。".to_string())
    .with_tools(tools)
    .with_tool_executor(executor)
    .build();
```

### 4. 带持久化的 Agent

```rust
use mofa_sdk::llm::{LLMAgentBuilder, OpenAIProvider};
use mofa_foundation::persistence::PostgresMessageStore;
use uuid::Uuid;

let agent_id = Uuid::new_v4();
let tenant_id = Uuid::new_v4();

// 创建持久化存储
let message_store = PostgresMessageStore::new("postgresql://...").await?;
let session_store = message_store.clone();

let agent = LLMAgentBuilder::new()
    .with_id(agent_id.to_string())
    .with_provider(Arc::new(OpenAIProvider::from_env()))
    .with_session_id("user_session_123".to_string())
    .with_sliding_window(20)  // 保持最近20轮对话
    .with_persistence_stores(
        message_store,
        session_store,
        Uuid::new_v4(),  // user_id
        tenant_id,
        agent_id
    )
    .build_async()
    .await?;
```

### 5. 带 TTS 插件的 Agent

```rust
use mofa_sdk::llm::{LLMAgentBuilder, OpenAIProvider, TTSPlugin};
use uuid::Uuid;

// 使用 TTS 插件（客户端）
let agent = Arc::new(
LLMAgentBuilder::new()
.with_id(Uuid::new_v4().to_string())
.with_name("Chat TTS Agent")
.with_session_id(Uuid::new_v4().to_string())
.with_provider(Arc::new(openai_from_env()?))
.with_system_prompt("你是一个友好的AI助手。")
.with_temperature(0.7)
.with_plugin(TTSPlugin::with_engine("tts", kokoro_engine, Some("zf_088")))
.build();
);
```

### 6. 带 Rhai 运行时插件的 Agent

```rust
use mofa_sdk::llm::{LLMAgentBuilder, OpenAIProvider};
use mofa_sdk::{RhaiPlugin, RhaiPluginConfig, PluginContext};

// 创建 Rhai 插件（支持热重载）
let config = RhaiPluginConfig::new_file("dynamic_rules", "./rules/plugin.rhai");
let mut rhai_plugin = RhaiPlugin::new(config).await?;

let ctx = PluginContext::new("rules_engine_agent");
rhai_plugin.load(&ctx).await?;
rhai_plugin.init_plugin().await?;
rhai_plugin.start().await?;

let agent = LLMAgentBuilder::new()
    .with_provider(Arc::new(OpenAIProvider::from_env()))
    .with_plugin(rhai_plugin)
    .build();
```

### 7. 多租户场景

```rust
use mofa_sdk::llm::{LLMAgentBuilder, OpenAIProvider};
use uuid::Uuid;

let agent = LLMAgentBuilder::new()
    .with_id(Uuid::new_v4().to_string())
    .with_provider(Arc::new(OpenAIProvider::from_env()))
    .with_user("user_abc".to_string())    // 用户隔离
    .with_tenant("tenant_xyz".to_string()) // 租户隔离
    .build();
```

### 8. 带事件处理

```rust
use mofa_sdk::llm::{LLMAgentBuilder, OpenAIProvider, LLMAgentEventHandler};

struct MyEventHandler;

impl LLMAgentEventHandler for MyEventHandler {
    fn on_message_start(&self, msg: &str) {
        println!("开始处理消息: {}", msg);
    }

    fn on_message_complete(&self, result: &str) {
        println!("消息处理完成: {}", result);
    }
}

let agent = LLMAgentBuilder::new()
    .with_provider(Arc::new(OpenAIProvider::from_env()))
    .with_event_handler(Box::new(MyEventHandler))
    .build();
```

### 9. 使用热重载提示词模板

```rust
use mofa_sdk::llm::{LLMAgentBuilder, OpenAIProvider, HotReloadableRhaiPromptPlugin};

// 支持运行时动态修改提示词，无需重启
let prompt_plugin = HotReloadableRhaiPromptPlugin::new("./prompts/template.rhai")?;

let agent = LLMAgentBuilder::new()
    .with_provider(Arc::new(OpenAIProvider::from_env()))
    .with_prompt_plugin(prompt_plugin)
    .build();
```

## LLMAgentBuilder 方法说明

### 核心配置

| 方法 | 参数 | 说明 | 默认值 |
|------|------|------|--------|
| `new()` | - | 创建新的 Builder 实例 | - |
| `with_id()` | `id: String` | 设置智能体 ID（仅支持 UUID 格式） | 自动生成 UUID v7 |
| `with_name()` | `name: String` | 设置智能体名称 | - |
| `with_provider()` | `provider: Arc<dyn LLMProvider>` | 设置 LLM 提供商 | **必须设置** |
| `with_system_prompt()` | `prompt: String` | 设置系统提示词 | - |
| `with_temperature()` | `temperature: f32` | 设置温度参数 (0.0-1.0) | - |
| `with_max_tokens()` | `max_tokens: u32` | 设置最大输出 token 数 | - |

### 工具和执行

| 方法 | 参数 | 说明 | 默认值 |
|------|------|------|--------|
| `with_tool()` | `tool: Tool` | 添加单个工具 | - |
| `with_tools()` | `tools: Vec<Tool>` | 批量添加工具 | - |
| `with_tool_executor()` | `executor: Arc<dyn ToolExecutor>` | 设置工具执行器 | - |

### 插件系统

| 方法 | 参数 | 说明 |
|------|------|------|
| `with_plugin()` | `plugin: AgentPlugin` | 添加单个插件 |
| `with_plugins()` | `plugins: Vec<Box<dyn AgentPlugin>>` | 批量添加插件 |
| `with_tts_engine()` | `tts_engine: TTSPlugin` | 设置 TTS 插件 |
| `with_prompt_plugin()` | `plugin: PromptTemplatePlugin` | 设置提示词模板插件 |
| `with_hot_reload_prompt_plugin()` | `plugin: HotReloadableRhaiPromptPlugin` | 设置热重载提示词插件 |

### 事件和持久化

| 方法 | 参数 | 说明 |
|------|------|------|
| `with_event_handler()` | `handler: Box<dyn LLMAgentEventHandler>` | 设置事件处理器 |
| `with_persistence_handler()` | `persistence: Arc<dyn PersistenceCallback>` | 设置持久化处理器 |
| `with_persistence_stores()` | `message_store, session_store, user_id, tenant_id, agent_id` | 设置持久化存储 |

### 会话管理

| 方法 | 参数 | 说明 | 默认值 |
|------|------|------|--------|
| `with_session_id()` | `session_id: String` | 设置会话 ID | - |
| `with_sliding_window()` | `size: usize` | 设置上下文窗口大小（轮次） | - |

### 多租户

| 方法 | 参数 | 说明 |
|------|------|------|
| `with_user()` | `user_id: String` | 设置用户 ID |
| `with_tenant()` | `tenant_id: String` | 设置租户 ID |

### 配置辅助

| 方法 | 参数 | 说明 |
|------|------|------|
| `with_config()` | `key: String, value: String` | 添加自定义配置 |
| `from_env()` | - | 从环境变量创建配置 |

### 构建方法

| 方法 | 说明 |
|------|------|
| `build()` | 同步构建（Provider 未设置会 panic） |
| `try_build()` | 同步构建，返回 Result |
| `build_async()` | 异步构建（支持从数据库加载） |

---

# uniffi
生成python绑定
cd crates/mofa-sdk                                                                                                                                                   
./generate-bindings.sh python 
