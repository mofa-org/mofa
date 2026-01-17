# 研究：RSS 转多主播新闻稿数据流

**功能**: 001-rss-newscaster-script
**日期**: 2026-01-09
**状态**: 完成

## 研究任务

### 1. MoFA 数据流模式

**决策**: 遵循现有的 MoFA 数据流模式，如 `flows/podcast-generator/` 和 `flows/openai_chat_agent/` 所示。

**理由**: 
- 现有模式已被验证且符合 MoFA 宪法
- 基于 YAML 的声明式数据流定义
- Agent 通过 dora-rs 使用 pyarrow 数组进行通信
- 每个 Agent 是一个 Python 包，使用 `MofaAgent` 基类的 `main.py` 入口点

**考虑的替代方案**:
- 自定义工作流引擎：拒绝 - 违反宪法原则二（数据流优先）
- 命令式 Python 脚本：拒绝 - 违反宪法原则二（首选声明式 YAML）

### 2. RSS 解析库

**决策**: 使用 `feedparser` 库进行 RSS/Atom 订阅解析。

**理由**:
- 最广泛使用的 Python RSS 解析器
- 处理 RSS 2.0 和 Atom 格式（根据规格假设）
- 对格式错误的订阅有健壮的错误处理
- 活跃的维护和社区支持
- 简单的 API：`feedparser.parse(url)` 返回结构化数据

**考虑的替代方案**:
- `atoma`：更轻量但不够成熟，社区较小
- 自定义 XML 解析：拒绝 - 不必要的复杂性，重新发明轮子

### 3. 稿件生成的 LLM 集成

**决策**: 通过现有 MoFA 模式使用 OpenAI 兼容 API（如 `openai_chat_agent` 所示）。

**理由**:
- 代码库中已验证的模式
- 通过 `LLM_API_BASE` 环境变量支持多个 LLM 提供商
- 处理流式和非流式响应
- 基于环境的配置（`LLM_API_KEY`、`LLM_MODEL`）

**考虑的替代方案**:
- 直接 Anthropic/Claude API：可行但增加依赖，OpenAI 兼容更灵活
- 本地 LLM：可能的未来增强，但为 MVP 增加复杂性

### 4. 稿件输出格式

**决策**: 带有发言者标签的结构化 JSON 输出，然后格式化为可读文本。

**理由**:
- JSON 结构允许下游程序化处理
- 文本格式适合人工审阅或 TTS 转换（根据规格假设）
- 清晰的发言者标签，使用 `【男主播】`、`【女主播】`、`【评论员】` 格式
- 与现有 `script-segmenter` Agent 模式兼容，便于未来 TTS 集成

**输出格式**:
```json
{
  "broadcast_script": {
    "title": "新闻播报 - {date}",
    "segments": [
      {
        "speaker": "male_anchor",
        "speaker_label": "【男主播】",
        "content": "...",
        "position": 1
      },
      ...
    ]
  }
}
```

**考虑的替代方案**:
- 仅 Markdown 格式：程序化使用时结构性较差
- XML 格式：对此用例过于复杂

### 5. 节点架构

**决策**: 三节点数据流架构：

1. **rss-input**: 动态节点，接受来自用户/环境的 RSS URL
2. **news-processor**: 获取并解析 RSS 内容，提取结构化新闻项目
3. **script-generator**: 基于 LLM 的 Agent，生成多主播稿件

**理由**:
- 遵循 Unix 哲学（每个节点只做好一件事）
- 符合用户规格："第一个接收 RSS URL 作为输入，第二个节点处理新闻数据获取和处理，第三个将数据转换为稿件"
- 每个节点可独立测试
- 节点之间有清晰的数据契约

**考虑的替代方案**:
- 两节点设计（合并处理 + 生成）：降低可组合性，更难独立测试
- 四节点设计（每个主播单独节点）：过度设计，LLM 可以在一次调用中处理所有三个主播

### 6. 主播配置

**决策**: 使用环境变量和可选配置文件进行主播自定义。

**理由**:
- 符合 MoFA 模式（参见 TTS Agent 使用环境变量进行语音配置）
- 支持 FR-010："系统必须支持通过数据流参数配置主播特征"
- 默认主播开箱即用；自定义为可选

**配置方法**:
```yaml
# 在数据流 YAML 中
env:
  MALE_ANCHOR_STYLE: "权威、专业"
  FEMALE_ANCHOR_STYLE: "温暖、引人入胜"
  COMMENTATOR_STYLE: "分析性、有洞察力"
```

**考虑的替代方案**:
- 单独的配置 YAML 文件：为 MVP 增加复杂性
- 硬编码主播：不满足 FR-010 要求

### 7. 错误处理

**决策**: 优雅降级并提供清晰的错误消息。

**理由**:
- 规格中定义了边界情况（空订阅、无效 URL、缺少描述）
- 返回有意义的 JSON 错误结构
- 记录错误以便调试

**错误响应格式**:
```json
{
  "error": true,
  "error_type": "feed_fetch_error",
  "message": "无法获取 RSS 订阅：连接超时",
  "partial_result": null
}
```

## 已解决的澄清事项

所有技术上下文项目现已解决。没有剩余的"待澄清"标记。

## 依赖摘要

| 依赖 | 版本 | 用途 |
|------|------|------|
| mofa | latest | 框架基类和工具 |
| dora-rs | latest | 数据流运行时 |
| pyarrow | latest | 节点间数据传递 |
| feedparser | >=6.0 | RSS/Atom 解析 |
| openai | >=1.0 | LLM API 客户端 |
| python-dotenv | latest | 环境配置 |

## 下一步

进入第 1 阶段：设计与契约
- 生成包含实体定义的 data-model.md
- 生成节点间通信的契约
- 创建包含可运行示例的 quickstart.md
