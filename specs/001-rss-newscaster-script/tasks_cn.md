# 任务清单：RSS 转多主播新闻稿数据流

**输入**: 来自 `/specs/001-rss-newscaster-script/` 的设计文档
**前提条件**: plan.md、spec.md、research.md、data-model.md、contracts/

**测试**: 根据宪法中的质量与测试要求包含测试（需要可运行的数据流示例）。

**组织方式**: 任务按用户故事分组，以便独立实现和测试每个故事。

## 格式：`[ID] [P?] [Story] 描述`

- **[P]**: 可并行执行（不同文件，无依赖）
- **[Story]**: 任务所属的用户故事（例如 US1、US2、US3）
- 描述中包含确切的文件路径

## 路径约定

基于 plan.md 结构：
- **Agent**: `agents/[agent-name]/` - 每个 Agent 是独立的 Python 包
- **数据流**: `flows/rss-newscaster/` - 数据流 YAML 和 README
- **测试**: `agents/[agent-name]/tests/` - 每个 Agent 的测试

---

## 第 1 阶段：设置（共享基础设施）

**目的**: 为所有三个 Agent 进行项目初始化和基本结构创建

- [x] T001 在 agents/rss-input/ 创建 rss-input Agent 目录结构
- [x] T002 [P] 在 agents/news-processor/ 创建 news-processor Agent 目录结构
- [x] T003 [P] 在 agents/script-generator/ 创建 script-generator Agent 目录结构
- [x] T004 [P] 在 flows/rss-newscaster/ 创建数据流目录结构
- [x] T005 在 agents/rss-input/pyproject.toml 创建 rss-input Agent 的 pyproject.toml
- [x] T006 [P] 在 agents/news-processor/pyproject.toml 创建 news-processor Agent 的 pyproject.toml
- [x] T007 [P] 在 agents/script-generator/pyproject.toml 创建 script-generator Agent 的 pyproject.toml
- [x] T008 为所有 Agent 模块创建 __init__.py 文件

**检查点**: 所有 Agent 包结构已准备好进行实现

---

## 第 2 阶段：基础设施（阻塞性前提条件）

**目的**: 必须在任何用户故事实现之前完成的核心基础设施

**⚠️ 关键**: 此阶段完成之前不能开始任何用户故事的工作

- [x] T009 在 flows/rss-newscaster/rss_newscaster_dataflow.yml 创建带有节点定义的数据流 YAML 骨架
- [x] T010 在 flows/rss-newscaster/.env.secret.example 创建包含必需环境变量的示例文件
- [x] T011 创建用于 JSON 模式验证的共享数据模型模块（如需要）- 已跳过：直接在 Agent 中使用 JSON

**检查点**: 基础设施就绪 - 可以开始用户故事实现

---

## 第 3 阶段：用户故事 1 - 从 RSS 订阅生成新闻稿件（优先级：P1）🎯 MVP

**目标**: 将原始 RSS 订阅转换为精心制作的可播出稿件，包含三种不同的声音：男主播、女主播和资深评论员。

**独立测试**: 提供示例 RSS 订阅 URL，验证输出包含所有三位主播角色的正确格式稿件及适当的角色分配。

### 用户故事 1 的实现

#### 节点 1：rss-input Agent

- [x] T012 [US1] 在 agents/rss-input/rss_input/main.py 创建使用 MofaAgent 基类的 main.py
- [x] T013 [US1] 在 agents/rss-input/rss_input/main.py 实现 RSS URL 输入处理（交互式和基于环境变量）
- [x] T014 [US1] 在 agents/rss-input/rss_input/main.py 按 data-model.md 实现 RSSInput JSON 输出格式
- [x] T015 [US1] 在 agents/rss-input/rss_input/main.py 添加带 --help 支持的参数解析

#### 节点 2：news-processor Agent

- [x] T016 [US1] 在 agents/news-processor/news_processor/main.py 创建使用 MofaAgent 基类的 main.py
- [x] T017 [US1] 在 agents/news-processor/news_processor/main.py 使用 feedparser 库实现 RSS 订阅获取
- [x] T018 [US1] 在 agents/news-processor/news_processor/main.py 按 data-model.md 实现 RSS 解析以提取 NewsItem 实体
- [x] T019 [US1] 在 agents/news-processor/news_processor/main.py 按 data-model.md 实现 ProcessedFeed JSON 输出格式
- [x] T020 [US1] 在 agents/news-processor/news_processor/main.py 添加订阅获取失败的错误处理（feed_fetch_error）
- [x] T021 [US1] 在 agents/news-processor/news_processor/main.py 添加解析失败的错误处理（feed_parse_error）
- [x] T022 [US1] 在 agents/news-processor/news_processor/main.py 添加空订阅的错误处理（empty_feed_error）

#### 节点 3：script-generator Agent

- [x] T023 [US1] 在 agents/script-generator/script_generator/main.py 创建使用 MofaAgent 基类的 main.py
- [x] T024 [US1] 在 agents/script-generator/script_generator/main.py 使用环境变量实现 OpenAI API 客户端设置
- [x] T025 [US1] 在 agents/script-generator/script_generator/main.py 实现默认 Persona 定义（male_anchor、female_anchor、commentator）
- [x] T026 [US1] 在 agents/script-generator/script_generator/main.py 实现生成三主播新闻稿的 LLM 提示词
- [x] T027 [US1] 在 agents/script-generator/script_generator/main.py 按 data-model.md 实现带发言者标签的 ScriptSegment 生成
- [x] T028 [US1] 在 agents/script-generator/script_generator/main.py 按 data-model.md 实现 BroadcastScript JSON 输出格式
- [x] T029 [US1] 在 agents/script-generator/script_generator/main.py 的 LLM 提示词中添加发言者之间的自然过渡
- [x] T030 [US1] 在 agents/script-generator/script_generator/main.py 添加 LLM 失败的错误处理（llm_error）

#### 数据流集成

- [x] T031 [US1] 在 flows/rss-newscaster/rss_newscaster_dataflow.yml 完成带有完整节点连接的数据流 YAML
- [x] T032 [US1] 在 flows/rss-newscaster/rss_newscaster_dataflow.yml 添加环境变量配置

#### 可运行示例（根据宪法要求）

- [x] T033 [US1] 在 flows/rss-newscaster/tests/test_dataflow.py 创建可运行的数据流示例测试
- [x] T034 [US1] 在 flows/rss-newscaster/tests/ 添加用于测试的示例 RSS 订阅 URL

**检查点**: 用户故事 1 完成 - 可以从单个 RSS 订阅生成三主播稿件

---

## 第 4 阶段：用户故事 2 - 自定义主播特征（优先级：P2）

**目标**: 允许内容制作人调整每位新闻主播的个性特征、说话风格或关注领域。

**独立测试**: 通过环境变量修改主播配置，验证生成的稿件反映自定义特征。

### 用户故事 2 的实现

- [x] T035 [US2] 在 agents/script-generator/script_generator/main.py 添加从环境变量解析 PersonaConfig
- [x] T036 [US2] 在 agents/script-generator/script_generator/main.py 实现自定义名称的 PersonaOverride 处理
- [x] T037 [US2] 在 agents/script-generator/script_generator/main.py 实现自定义风格的 PersonaOverride 处理
- [x] T038 [US2] 在 agents/script-generator/script_generator/main.py 实现自定义关注领域的 PersonaOverride 处理
- [x] T039 [US2] 在 agents/script-generator/script_generator/main.py 的 LLM 提示词中添加基调配置（正式/休闲/中性）
- [x] T040 [US2] 在 flows/rss-newscaster/rss_newscaster_dataflow.yml 更新数据流 YAML 添加主播配置环境变量
- [x] T041 [US2] 在 agents/rss-input/rss_input/main.py 添加从 rss-input 到 script-generator 的配置传递
- [x] T042 [US2] 在 agents/script-generator/script_generator/main.py 添加无效配置的错误处理（config_error）

**检查点**: 用户故事 2 完成 - 可以通过配置自定义主播

---

## 第 5 阶段：用户故事 3 - 处理多个 RSS 来源（优先级：P3）

**目标**: 将多个 RSS 订阅的新闻合并为单一连贯的播出稿件。

**独立测试**: 提供两个或更多 RSS 订阅 URL，验证输出连贯地整合了所有来源的报道。

### 用户故事 3 的实现

- [x] T043 [US3] 在 agents/rss-input/rss_input/main.py 更新 rss-input 以接受多个 URL
- [x] T044 [US3] 在 agents/news-processor/news_processor/main.py 更新 news-processor 以获取和解析多个订阅
- [x] T045 [US3] 在 agents/news-processor/news_processor/main.py 实现重叠报道的去重逻辑
- [x] T046 [US3] 在 agents/news-processor/news_processor/main.py 将多个订阅的 NewsItems 聚合到单个 ProcessedFeed
- [x] T047 [US3] 在 agents/script-generator/script_generator/main.py 更新 script-generator 以处理来自多个来源的合并新闻
- [x] T048 [US3] 在 agents/script-generator/script_generator/main.py 确保 source_feeds 数组正确列出所有输入 URL

**检查点**: 用户故事 3 完成 - 可以将多个 RSS 订阅合并为单一稿件

---

## 第 6 阶段：完善与跨领域关注点

**目的**: 影响多个用户故事的改进

- [x] T049 [P] 在 agents/rss-input/README.md 创建 rss-input Agent 的 README.md
- [x] T050 [P] 在 agents/news-processor/README.md 创建 news-processor Agent 的 README.md
- [x] T051 [P] 在 agents/script-generator/README.md 创建 script-generator Agent 的 README.md
- [x] T052 [P] 在 flows/rss-newscaster/README.md 创建 rss-newscaster 数据流的 README.md
- [x] T053 为配置文件添加内联注释
- [ ] T054 运行 quickstart.md 验证以确认所有步骤正常工作
- [ ] T055 使用 20+ 条新闻进行性能测试以验证 SC-001（< 2 分钟）

---

## 依赖关系与执行顺序

### 阶段依赖

- **设置（第 1 阶段）**: 无依赖 - 可立即开始
- **基础设施（第 2 阶段）**: 依赖设置完成 - 阻塞所有用户故事
- **用户故事（第 3 阶段+）**: 全部依赖基础设施阶段完成
  - 用户故事 1（P1）: 必须首先完成（核心功能）
  - 用户故事 2（P2）: 可在 US1 完成后开始
  - 用户故事 3（P3）: 可在 US1 完成后开始（独立于 US2）
- **完善（第 6 阶段）**: 依赖所有用户故事完成

### 用户故事依赖

- **用户故事 1（P1）**: 可在基础设施（第 2 阶段）后开始 - 不依赖其他故事
- **用户故事 2（P2）**: 基于 US1 构建 - 添加主播自定义功能
- **用户故事 3（P3）**: 基于 US1 构建 - 添加多来源处理

### 每个用户故事内部

- Agent 实现顺序：rss-input → news-processor → script-generator
- 所有 Agent 就绪后进行数据流集成
- 可运行示例验证完整流程

### 并行机会

**第 1 阶段（设置）**:
- T002、T003、T004 可与 T001 并行
- T006、T007 可与 T005 并行

**第 3 阶段（US1）**:
- rss-input、news-processor、script-generator Agent 可并行开发
- T012-T015（rss-input）与 T016-T022（news-processor）与 T023-T030（script-generator）并行

**第 6 阶段（完善）**:
- 所有 README 任务（T049-T052）可并行

---

## 并行示例：用户故事 1

```bash
# 并行启动所有三个 Agent：
任务: "在 agents/rss-input/rss_input/main.py 创建 rss-input Agent 的 main.py"
任务: "在 agents/news-processor/news_processor/main.py 创建 news-processor Agent 的 main.py"
任务: "在 agents/script-generator/script_generator/main.py 创建 script-generator Agent 的 main.py"
```

---

## 实现策略

### MVP 优先（仅用户故事 1）

1. 完成第 1 阶段：设置（T001-T008）
2. 完成第 2 阶段：基础设施（T009-T011）
3. 完成第 3 阶段：用户故事 1（T012-T034）
4. **停止并验证**: 使用真实 RSS 订阅测试
5. 如果就绪则部署/演示 - 核心功能可用！

### 增量交付

1. 完成设置 + 基础设施 → 基础就绪
2. 添加用户故事 1 → 独立测试 → 部署/演示（MVP！）
3. 添加用户故事 2 → 使用自定义主播测试 → 部署/演示
4. 添加用户故事 3 → 使用多个订阅测试 → 部署/演示
5. 每个故事增加价值而不破坏之前的故事

---

## 备注

- [P] 任务 = 不同文件，无依赖
- [Story] 标签将任务映射到特定用户故事以便追溯
- 每个用户故事应可独立完成和测试
- 每个任务或逻辑组完成后提交
- 在任何检查点停止以独立验证故事
- 遵循现有 MoFA 模式（参见 podcast-generator、openai_chat_agent）
