# Script Generator Agent

脚本生成 Agent，使用 LLM 生成多主播新闻播报稿件。

## 功能

- 使用 OpenAI 兼容 API 生成新闻稿件
- 三个预设主播角色（男主播、女主播、评论员）
- 支持自定义主播名称、风格和关注领域
- 支持多种播报风格（正式、轻松、中性）
- 自动解析稿件段落并标注发言人

## 安装

```bash
cd agents/script-generator
pip install -e .
```

## 依赖

- `openai` - OpenAI API 客户端
- `mofa` - MoFA Agent 框架
- `pyarrow` - 数据序列化
- `python-dotenv` - 环境变量管理

## 配置

### 必需环境变量

| 变量名 | 说明 |
|--------|------|
| `LLM_API_KEY` | OpenAI API 密钥 |

### 可选环境变量

| 变量名 | 说明 | 默认值 |
|--------|------|--------|
| `LLM_API_BASE` | API 基础 URL | OpenAI 默认 |
| `LLM_MODEL` | 模型名称 | `gpt-4o` |
| `BROADCAST_TONE` | 播报风格 | `formal` |
| `MALE_ANCHOR_NAME` | 男主播名称 | `张明` |
| `MALE_ANCHOR_STYLE` | 男主播风格 | `清晰、权威的新闻播报` |
| `FEMALE_ANCHOR_NAME` | 女主播名称 | `李华` |
| `FEMALE_ANCHOR_STYLE` | 女主播风格 | `亲和、引人入胜的新闻播报` |
| `COMMENTATOR_NAME` | 评论员名称 | `王教授` |
| `COMMENTATOR_STYLE` | 评论员风格 | `分析性、提供背景和专家视角` |
| `COMMENTATOR_FOCUS` | 评论员关注领域 | - |

### 播报风格选项

- `formal` - 正式、严肃的新闻播报风格
- `casual` - 轻松、亲切的早间节目风格
- `neutral` - 中性、客观的新闻播报风格

## 输入格式

接收 `ProcessedFeed` JSON（来自 news-processor）。

## 输出格式

输出 `BroadcastScript` JSON：

```json
{
  "id": "script-20250109120000-abc12345",
  "title": "新闻播报 - 2025-01-09",
  "generated_at": "2025-01-09T12:00:00Z",
  "segments": [
    {
      "position": 1,
      "speaker": "male_anchor",
      "speaker_label": "【张明】",
      "content": "各位观众朋友，大家好...",
      "segment_type": "intro"
    }
  ],
  "segment_count": 10,
  "source_feeds": ["https://example.com/feed.xml"],
  "news_item_count": 5,
  "personas": [...],
  "metadata": {
    "feed_title": "Example Feed",
    "tone": "formal",
    "multi_source": false
  }
}
```

## 段落类型

| 类型 | 说明 |
|------|------|
| `intro` | 开场白 |
| `news` | 新闻播报 |
| `analysis` | 评论分析 |
| `transition` | 过渡语 |
| `outro` | 结束语 |

## 错误处理

| 错误类型 | 说明 |
|----------|------|
| `llm_error` | LLM API 调用失败 |
| `config_error` | 配置参数无效 |

## 在 Dataflow 中使用

```yaml
nodes:
  - id: script-generator
    path: dynamic
    inputs:
      processed_feed: news-processor/processed_feed
    outputs:
      - broadcast_script
    env:
      LLM_API_KEY: "${LLM_API_KEY}"
      LLM_MODEL: "gpt-4o"
      BROADCAST_TONE: "formal"
```

## 数据模型

参见 `specs/001-rss-newscaster-script/data-model.md` 中的 `BroadcastScript`、`ScriptSegment` 和 `Persona` 定义。
