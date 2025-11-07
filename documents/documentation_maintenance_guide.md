# MoFA Documentation Maintenance Guide

[English](#english) | [简体中文](#chinese)

---

<a name="english"></a>
## English Version

### 1. Overview

This guide helps maintainers keep MoFA documentation synchronized across three platforms:
1. **Root README** (`/README.md`, `/README_en.md`) - Quick start and overview
2. **Detailed Docs** (`/documents/`) - In-depth guides and examples
3. **Official Website** (mofa.ai) - Tutorials and structured learning paths

### 2. Documentation Structure

```
mofa/
├── README.md                           # Chinese quick start (500-800 lines)
├── README_en.md                        # English quick start (500-800 lines)
├── debug.md                            # Debug guide
└── documents/
    ├── README.md                       # Documents index (English)
    ├── README_cn.md                    # Documents index (Chinese)
    ├── test_plan.md                    # Testing strategy
    ├── documentation_maintenance_guide.md  # This file
    ├── build_agent/
    │   └── build_agent.md              # Agent building tutorial
    ├── examples/
    │   ├── hello_world.md              # Hello World example (English)
    │   ├── hello_world_cn.md           # Hello World example (Chinese)
    │   └── ...                         # Other examples
    ├── archive/
    │   ├── README_v0.1_en.md           # Archived version docs
    │   └── README_v0.1_cn.md
    └── images/                         # Documentation images
```

### 3. Three-Tier Documentation Strategy

#### **Tier 1: Root README (Gateway)**
**Purpose**: First impression, quick start in <5 minutes

**Content Guidelines**:
- Design philosophy (4-6 bullet points)
- Installation (3 methods max)
- Quick start example (hello world in <10 lines)
- Core CLI commands table (8-12 commands)
- Support matrix table
- Links to detailed docs and website

**Update Triggers**:
- ✅ New CLI command added
- ✅ Installation method changed
- ✅ Support matrix updated (new Python version, OS support)
- ✅ Core features added/removed
- ❌ Minor bug fixes (no update needed)
- ❌ Internal refactoring (no update needed)

**Review Frequency**: Every release (before version bump)

---

#### **Tier 2: Detailed Docs (`/documents/`)**
**Purpose**: Comprehensive guides for users who need depth

**Content Categories**:

| Category | Files | Update Trigger |
|----------|-------|----------------|
| **Agent Building** | `build_agent/build_agent.md` | Agent template changes, MofaAgent API changes |
| **Examples** | `examples/hello_world*.md`, etc. | Example code changes, dataflow structure changes |
| **Architecture** | `README.md`, `README_cn.md` | Design pattern changes, new templates added |
| **Testing** | `test_plan.md` | Test strategy changes (quarterly review) |
| **Archive** | `archive/README_v*.md` | Major version releases (snapshot before breaking changes) |

**Update Triggers**:
- ✅ CLI command behavior changed (e.g., `run-flow` now supports `--timeout`)
- ✅ TUI interface redesigned (e.g., `mofa list` layout changed)
- ✅ Dataflow start/stop process simplified
- ✅ New agent template added
- ✅ Configuration file format changed (e.g., `agent.yml` schema update)
- ✅ Screenshots outdated (UI changed)

**Review Frequency**: Monthly + on-demand when major features land

---

#### **Tier 3: Official Website (mofa.ai)**
**Purpose**: Structured learning, SEO, marketing

**Content Sync Strategy**:
- Website is **source of truth** for tutorials
- `documents/` contains **developer-focused** technical guides
- Copy polished tutorials from website → `documents/examples/` when stable

**Update Triggers**:
- ✅ New tutorial published on website → add link to README
- ✅ API breaking changes → update website quickstart
- ✅ New major feature (e.g., MoFA Stage) → create website tutorial series
- ❌ Typo fixes on website (no sync needed)

**Review Frequency**: Quarterly (align with roadmap milestones)

---

### 4. Common Update Scenarios

#### **Scenario 1: CLI Command Changed**
**Example**: `mofa run-flow` now supports `--timeout` flag

**Action Checklist**:
- [ ] Update root README CLI command table
- [ ] Update `documents/README*.md` command reference
- [ ] Update affected example docs (e.g., `examples/hello_world.md`)
- [ ] Add example usage to detailed docs
- [ ] Notify website team to update quickstart tutorial
- [ ] Update help text in `mofa/cli.py` (code change)

**Template for CLI Updates**:
```markdown
### Updated Command
**Before**:
```bash
mofa run-flow dataflow.yml --detach
```

**After** (v0.1.34+):
```bash
mofa run-flow dataflow.yml --detach --timeout 300
```

**New Parameters**:
- `--timeout`: Maximum execution time in seconds (default: unlimited)
```

---

#### **Scenario 2: TUI Interface Redesigned**
**Example**: `mofa list` now shows metadata in split-pane view

**Action Checklist**:
- [ ] Take new screenshots (save to `documents/images/`)
- [ ] Update root README if mentioned
- [ ] Update `documents/examples/` tutorials that show TUI usage
- [ ] Record short demo GIF for website
- [ ] Update changelog with "UI Improvements" section

**Screenshot Naming Convention**:
```
documents/images/
├── cli_list_v0.1.34.png          # Versioned screenshots
├── cli_vibe_interactive.gif      # Feature-focused naming
└── dataflow_diagram_cn.png       # Language suffix
```

---

#### **Scenario 3: Dataflow Process Simplified**
**Example**: No longer need `dora up` before `mofa run-flow`

**Action Checklist**:
- [ ] Update ALL examples showing old flow
- [ ] Add migration note to root README (temporary, 1-2 versions)
- [ ] Update `documents/build_agent/build_agent.md`
- [ ] Archive old docs to `documents/archive/` (if major change)
- [ ] Update website quickstart

**Migration Note Template**:
```markdown
> **⚠️ Breaking Change (v0.1.34)**
> Starting from v0.1.34, `dora up` is no longer required before running flows.
>
> **Old way (v0.1.33 and earlier)**:
> ```bash
> dora up && dora build flow.yml && dora start flow.yml
> ```
>
> **New way (v0.1.34+)**:
> ```bash
> mofa run-flow flow.yml
> ```
>
> See [Migration Guide](documents/migration_v0.1.34.md) for details.
```

---

#### **Scenario 4: New Agent Template Added**
**Example**: Added "Multi-Modal Agent" template

**Action Checklist**:
- [ ] Create detailed guide in `documents/build_agent/multimodal_template.md`
- [ ] Add entry to `documents/README.md` template list
- [ ] Update root README if it's a major feature
- [ ] Create example in `examples/multimodal_example/`
- [ ] Notify website team for tutorial creation

---

### 5. Synchronization Workflow

#### **When to Sync**

| Event | Root README | `/documents/` | Website |
|-------|-------------|---------------|---------|
| CLI command added/changed | ✅ Always | ✅ Always | ✅ Within 1 week |
| TUI redesigned | ✅ If major | ✅ Always | ✅ Within 2 weeks |
| Bug fix (no API change) | ❌ No | ❌ No | ❌ No |
| New example added | 🟡 Link only | ✅ Full content | 🟡 Optional |
| Version released | ✅ Version badge | ✅ Archive old | ✅ Changelog |
| Breaking change | ✅ Migration note | ✅ Full update | ✅ Urgent update |

Legend: ✅ Required | 🟡 Optional | ❌ Skip

---

#### **Step-by-Step Sync Process**

**1. Pre-Development Phase** (Design Review)
```bash
# Before implementing feature, check documentation impact
- Will this change CLI commands? → Plan README update
- Will this change TUI? → Prepare screenshot plan
- Will this change dataflow format? → Plan migration guide
```

**2. Development Phase** (Code + Docs Together)
```bash
# Update docs in the SAME PR as code changes
git checkout -b feature/new-timeout-flag
# 1. Implement feature in code
# 2. Update inline help text (mofa/cli.py)
# 3. Update root README
# 4. Update documents/
# 5. Commit together
git add README*.md documents/ mofa/cli.py
git commit -m "feat: Add --timeout flag to run-flow

- Implement timeout logic in commands/run_flow.py
- Update README CLI reference
- Update examples/hello_world.md
- Add migration note for v0.1.34"
```

**3. Pre-Release Phase** (Documentation Audit)
```bash
# Before bumping version, run documentation checklist
□ All CLI commands in README match `mofa --help` output?
□ Screenshots up-to-date?
□ Example code tested and working?
□ Chinese and English docs in sync?
□ Changelog updated?
□ Website notification sent?
```

**4. Release Phase** (Archive & Publish)
```bash
# For major versions (0.2.0, 1.0.0), archive old docs
cp README.md documents/archive/README_v0.1_en.md
cp README_cn.md documents/archive/README_v0.1_cn.md
git tag v0.2.0
```

**5. Post-Release Phase** (Website Sync)
```bash
# Send update notification to website team
Subject: Documentation Update Needed - MoFA v0.1.34

Changed:
- New CLI command: mofa run-flow --timeout
- TUI redesign: mofa list (screenshots attached)
- Breaking change: Simplified dataflow start process

Action Required:
- Update quickstart tutorial (HIGH priority)
- Update CLI reference page
- Add migration guide link

Send to: docs@mofa.ai
```

---

### 6. Quality Checklist

Before committing documentation changes, verify:

#### **Content Quality**
- [ ] Code examples tested and working
- [ ] Screenshots show correct version/output
- [ ] Links not broken (use `markdown-link-check`)
- [ ] Chinese and English versions say the same thing
- [ ] Version numbers accurate (e.g., "v0.1.34+")

#### **Structure Quality**
- [ ] Headings follow consistent hierarchy
- [ ] TOC updated (if file has table of contents)
- [ ] Code blocks have language tags (```python, ```bash)
- [ ] Commands copy-pasteable (no extra prompts like `$`)

#### **Style Quality**
- [ ] Use active voice ("Run the command" not "The command can be run")
- [ ] Imperative for instructions ("Click the button" not "You can click")
- [ ] Consistent terminology (Agent not agent, Dataflow not dataflow)
- [ ] No unnecessary emojis (use sparingly for warnings/tips)

---

### 7. Tools and Automation

#### **Documentation Linting**
```bash
# Install tools
pip install markdown-link-check mdformat

# Check broken links
markdown-link-check README.md documents/**/*.md

# Format markdown
mdformat README.md documents/
```

#### **Screenshot Management**
```bash
# Naming convention
documents/images/{feature}_{version}.{png|gif}

# Example
documents/images/cli_list_v0.1.34.png
documents/images/tui_vibe_interactive.gif

# Cleanup old screenshots when updating
git rm documents/images/cli_list_v0.1.32.png
```

#### **Version Archiving Script**
```bash
#!/bin/bash
# scripts/archive_docs.sh
VERSION=$1
cp README.md documents/archive/README_v${VERSION}_en.md
cp README_cn.md documents/archive/README_v${VERSION}_cn.md
echo "Archived docs for v${VERSION}"
```

---

### 8. Roles and Responsibilities

| Role | Responsibilities | Contact |
|------|------------------|---------|
| **Feature Developer** | Update docs in same PR as code changes; test examples | Team members |
| **Doc Maintainer** | Review doc PRs; enforce style guide; quarterly audit | @tunedbayonet (Discord) / yina.dai@mofa.ai |
| **Release Manager** | Archive docs on major releases; sync with website team | @tunedbayonet / yina.dai@mofa.ai |
| **Community Manager** | Collect user feedback on docs; identify confusing sections | docs@mofa.ai |

---

### 9. Review Schedule

| Frequency | Task | Owner |
|-----------|------|-------|
| **Every PR** | Doc changes included for code changes | Developer |
| **Weekly** | Fix reported doc issues | Doc Maintainer |
| **Monthly** | Review `documents/` for accuracy | Doc Maintainer |
| **Quarterly** | Sync website tutorials | Doc Maintainer + Website Team |
| **Every Release** | README audit + archiving | Release Manager |
| **Yearly** | Full documentation restructure review | Team |

---

### 10. Common Pitfalls and Solutions

| Problem | Solution |
|---------|----------|
| **Docs lag behind code** | Make doc updates mandatory in PR checklist |
| **Screenshots outdated** | Version screenshots; delete old ones |
| **Chinese/English mismatch** | Use side-by-side review tool; pair review |
| **Broken links** | Use automated link checker in CI |
| **Examples don't work** | Add example testing to CI pipeline |
| **Website out of sync** | Set up quarterly sync meetings |

---

### 11. Emergency Documentation Fixes

For critical documentation bugs (e.g., wrong command breaks user workflow):

```bash
# 1. Fast-track fix
git checkout -b hotfix/doc-critical-error
# Edit files
git commit -m "docs: Fix critical error in run-flow example"

# 2. Skip normal review process (single approver OK)
# 3. Merge immediately
# 4. Notify users via Discord
# Post update in https://discord.com/channels/1383895229245030471/1436216311607857287

# 5. Backport to website ASAP
# Send urgent notification to docs@mofa.ai
```

---

### 12. Metrics to Track

Monitor documentation health:

- **Freshness**: % of docs updated in last 3 months
- **Accuracy**: Number of reported doc bugs per release
- **Coverage**: % of CLI commands documented
- **User Satisfaction**: Doc-related issues vs. code issues ratio
- **Example Health**: % of examples passing automated tests

---

### 13. Resources

- **Style Guide**: Follow [Google Developer Documentation Style Guide](https://developers.google.com/style)
- **Markdown Linter**: [markdownlint](https://github.com/DavidAnson/markdownlint)
- **Screenshot Tool**: [Flameshot](https://flameshot.org/) (Linux/macOS)
- **GIF Recorder**: [LICEcap](https://www.cockos.com/licecap/)
- **Link Checker**: [markdown-link-check](https://github.com/tcort/markdown-link-check)

---

<a name="chinese"></a>
## 简体中文版本

### 1. 概述

本指南帮助维护者在三个平台上保持 MoFA 文档同步:
1. **根目录 README** (`/README.md`, `/README_en.md`) - 快速开始和概览
2. **详细文档** (`/documents/`) - 深入指南和示例
3. **官方网站** (mofa.ai) - 教程和结构化学习路径

### 2. 文档结构

```
mofa/
├── README.md                           # 中文快速入门(500-800行)
├── README_en.md                        # 英文快速入门(500-800行)
├── debug.md                            # 调试指南
└── documents/
    ├── README.md                       # 文档索引(英文)
    ├── README_cn.md                    # 文档索引(中文)
    ├── test_plan.md                    # 测试策略
    ├── documentation_maintenance_guide.md  # 本文件
    ├── build_agent/
    │   └── build_agent.md              # Agent构建教程
    ├── examples/
    │   ├── hello_world.md              # Hello World示例(英文)
    │   ├── hello_world_cn.md           # Hello World示例(中文)
    │   └── ...                         # 其他示例
    ├── archive/
    │   ├── README_v0.1_en.md           # 归档版本文档
    │   └── README_v0.1_cn.md
    └── images/                         # 文档图片
```

### 3. 三层文档策略

#### **第一层: 根目录 README (入口)**
**目的**: 第一印象，5分钟内快速上手

**内容指南**:
- 设计理念(4-6个要点)
- 安装方法(最多3种)
- 快速开始示例(少于10行的hello world)
- 核心CLI命令表(8-12个命令)
- 支持矩阵表
- 指向详细文档和网站的链接

**更新触发条件**:
- ✅ 新增CLI命令
- ✅ 安装方法变更
- ✅ 支持矩阵更新(新Python版本、系统支持)
- ✅ 核心功能增删
- ❌ 小bug修复(无需更新)
- ❌ 内部重构(无需更新)

**审查频率**: 每次发布前(版本号升级前)

---

#### **第二层: 详细文档 (`/documents/`)**
**目的**: 为需要深度了解的用户提供全面指南

**内容分类**:

| 类别 | 文件 | 更新触发条件 |
|------|------|-------------|
| **Agent构建** | `build_agent/build_agent.md` | Agent模板变更、MofaAgent API变更 |
| **示例** | `examples/hello_world*.md`等 | 示例代码变更、数据流结构变更 |
| **架构** | `README.md`, `README_cn.md` | 设计模式变更、新模板添加 |
| **测试** | `test_plan.md` | 测试策略变更(季度审查) |
| **归档** | `archive/README_v*.md` | 主版本发布(破坏性变更前快照) |

**更新触发条件**:
- ✅ CLI命令行为变更(如`run-flow`支持`--timeout`)
- ✅ TUI界面重新设计(如`mofa list`布局变化)
- ✅ 数据流启停流程简化
- ✅ 新增agent模板
- ✅ 配置文件格式变更(如`agent.yml`模式更新)
- ✅ 截图过时(UI变化)

**审查频率**: 每月 + 重大功能落地时

---

#### **第三层: 官方网站 (mofa.ai)**
**目的**: 结构化学习、SEO、市场推广

**内容同步策略**:
- 网站是教程的**真理之源**
- `documents/`包含**面向开发者**的技术指南
- 成熟教程从网站复制到`documents/examples/`

**更新触发条件**:
- ✅ 网站发布新教程 → 在README添加链接
- ✅ API破坏性变更 → 更新网站快速入门
- ✅ 新增重大功能(如MoFA Stage) → 创建网站教程系列
- ❌ 网站错别字修复(无需同步)

**审查频率**: 每季度(与路线图里程碑对齐)

---

### 4. 常见更新场景

#### **场景1: CLI命令变更**
**示例**: `mofa run-flow`现在支持`--timeout`参数

**操作清单**:
- [ ] 更新根目录README的CLI命令表
- [ ] 更新`documents/README*.md`的命令参考
- [ ] 更新受影响的示例文档(如`examples/hello_world.md`)
- [ ] 在详细文档中添加示例用法
- [ ] 通知网站团队更新快速入门教程
- [ ] 更新`mofa/cli.py`中的帮助文本(代码变更)

**CLI更新模板**:
```markdown
### 更新的命令
**之前**:
```bash
mofa run-flow dataflow.yml --detach
```

**之后** (v0.1.34+):
```bash
mofa run-flow dataflow.yml --detach --timeout 300
```

**新参数**:
- `--timeout`: 最大执行时间(秒)(默认:无限制)
```

---

#### **场景2: TUI界面重新设计**
**示例**: `mofa list`现在以分屏视图显示元数据

**操作清单**:
- [ ] 拍摄新截图(保存到`documents/images/`)
- [ ] 如有提及则更新根目录README
- [ ] 更新展示TUI用法的`documents/examples/`教程
- [ ] 为网站录制简短演示GIF
- [ ] 在changelog中更新"UI改进"部分

**截图命名规范**:
```
documents/images/
├── cli_list_v0.1.34.png          # 带版本号的截图
├── cli_vibe_interactive.gif      # 以功能为重点的命名
└── dataflow_diagram_cn.png       # 语言后缀
```

---

#### **场景3: 数据流流程简化**
**示例**: `mofa run-flow`之前不再需要`dora up`

**操作清单**:
- [ ] 更新所有展示旧流程的示例
- [ ] 在根目录README添加迁移说明(临时,1-2个版本)
- [ ] 更新`documents/build_agent/build_agent.md`
- [ ] 将旧文档归档到`documents/archive/`(如果重大变更)
- [ ] 更新网站快速入门

**迁移说明模板**:
```markdown
> **⚠️ 破坏性变更 (v0.1.34)**
> 从v0.1.34开始，运行流程前不再需要`dora up`。
>
> **旧方式 (v0.1.33及更早版本)**:
> ```bash
> dora up && dora build flow.yml && dora start flow.yml
> ```
>
> **新方式 (v0.1.34+)**:
> ```bash
> mofa run-flow flow.yml
> ```
>
> 详见[迁移指南](documents/migration_v0.1.34.md)
```

---

#### **场景4: 新增Agent模板**
**示例**: 添加"多模态Agent"模板

**操作清单**:
- [ ] 在`documents/build_agent/multimodal_template.md`创建详细指南
- [ ] 在`documents/README.md`模板列表中添加条目
- [ ] 如果是重大功能则更新根目录README
- [ ] 在`examples/multimodal_example/`创建示例
- [ ] 通知网站团队创建教程

---

### 5. 同步工作流

#### **何时同步**

| 事件 | 根目录README | `/documents/` | 网站 |
|------|-------------|---------------|------|
| CLI命令增加/变更 | ✅ 总是 | ✅ 总是 | ✅ 1周内 |
| TUI重新设计 | ✅ 如果重大 | ✅ 总是 | ✅ 2周内 |
| Bug修复(无API变更) | ❌ 否 | ❌ 否 | ❌ 否 |
| 新增示例 | 🟡 仅链接 | ✅ 完整内容 | 🟡 可选 |
| 版本发布 | ✅ 版本徽章 | ✅ 归档旧版 | ✅ 更新日志 |
| 破坏性变更 | ✅ 迁移说明 | ✅ 完整更新 | ✅ 紧急更新 |

图例: ✅ 必需 | 🟡 可选 | ❌ 跳过

---

#### **分步同步流程**

**1. 开发前阶段** (设计审查)
```bash
# 实现功能前，检查文档影响
- 会改变CLI命令吗? → 计划README更新
- 会改变TUI吗? → 准备截图计划
- 会改变数据流格式吗? → 计划迁移指南
```

**2. 开发阶段** (代码+文档一起)
```bash
# 在同一个PR中更新文档和代码
git checkout -b feature/new-timeout-flag
# 1. 在代码中实现功能
# 2. 更新内联帮助文本(mofa/cli.py)
# 3. 更新根目录README
# 4. 更新documents/
# 5. 一起提交
git add README*.md documents/ mofa/cli.py
git commit -m "feat: 为run-flow添加--timeout参数

- 在commands/run_flow.py实现超时逻辑
- 更新README CLI参考
- 更新examples/hello_world.md
- 为v0.1.34添加迁移说明"
```

**3. 发布前阶段** (文档审核)
```bash
# 升级版本前，运行文档检查清单
□ README中的所有CLI命令与`mofa --help`输出匹配?
□ 截图是最新的?
□ 示例代码已测试且可运行?
□ 中英文文档同步?
□ Changelog已更新?
□ 已发送网站通知?
```

**4. 发布阶段** (归档和发布)
```bash
# 对于主版本(0.2.0, 1.0.0)，归档旧文档
cp README.md documents/archive/README_v0.1_en.md
cp README_cn.md documents/archive/README_v0.1_cn.md
git tag v0.2.0
```

**5. 发布后阶段** (网站同步)
```bash
# 向网站团队发送更新通知
主题: 需要更新文档 - MoFA v0.1.34

变更:
- 新CLI命令: mofa run-flow --timeout
- TUI重新设计: mofa list (附截图)
- 破坏性变更: 简化数据流启动流程

需要操作:
- 更新快速入门教程(高优先级)
- 更新CLI参考页面
- 添加迁移指南链接

发送到: docs@mofa.ai
```

---

### 6. 质量检查清单

提交文档变更前，验证:

#### **内容质量**
- [ ] 代码示例已测试且可运行
- [ ] 截图显示正确的版本/输出
- [ ] 链接未损坏(使用`markdown-link-check`)
- [ ] 中英文版本内容一致
- [ ] 版本号准确(如"v0.1.34+")

#### **结构质量**
- [ ] 标题遵循一致的层次结构
- [ ] TOC已更新(如果文件有目录)
- [ ] 代码块有语言标签(```python, ```bash)
- [ ] 命令可复制粘贴(无额外提示符如`$`)

#### **风格质量**
- [ ] 使用主动语态("运行命令"而非"命令可以被运行")
- [ ] 指令使用祈使句("点击按钮"而非"你可以点击")
- [ ] 术语一致(Agent不是agent，Dataflow不是dataflow)
- [ ] 无不必要的emoji(仅在警告/提示时谨慎使用)

---

### 7. 工具和自动化

#### **文档检查**
```bash
# 安装工具
pip install markdown-link-check mdformat

# 检查损坏链接
markdown-link-check README.md documents/**/*.md

# 格式化markdown
mdformat README.md documents/
```

#### **截图管理**
```bash
# 命名规范
documents/images/{功能}_{版本}.{png|gif}

# 示例
documents/images/cli_list_v0.1.34.png
documents/images/tui_vibe_interactive.gif

# 更新时清理旧截图
git rm documents/images/cli_list_v0.1.32.png
```

#### **版本归档脚本**
```bash
#!/bin/bash
# scripts/archive_docs.sh
VERSION=$1
cp README.md documents/archive/README_v${VERSION}_en.md
cp README_cn.md documents/archive/README_v${VERSION}_cn.md
echo "已归档v${VERSION}的文档"
```

---

### 8. 角色和职责

| 角色 | 职责 | 联系方式 |
|------|------|---------|
| **功能开发者** | 在同一PR中更新文档和代码；测试示例 | 团队成员 |
| **文档维护者** | 审查文档PR；执行风格指南；季度审核 | @tunedbayonet (Discord) / yina.dai@mofa.ai |
| **发布管理员** | 主版本发布时归档文档；与网站团队同步 | @tunedbayonet / yina.dai@mofa.ai |
| **社区管理员** | 收集用户对文档的反馈；识别令人困惑的部分 | docs@mofa.ai |

---

### 9. 审查计划

| 频率 | 任务 | 负责人 |
|------|------|--------|
| **每个PR** | 代码变更包含文档更新 | 开发者 |
| **每周** | 修复报告的文档问题 | 文档维护者 |
| **每月** | 审查`documents/`的准确性 | 文档维护者 |
| **每季度** | 同步网站教程 | 文档维护者+网站团队 |
| **每次发布** | README审核+归档 | 发布管理员 |
| **每年** | 全面文档重组审查 | 团队 |

---

### 10. 常见陷阱和解决方案

| 问题 | 解决方案 |
|------|----------|
| **文档落后于代码** | 在PR检查清单中强制要求文档更新 |
| **截图过时** | 给截图加版本号；删除旧版本 |
| **中英文不匹配** | 使用并排审查工具；配对审查 |
| **链接损坏** | 在CI中使用自动化链接检查器 |
| **示例无法运行** | 在CI管道中添加示例测试 |
| **网站不同步** | 设置季度同步会议 |

---

### 11. 紧急文档修复

对于严重的文档bug(如错误命令破坏用户工作流):

```bash
# 1. 快速修复
git checkout -b hotfix/doc-critical-error
# 编辑文件
git commit -m "docs: 修复run-flow示例中的严重错误"

# 2. 跳过正常审查流程(单个审批者即可)
# 3. 立即合并
# 4. 通过Discord通知用户
# 在 https://discord.com/channels/1383895229245030471/1436216311607857287 发布更新

# 5. 尽快移植到网站
# 发送紧急通知到 docs@mofa.ai
```

---

### 12. 跟踪指标

监控文档健康度:

- **新鲜度**: 最近3个月内更新的文档百分比
- **准确性**: 每次发布报告的文档bug数量
- **覆盖率**: 已记录的CLI命令百分比
- **用户满意度**: 文档相关问题与代码问题的比率
- **示例健康度**: 通过自动化测试的示例百分比

---

### 13. 资源

- **风格指南**: 遵循[Google开发者文档风格指南](https://developers.google.com/style)
- **Markdown检查器**: [markdownlint](https://github.com/DavidAnson/markdownlint)
- **截图工具**: [Flameshot](https://flameshot.org/) (Linux/macOS)
- **GIF录制器**: [LICEcap](https://www.cockos.com/licecap/)
- **链接检查器**: [markdown-link-check](https://github.com/tcort/markdown-link-check)

---

## Appendix: Quick Reference Card

### Documentation Update Decision Tree

```
Code Change Made?
├─ YES → Does it affect user-facing behavior?
│         ├─ YES → Does it change CLI commands?
│         │        ├─ YES → Update: README + documents/ + notify website
│         │        └─ NO  → Does it change TUI?
│         │                 ├─ YES → Update: screenshots + documents/ + (README if major)
│         │                 └─ NO  → Does it change dataflow format?
│         │                          ├─ YES → Update: ALL docs + migration guide
│         │                          └─ NO  → Update: affected examples only
│         └─ NO  → Internal refactor?
│                  ├─ YES → No doc update needed
│                  └─ NO  → Bug fix? → No doc update (unless fix changes usage)
└─ NO → New tutorial/example?
         ├─ YES → Add to documents/examples/ + link from README
         └─ NO  → Version release?
                  └─ YES → Archive old docs + update changelog
```

### Emergency Contact

| Issue Type | Contact | Response Time |
|------------|---------|---------------|
| Critical doc bug | @tunedbayonet on Discord or yina.dai@mofa.ai | < 2 hours |
| Website sync urgent | docs@mofa.ai or yina.dai@mofa.ai | < 1 day |
| Translation error | docs@mofa.ai | < 1 week |
| Screenshot outdated | @tunedbayonet on Discord or yina.dai@mofa.ai | < 1 week |
| General feedback | docs@mofa.ai | < 3 days |
| Direct contact | yina.dai@mofa.ai | < 1 day |

**Discord Channel**: https://discord.com/channels/1383895229245030471/1436216311607857287

---

**Last Updated**: 2025-01-07
**Version**: 1.0
**Maintainer**: @tunedbayonet (Discord) | yina.dai@mofa.ai
**Feedback**: All documentation feedback to docs@mofa.ai or yina.dai@mofa.ai
