# 第 2 章：环境搭建

> **学习目标：** 克隆仓库，构建工作区，设置 LLM 提供者，并通过运行示例验证一切正常。

## 安装 Rust

MoFA 需要 Rust **1.85 或更高版本**（edition 2024）。通过 [rustup](https://rustup.rs/) 安装：

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

验证版本：

```bash
rustc --version
# 应显示 1.85.0 或更高版本
```

如果你已经安装了 Rust，更新它：

```bash
rustup update
```

## 克隆和构建

```bash
git clone https://github.com/moxin-org/mofa.git
cd mofa
git checkout feature/mofa-rs
```

构建整个工作区：

```bash
cargo build
```

> **Rust 提示：Cargo 工作区**
> MoFA 是一个 Cargo 工作区——一组共享 `Cargo.lock` 和输出目录的相关 crate（包）。当你在根目录运行 `cargo build` 时，它会构建所有 10 个 crate。你可以用 `cargo build -p mofa-sdk` 构建单个 crate。

首次构建需要几分钟来下载和编译依赖。后续构建由于增量编译会快很多。

## IDE 设置

推荐使用 **VS Code** 配合 [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer) 扩展：

1. 安装 VS Code
2. 安装 `rust-analyzer` 扩展
3. 在 VS Code 中打开 `mofa/` 文件夹
4. 等待 rust-analyzer 完成索引（观察状态栏）

rust-analyzer 提供自动补全、跳转到定义、内联类型提示和错误检查——这些都是浏览 MoFA 代码库的必备功能。

## 设置 LLM 提供者

第 4 章及之后的章节需要至少一个 LLM 提供者。选择一个：

### 选项 A：OpenAI（云端，需要 API 密钥）

1. 从 [platform.openai.com](https://platform.openai.com/) 获取 API 密钥
2. 设置环境变量：

```bash
export OPENAI_API_KEY="sk-your-key-here"
```

将此添加到你的 shell 配置文件（`~/.bashrc`、`~/.zshrc` 等）以使其持久化。

### 选项 B：Ollama（本地运行，免费，无需 API 密钥）

1. 从 [ollama.ai](https://ollama.ai/) 安装 Ollama
2. 拉取模型：

```bash
ollama pull llama3.2
```

3. Ollama 默认运行在 `http://localhost:11434`——不需要环境变量。

> **应该选哪个？** Ollama 非常适合开发——免费且在本地运行。OpenAI 在复杂任务上效果更好。你可以两个都用；MoFA 使切换提供者变得很容易。

## 验证：运行示例

让我们通过运行 `chat_stream` 示例来验证你的设置：

```bash
# 使用 OpenAI
cd examples/chat_stream
cargo run

# 使用 Ollama（你需要修改提供者——参见第 4 章）
```

你应该能看到智能体以流式输出回应提示。按 `Ctrl+C` 退出。

如果你还没有 API 密钥，仍然可以验证构建是否正常：

```bash
cargo check
```

这会编译所有 crate 但不生成二进制文件——比 `cargo build` 更快，并确认没有编译错误。

## 运行测试

验证测试套件通过：

```bash
cargo test
```

或测试特定 crate：

```bash
cargo test -p mofa-sdk
```

## 项目结构一览

现在你已经有了代码，花点时间看看项目结构：

```
mofa/
├── Cargo.toml              # 工作区根目录——列出所有 crate
├── crates/
│   ├── mofa-kernel/        # Trait 和核心类型（从这里开始理解 API）
│   ├── mofa-foundation/    # 具体实现（LLM、智能体、持久化）
│   ├── mofa-runtime/       # 智能体生命周期、运行器、注册表
│   ├── mofa-plugins/       # Rhai、WASM、热重载、内置工具
│   ├── mofa-sdk/           # 统一 API——你在代码中导入的内容
│   ├── mofa-cli/           # `mofa` CLI 工具
│   ├── mofa-ffi/           # 跨语言绑定
│   ├── mofa-monitoring/    # 仪表板、指标、链路追踪
│   ├── mofa-extra/         # Rhai 引擎、规则引擎
│   └── mofa-macros/        # 过程宏
├── examples/               # 27+ 可运行示例
└── docs/                   # 文档（你在这里）
```

> **架构说明：** 浏览代码时，先从 `mofa-kernel` 开始理解 trait 契约，然后查看 `mofa-foundation` 了解它们是如何实现的。`mofa-sdk` crate 将所有内容重新导出为清晰的公共 API。

## 故障排除

**构建失败，提示"edition 2024 is not supported"**
→ 你的 Rust 版本太旧。运行 `rustup update` 获取 1.85+。

**缺少系统依赖（Linux）**
→ 安装开发包：`sudo apt install pkg-config libssl-dev`（Ubuntu/Debian）。

**首次构建很慢**
→ 这是正常的。后续构建会快很多。使用 `cargo check` 进行快速迭代。

**rust-analyzer 显示错误但 `cargo build` 正常**
→ 重启 rust-analyzer（Ctrl+Shift+P → "rust-analyzer: Restart Server"）。它有时需要重新索引。

## 关键要点

- MoFA 需要 Rust 1.85+（edition 2024）
- `cargo build` 构建整个工作区；`cargo build -p <crate>` 构建单个 crate
- LLM 章节需要 OpenAI API 密钥或 Ollama
- `examples/` 目录包含 27+ 可运行示例
- 从 `mofa-kernel`（trait）→ `mofa-foundation`（实现）开始探索代码

---

**下一章：** [第 3 章：你的第一个智能体](03-first-agent.md) — 从零实现 `MoFAAgent` trait。

[← 返回目录](README.md)

---

[English](../../tutorial/02-setup.md) | **简体中文**
