# 实现计划：RSS 转多主播新闻稿数据流

**分支**: `001-rss-newscaster-script` | **日期**: 2026-01-09 | **规格**: [spec.md](./spec.md)
**输入**: 来自 `/specs/001-rss-newscaster-script/spec.md` 的功能规格说明

**说明**: 此模板由 `/speckit.plan` 命令填写。执行工作流请参见 `.specify/templates/commands/plan.md`。

## 概述

构建一个 MoFA 数据流，将 RSS 订阅内容转换为适合三位不同主播角色（男主播、女主播和资深评论员）的可播出稿件。该数据流遵循 MoFA 基于 YAML 的声明式模式，包含三个主要节点：RSS 输入、新闻处理和基于 LLM 的稿件生成。

## 技术上下文

**语言/版本**: Python 3.10+  
**主要依赖**: MoFA 框架、dora-rs、pyarrow、feedparser（RSS 解析）、openai（LLM 客户端）  
**存储**: 不适用（无状态数据流，文本输出）  
**测试**: pytest 配合可运行的数据流示例  
**目标平台**: Linux、macOS、WSL2（MoFA 支持的平台）  
**项目类型**: 包含多个 Agent 的 MoFA 数据流  
**性能目标**: 在 2 分钟内处理包含最多 20 个项目的 RSS 订阅（根据 SC-001）  
**约束**: 仅文本输出（此数据流不生成音频）  
**规模/范围**: 每个 RSS 订阅 1-50 条新闻（根据 FR-007）

## 宪法检查

*关卡：必须在第 0 阶段研究前通过。第 1 阶段设计后重新检查。*

| 原则 | 合规性 | 备注 |
|------|--------|------|
| 一、可组合的 AI 架构 | ✅ 通过 | 三个自包含的 Agent，具有清晰的输入/输出：rss-input → news-processor → script-generator |
| 二、数据流优先 | ✅ 通过 | 声明式 YAML 数据流定义，通过 pyarrow 显式传递数据，无隐藏状态 |
| 三、一切皆 Agent | ✅ 通过 | RSS 获取器、新闻处理器和 LLM 稿件生成器都实现为 MoFA Agent |
| 四、可访问性与简洁性 | ✅ 通过 | Python 优先，YAML 配置，包含可运行示例 |
| 五、模块化与可扩展设计 | ✅ 通过 | 每个 Agent 可插拔，主播特征可通过参数配置 |
| 六、平台务实主义 | ✅ 通过 | Python 3.10+，跨平台（Linux/macOS/WSL2） |

**开发标准合规性**:
- 新 Agent 将包含文档更新
- 每个 Agent 将包含可运行的数据流示例
- CLI 命令将提供带有清晰描述的 `--help`
- 配置文件将包含内联注释

**质量与测试合规性**:
- 新 Agent 将包含可运行的数据流示例（根据宪法）
- 集成测试将覆盖完整的 RSS → 稿件流水线

## 项目结构

### 文档（本功能）

```text
specs/001-rss-newscaster-script/
├── plan.md              # 本文件（/speckit.plan 命令输出）
├── research.md          # 第 0 阶段输出（/speckit.plan 命令）
├── data-model.md        # 第 1 阶段输出（/speckit.plan 命令）
├── quickstart.md        # 第 1 阶段输出（/speckit.plan 命令）
├── contracts/           # 第 1 阶段输出（/speckit.plan 命令）
└── tasks.md             # 第 2 阶段输出（/speckit.tasks 命令 - 不由 /speckit.plan 创建）
```

### 源代码（仓库根目录）

```text
# MoFA 数据流结构（遵循现有模式如 podcast-generator）

agents/
├── rss-input/                    # 节点 1：RSS URL 输入 Agent
│   ├── pyproject.toml
│   ├── README.md
│   ├── rss_input/
│   │   ├── __init__.py
│   │   └── main.py
│   └── tests/
│       └── test_main.py
├── news-processor/               # 节点 2：RSS 获取和解析 Agent
│   ├── pyproject.toml
│   ├── README.md
│   ├── news_processor/
│   │   ├── __init__.py
│   │   └── main.py
│   └── tests/
│       └── test_main.py
└── script-generator/             # 节点 3：基于 LLM 的稿件生成 Agent
    ├── pyproject.toml
    ├── README.md
    ├── script_generator/
    │   ├── __init__.py
    │   └── main.py
    └── tests/
        └── test_main.py

flows/
└── rss-newscaster/
    ├── rss_newscaster_dataflow.yml   # 主数据流定义
    └── README.md
```

**结构决策**: 包含三个 Agent 的 MoFA 数据流结构，遵循仓库中的现有模式（类似于 `podcast-generator` 流）。每个 Agent 是一个独立的 Python 包，拥有自己的 `pyproject.toml`、源模块和测试。

## 复杂度跟踪

> **仅在宪法检查有需要说明的违规时填写**

无违规。所有原则都通过提议的设计满足。
