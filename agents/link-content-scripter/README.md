# Link Content Scripter

从 RSS Feed 每条 entry 的链接获取内容，并生成新闻播报稿的 MoFA Agent。

## 概述

该 Agent 接收来自 `rss-fetcher` 的 RSS Feed JSON 数据，对每条 entry：
1. 访问其 link 链接，获取实际网页内容
2. 提取网页的主要文本内容
3. 使用 LLM 基于获取的内容生成 3-5 句话的新闻播报稿
4. 输出所有新闻稿的集合

## 输入

| 输入名称 | 类型 | 描述 |
|----------|------|------|
| rss_feed | JSON | 来自 rss-fetcher 的 RSS Feed 数据 |

## 输出

| 输出名称 | 类型 | 描述 |
|----------|------|------|
| news_scripts | JSON | 新闻播报稿集合 |

输出格式示例：
```json
{
  "error": false,
  "feed_title": "Example Feed",
  "feed_url": "https://example.com/feed.xml",
  "scripts": [
    {
      "id": "entry-id",
      "title": "Article Title",
      "script": "今日要闻：... 这是一条重要新闻。根据报道...",
      "link": "https://example.com/article",
      "published": "2024-01-15T10:30:00",
      "content_fetched": true,
      "content_length": 2500,
      "generated_at": "2024-01-15T12:00:00Z"
    }
  ],
  "script_count": 10,
  "fetch_success_count": 8,
  "fetch_fail_count": 2,
  "generated_at": "2024-01-15T12:00:00Z"
}
```

## 配置

环境变量：
- `LLM_API_KEY` 或 `OPENAI_API_KEY`: LLM API 密钥（必需）
- `LLM_API_BASE` 或 `OPENAI_API_BASE`: LLM API 基础 URL（可选）
- `LLM_MODEL`: 使用的模型名称（默认: gpt-4o-mini）
- `MAX_ENTRIES`: 最大处理条目数（默认: 10）

## 工作流程

```
RSS Feed JSON
    │
    ▼
┌─────────────────────────────────┐
│  对每条 entry:                   │
│  1. 获取 entry.link 的网页内容    │
│  2. 提取主要文本                  │
│  3. 使用 LLM 生成播报稿           │
└─────────────────────────────────┘
    │
    ▼
新闻播报稿集合 JSON
```

## 使用方法

### 在 Dataflow 中使用

```yaml
nodes:
  - id: rss-fetcher
    build: pip install -e ../../agents/rss-fetcher
    path: rss-fetcher
    inputs:
      rss_url: input-node/url
    outputs:
      - rss_feed

  - id: link-content-scripter
    build: pip install -e ../../agents/link-content-scripter
    path: link-content-scripter
    inputs:
      rss_feed: rss-fetcher/rss_feed
    outputs:
      - news_scripts
    env:
      LLM_API_KEY: ${LLM_API_KEY}
      LLM_MODEL: gpt-4o-mini
      MAX_ENTRIES: 10
```

### 单独测试

```bash
cd agents/link-content-scripter
pip install -e .
python -m link_content_scripter
```

## 与 feed-to-scripts 的区别

| 特性 | feed-to-scripts | link-content-scripter |
|------|-----------------|----------------------|
| 内容来源 | RSS entry 的 description | 访问 link 获取完整网页内容 |
| 内容质量 | 依赖 RSS 摘要 | 基于完整文章内容 |
| 处理速度 | 快 | 较慢（需要网络请求） |
| 适用场景 | RSS 摘要完整的情况 | 需要详细内容的情况 |

## 开发

```bash
# 安装依赖
pip install -e .

# 运行测试
pytest tests/
```

## 错误类型

- `parse_error`: 输入 JSON 解析失败
- `empty_feed`: Feed 数据中没有条目
- `fetch_error`: 无法获取链接内容
