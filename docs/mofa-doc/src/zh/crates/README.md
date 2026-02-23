# Crates

MoFA 工作空间 crate 文档。

## 概述

- **mofa-kernel** — 微内核核心，包含 trait 定义
- **mofa-foundation** — 具体实现和业务逻辑
- **mofa-runtime** — 智能体生命周期和消息总线
- **mofa-plugins** — 插件系统（Rhai、WASM、Rust）
- **mofa-sdk** — 高层用户面向 API
- **mofa-ffi** — 外部函数接口绑定
- **mofa-cli** — 命令行接口工具
- **mofa-macros** — 过程宏
- **mofa-monitoring** — 可观测性和指标
- **mofa-extra** — 额外工具

## 架构

```
mofa-sdk (用户 API)
    ├── mofa-runtime (执行)
    ├── mofa-foundation (实现)
    ├── mofa-kernel (Trait)
    └── mofa-plugins (扩展)
```

## 下一步

探索各个 crate 的文档。
