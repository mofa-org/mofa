# 第 8 章：插件与脚本

> **学习目标：** 理解 `AgentPlugin` trait 的生命周期，编写 Rhai 脚本插件，启用热重载，并了解何时使用编译时插件与运行时插件。

## 双层插件系统

如第 1 章所介绍，MoFA 有两个插件层：

| 层 | 语言 | 使用场景 |
|----|------|----------|
| **编译时** | Rust / WASM | 性能关键路径：LLM 适配器、数据处理、原生 API |
| **运行时** | Rhai 脚本 | 业务逻辑、内容过滤器、规则引擎，以及任何频繁变化的内容 |

两层都实现相同的 `AgentPlugin` trait，因此系统统一管理它们。

## AgentPlugin Trait

每个插件遵循明确定义的生命周期：

```rust
// crates/mofa-kernel/src/plugin/mod.rs

#[async_trait]
pub trait AgentPlugin: Send + Sync {
    fn metadata(&self) -> &PluginMetadata;
    fn state(&self) -> PluginState;

    // 生命周期方法——按此顺序调用：
    async fn load(&mut self, ctx: &PluginContext) -> PluginResult<()>;
    async fn init_plugin(&mut self) -> PluginResult<()>;
    async fn start(&mut self) -> PluginResult<()>;
    async fn pause(&mut self) -> PluginResult<()>;   // 可选
    async fn resume(&mut self) -> PluginResult<()>;  // 可选
    async fn stop(&mut self) -> PluginResult<()>;
    async fn unload(&mut self) -> PluginResult<()>;

    // 主要执行
    async fn execute(&mut self, input: String) -> PluginResult<String>;
    async fn health_check(&self) -> PluginResult<bool>;
}
```

生命周期进程：

```
load → init_plugin → start → [execute...] → stop → unload
                       ↕
                  pause / resume
```

### PluginMetadata

每个插件声明其身份和能力：

```rust
pub struct PluginMetadata {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub plugin_type: PluginType,
    pub priority: PluginPriority,
    pub dependencies: Vec<String>,
    pub capabilities: Vec<String>,
}
```

插件类型包括：

```rust
pub enum PluginType {
    LLM,       // LLM 提供者适配器
    Tool,      // 工具实现
    Storage,   // 持久化后端
    Memory,    // 记忆实现
    Scripting, // 脚本引擎（Rhai 等）
    Skill,     // 技能包
    Custom(String),
}
```

## Rhai：运行时脚本引擎

[Rhai](https://rhai.rs/) 是一种为 Rust 设计的轻量级、快速的嵌入式脚本语言。MoFA 使用它作为运行时插件，因为：

- **支持热重载**：更改脚本，立即看到结果（无需重新编译）
- **沙箱化**：脚本无法访问文件系统或网络，除非你明确允许
- **对 Rust 友好**：容易在 Rhai 和 Rust 之间互相调用函数
- **快速**：编译为字节码，比解释型语言快很多

### Rhai 基本语法

```javascript
// 变量
let x = 42;
let name = "MoFA";

// 函数
fn greet(name) {
    "你好，" + name + "！"
}

// 条件
if x > 40 {
    print("x 很大");
} else {
    print("x 很小");
}

// 对象（映射）
let config = #{
    max_retries: 3,
    timeout: 30,
    enabled: true
};

// JSON 处理（内置）
let data = parse_json(input);
let result = #{
    processed: true,
    original: data
};
to_json(result)
```

## 构建：支持热重载的内容过滤器

让我们构建一个 Rhai 插件，基于可在运行时更新的规则过滤内容，无需重启应用。

创建新项目：

```bash
cargo new content_filter
cd content_filter
mkdir -p plugins
```

首先，创建 Rhai 脚本。编写 `plugins/content_filter.rhai`：

```javascript
// 内容过滤规则——编辑此文件，插件会自动重新加载！

// 屏蔽词列表
let blocked_words = ["spam", "scam", "phishing"];

// 处理输入
fn process(input) {
    let text = input.to_lower();
    let issues = [];

    // 检查屏蔽词
    for word in blocked_words {
        if text.contains(word) {
            issues.push("包含屏蔽词: " + word);
        }
    }

    // 检查文本长度
    if input.len() > 1000 {
        issues.push("文本超过 1000 字符限制");
    }

    // 检查过多大写字母（喊叫）
    let upper_count = 0;
    for ch in input.chars() {
        if ch >= 'A' && ch <= 'Z' {
            upper_count += 1;
        }
    }
    if input.len() > 10 && upper_count * 100 / input.len() > 70 {
        issues.push("大写字母过多（可能是喊叫）");
    }

    // 构建结果
    if issues.is_empty() {
        to_json(#{
            status: "approved",
            message: "内容通过所有检查"
        })
    } else {
        to_json(#{
            status: "rejected",
            issues: issues,
            message: "内容未通过 " + issues.len() + " 项检查"
        })
    }
}

// 入口点——由插件系统调用
process(input)
```

编写 `Cargo.toml`：

```toml
[package]
name = "content_filter"
version = "0.1.0"
edition = "2024"

[dependencies]
mofa-sdk = { path = "../../crates/mofa-sdk" }
mofa-plugins = { path = "../../crates/mofa-plugins" }
mofa-kernel = { path = "../../crates/mofa-kernel" }
tokio = { version = "1", features = ["full"] }
serde_json = "1"
```

编写 `src/main.rs`：

```rust
use mofa_kernel::plugin::PluginContext;
use mofa_plugins::rhai_runtime::{RhaiPlugin, RhaiPluginConfig};
use std::path::Path;
use tokio::time;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let plugin_path = Path::new("plugins/content_filter.rhai");

    // --- 第 1 步：创建并初始化 Rhai 插件 ---
    let config = RhaiPluginConfig::new_file("content_filter", plugin_path);
    let mut plugin = RhaiPlugin::new(config).await?;

    let ctx = PluginContext::new("tutorial_agent");
    plugin.load(&ctx).await?;
    plugin.init_plugin().await?;
    plugin.start().await?;

    println!("内容过滤插件已加载并启动！\n");

    // --- 第 2 步：用各种输入测试 ---
    let test_inputs = vec![
        "你好，这是一条关于 Rust 编程的正常消息。",
        "CLICK HERE FOR FREE MONEY! This is totally not a scam!",
        "Buy our product! No spam involved, we promise.",
        "THIS IS ALL CAPS AND VERY SHOUTY MESSAGE HERE!!!",
        "一条简短友好的留言。",
    ];

    for input in &test_inputs {
        let result = plugin.execute(input.to_string()).await?;
        let parsed: serde_json::Value = serde_json::from_str(&result)?;
        println!("输入:  \"{}\"", &input[..input.len().min(50)]);
        println!("结果: {} — {}\n",
            parsed["status"].as_str().unwrap_or("?"),
            parsed["message"].as_str().unwrap_or("?"),
        );
    }

    // --- 第 3 步：热重载演示 ---
    println!("=== 热重载演示 ===");
    println!("修改 plugins/content_filter.rhai 观察输出变化！");
    println!("按 Ctrl+C 停止。\n");

    // 轮询变化并重新执行
    let test_message = "Check this spam content for compliance.";
    let mut last_modified = std::fs::metadata(plugin_path)?.modified()?;

    for i in 1..=30 {
        // 检查文件是否被修改
        let current_modified = std::fs::metadata(plugin_path)?.modified()?;
        if current_modified != last_modified {
            println!("  [重载] 脚本已更改，正在重新加载...");

            // 重载插件
            plugin.stop().await?;
            plugin.unload().await?;

            let config = RhaiPluginConfig::new_file("content_filter", plugin_path);
            plugin = RhaiPlugin::new(config).await?;
            plugin.load(&ctx).await?;
            plugin.init_plugin().await?;
            plugin.start().await?;

            last_modified = current_modified;
            println!("  [重载] 完成！");
        }

        let result = plugin.execute(test_message.to_string()).await?;
        println!("  [{}] {}", i, result);

        time::sleep(time::Duration::from_secs(2)).await;
    }

    // --- 清理 ---
    plugin.stop().await?;
    plugin.unload().await?;

    Ok(())
}
```

运行它：

```bash
cargo run
```

在运行期间，尝试编辑 `plugins/content_filter.rhai`——例如，将 "compliance" 添加到 `blocked_words` 列表中。插件将重新加载，输出将改变。

## 刚才发生了什么？

1. **`RhaiPluginConfig::new_file()`** — 将插件指向一个 Rhai 脚本文件
2. **`RhaiPlugin::new(config)`** — 创建插件（编译脚本）
3. **生命周期**：`load → init_plugin → start` 准备插件执行
4. **`plugin.execute(input)`** — 以 `input` 作为变量运行 Rhai 脚本
5. **热重载**：我们检测文件更改并重新创建插件，重新编译脚本

> **架构说明：** `RhaiPlugin` 位于 `mofa-plugins`（`crates/mofa-plugins/src/rhai_runtime/plugin.rs`）。底层 Rhai 引擎在 `mofa-extra`（`crates/mofa-extra/src/rhai/`）。`AgentPlugin` trait 在 `mofa-kernel` 中。这遵循了架构规则：内核定义接口，插件提供实现。

## 插件管理器

在实际应用中，你会使用 `PluginManager` 来处理多个插件：

```rust
use mofa_sdk::plugins::PluginManager;

let mut manager = PluginManager::new();

// 注册插件
manager.register(Box::new(content_filter_plugin));
manager.register(Box::new(analytics_plugin));
manager.register(Box::new(logging_plugin));

// 初始化所有插件
manager.init_all().await?;

// 启动所有插件
manager.start_all().await?;

// 执行特定插件
let result = manager.execute("content_filter", input).await?;
```

## 将插件集成到 LLMAgent

插件可以通过构建器附加到 `LLMAgent`：

```rust
let agent = LLMAgentBuilder::new()
    .with_provider(provider)
    .with_plugin(content_filter_plugin)
    .with_plugin(analytics_plugin)
    .build();
```

智能体在生命周期中调用插件钩子——例如，`before_chat` 和 `after_chat` 事件让插件拦截和修改消息。

## WASM 插件（高级）

对于需要动态加载的性能关键型插件，MoFA 支持 WASM：

```rust
use mofa_sdk::plugins::WasmPlugin;

// 加载编译好的 WASM 模块
let plugin = WasmPlugin::from_file("plugins/my_plugin.wasm").await?;
```

WASM 插件从 Rust（或任何可以编译到 WASM 的语言）编译，在沙箱环境中运行。它们比 Rhai 脚本快，但更改时需要重新编译。

> **何时使用哪种？**
> - **Rhai**：业务规则、内容过滤器、工作流逻辑——任何频繁变化且不需要极致性能的内容
> - **WASM**：数据处理、加密、压缩——需要接近原生速度的计算密集型任务
> - **原生 Rust**：LLM 提供者、数据库适配器、核心基础设施——很少更改且需要完整 Rust 生态的内容

## 关键要点

- `AgentPlugin` 定义了生命周期：`load → init → start → execute → stop → unload`
- 插件有元数据（id、name、type、priority、dependencies）
- Rhai 脚本是运行时插件层——支持热重载、沙箱化、快速执行
- 热重载：检测文件更改，停止旧插件，从更新的脚本创建新插件
- `PluginManager` 在实际应用中处理多个插件
- WASM 插件提供动态加载和接近原生的性能
- 选择 Rhai 获得灵活性，WASM 获得性能，原生 Rust 用于基础设施

---

**下一章：** [第 9 章：下一步](09-whats-next_cn.md) — 贡献、GSoC 想法和高级主题。

[← 返回目录](README_cn.md)

---

**English** | [简体中文](../zh-CN/tutorial/08-plugins-and-scripting_cn.md)
