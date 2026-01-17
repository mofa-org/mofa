# 数据模型：RSS 转多主播新闻稿数据流

**功能**: 001-rss-newscaster-script
**日期**: 2026-01-09
**状态**: 完成

## 实体定义

### 1. RSSInput（RSS输入）

数据流的输入实体，包含 RSS 订阅 URL。

| 字段 | 类型 | 必需 | 描述 |
|------|------|------|------|
| urls | string[] | 是 | 一个或多个要处理的 RSS 订阅 URL |
| config | PersonaConfig | 否 | 可选的主播自定义配置 |

**验证规则**:
- `urls` 必须包含至少一个有效的 HTTP/HTTPS URL
- 每个请求最多 10 个 URL（防止过载）

### 2. NewsItem（新闻项目）

从 RSS 订阅中提取的单篇文章。

| 字段 | 类型 | 必需 | 描述 |
|------|------|------|------|
| id | string | 是 | 唯一标识符（通常是订阅项目 GUID 或链接哈希） |
| title | string | 是 | 文章标题 |
| description | string | 否 | 文章摘要或完整内容 |
| published_date | datetime | 否 | 发布时间戳 |
| source | string | 否 | 订阅标题或作者名称 |
| link | string | 否 | 原始文章 URL |
| feed_url | string | 是 | 来源 RSS 订阅 URL |

**验证规则**:
- `title` 不能为空
- 如果 `description` 为空，稿件生成仅使用标题

### 3. ProcessedFeed（已处理的订阅）

news-processor 节点的输出，包含结构化的新闻数据。

| 字段 | 类型 | 必需 | 描述 |
|------|------|------|------|
| feed_title | string | 是 | RSS 订阅的标题 |
| feed_url | string | 是 | 来源 URL |
| items | NewsItem[] | 是 | 提取的新闻项目数组 |
| item_count | integer | 是 | 项目总数 |
| processed_at | datetime | 是 | 处理时间戳 |
| errors | string[] | 否 | 遇到的任何解析错误 |

**状态转换**:
- `pending`（待处理）→ `processing`（处理中）→ `completed`（已完成）| `failed`（失败）

### 4. Persona（主播）

新闻主播角色定义。

| 字段 | 类型 | 必需 | 描述 |
|------|------|------|------|
| id | string | 是 | 主播标识符：`male_anchor`、`female_anchor`、`commentator` |
| name | string | 是 | 显示名称（例如，"张明"、"李华"、"王教授"） |
| role | string | 是 | 角色描述 |
| style | string | 是 | 说话风格特征 |
| focus | string | 否 | 内容关注领域（用于评论员） |

**默认主播**:

| ID | 默认名称 | 角色 | 风格 |
|----|----------|------|------|
| male_anchor | 张明 | 男主播 | 清晰、权威的新闻播报 |
| female_anchor | 李华 | 女主播 | 亲和、引人入胜的新闻播报 |
| commentator | 王教授 | 资深评论员 | 分析性、提供背景和专家视角 |

### 5. PersonaConfig（主播配置）

用于自定义主播的可选配置。

| 字段 | 类型 | 必需 | 描述 |
|------|------|------|------|
| male_anchor | PersonaOverride | 否 | 男主播的覆盖设置 |
| female_anchor | PersonaOverride | 否 | 女主播的覆盖设置 |
| commentator | PersonaOverride | 否 | 评论员的覆盖设置 |
| tone | string | 否 | 整体基调：`formal`（正式）、`casual`（休闲）、`neutral`（中性） |
| language | string | 否 | 输出语言（默认：跟随来源） |

### 6. PersonaOverride（主播覆盖）

单个主播的覆盖设置。

| 字段 | 类型 | 必需 | 描述 |
|------|------|------|------|
| name | string | 否 | 自定义显示名称 |
| style | string | 否 | 自定义说话风格 |
| focus | string | 否 | 自定义关注领域 |

### 7. ScriptSegment（稿件片段）

分配给一位主播的播出稿件的一部分。

| 字段 | 类型 | 必需 | 描述 |
|------|------|------|------|
| position | integer | 是 | 在播出中的顺序（从 1 开始） |
| speaker | string | 是 | 主播 ID |
| speaker_label | string | 是 | 显示标签（例如，"【男主播】"） |
| content | string | 是 | 此片段的稿件文本 |
| news_item_id | string | 否 | 对源新闻项目的引用 |
| segment_type | string | 是 | 类型：`intro`（开场）、`news`（新闻）、`transition`（过渡）、`analysis`（分析）、`outro`（结束） |

### 8. BroadcastScript（播出稿件）

包含所有片段的完整输出稿件。

| 字段 | 类型 | 必需 | 描述 |
|------|------|------|------|
| id | string | 是 | 唯一稿件标识符 |
| title | string | 是 | 播出标题（例如，"新闻播报 - 2026-01-09"） |
| generated_at | datetime | 是 | 生成时间戳 |
| segments | ScriptSegment[] | 是 | 有序的稿件片段列表 |
| segment_count | integer | 是 | 片段总数 |
| source_feeds | string[] | 是 | 源 RSS 订阅 URL 列表 |
| news_item_count | integer | 是 | 覆盖的新闻项目数量 |
| personas | Persona[] | 是 | 此稿件中使用的主播 |
| metadata | object | 否 | 附加元数据 |

**验证规则**:
- `segments` 必须包含至少一个片段
- 所有三位主播都应出现在稿件中（除非订阅项目很少）
- 片段必须按逻辑顺序排列，并有流畅的过渡

### 9. ErrorResponse（错误响应）

失败操作的错误输出。

| 字段 | 类型 | 必需 | 描述 |
|------|------|------|------|
| error | boolean | 是 | 对于错误响应始终为 `true` |
| error_type | string | 是 | 错误类别 |
| message | string | 是 | 人类可读的错误消息 |
| details | object | 否 | 附加错误上下文 |
| partial_result | object | 否 | 失败前的任何部分结果 |

**错误类型**:
- `feed_fetch_error`：无法获取 RSS 订阅
- `feed_parse_error`：无效的 RSS/Atom 格式
- `empty_feed_error`：订阅中没有项目
- `llm_error`：稿件生成失败
- `config_error`：无效的配置

## 实体关系

```
RSSInput (1) ──────> (N) NewsItem
                           │
                           ▼
               ProcessedFeed（包含 N 个 NewsItem）
                           │
                           ▼
                  BroadcastScript
                     │    │
                     │    └──> (3) Persona
                     ▼
              (N) ScriptSegment
```

## 节点间数据流

### 节点 1：rss-input → news-processor

**输出**：通过 pyarrow 以 JSON 字符串形式传递 `RSSInput`

```python
# 示例输出
{
  "urls": ["https://example.com/rss.xml"],
  "config": null  # 可选
}
```

### 节点 2：news-processor → script-generator

**输出**：通过 pyarrow 以 JSON 字符串形式传递 `ProcessedFeed`

```python
# 示例输出
{
  "feed_title": "科技新闻日报",
  "feed_url": "https://example.com/rss.xml",
  "items": [
    {
      "id": "item-001",
      "title": "人工智能重大突破发布",
      "description": "科学家揭示新的人工智能能力...",
      "published_date": "2026-01-09T10:00:00Z",
      "source": "科技新闻日报",
      "link": "https://example.com/article/001"
    }
  ],
  "item_count": 1,
  "processed_at": "2026-01-09T12:00:00Z",
  "errors": []
}
```

### 节点 3：script-generator → 输出

**输出**：通过 pyarrow 以 JSON 字符串形式传递 `BroadcastScript`

```python
# 示例输出
{
  "id": "script-20260109-001",
  "title": "新闻播报 - 2026-01-09",
  "generated_at": "2026-01-09T12:01:00Z",
  "segments": [
    {
      "position": 1,
      "speaker": "male_anchor",
      "speaker_label": "【张明】",
      "content": "各位观众朋友，大家好！欢迎收看今天的新闻播报。",
      "segment_type": "intro"
    },
    {
      "position": 2,
      "speaker": "female_anchor", 
      "speaker_label": "【李华】",
      "content": "今天的头条新闻，科学家宣布了人工智能领域的重大突破...",
      "news_item_id": "item-001",
      "segment_type": "news"
    },
    {
      "position": 3,
      "speaker": "commentator",
      "speaker_label": "【王教授】",
      "content": "这项突破意味着什么呢？让我来为大家分析一下...",
      "news_item_id": "item-001",
      "segment_type": "analysis"
    }
  ],
  "segment_count": 3,
  "source_feeds": ["https://example.com/rss.xml"],
  "news_item_count": 1,
  "personas": [...]
}
```
