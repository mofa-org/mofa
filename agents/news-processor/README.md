# News Processor Agent

新闻处理 Agent，负责获取和解析 RSS 订阅源内容。

## 功能

- 使用 feedparser 库获取和解析 RSS/Atom 订阅
- 支持同时处理多个 RSS 源
- 自动去重（基于标题相似度）
- 按发布时间排序
- 完善的错误处理

## 安装

```bash
cd agents/news-processor
pip install -e .
```

## 依赖

- `feedparser` - RSS/Atom 解析
- `mofa` - MoFA Agent 框架
- `pyarrow` - 数据序列化

## 输入格式

接收 `RSSInput` JSON：

```json
{
  "urls": ["https://example.com/feed1.xml", "https://example.com/feed2.xml"],
  "config": {}
}
```

## 输出格式

输出 `ProcessedFeed` JSON：

```json
{
  "feed_title": "Combined Feed (2 sources)",
  "feed_url": "https://example.com/feed1.xml",
  "source_feeds": [
    "https://example.com/feed1.xml",
    "https://example.com/feed2.xml"
  ],
  "items": [
    {
      "id": "abc123",
      "title": "新闻标题",
      "description": "新闻摘要...",
      "published_date": "2025-01-09T10:00:00Z",
      "source": "新闻来源",
      "link": "https://example.com/article",
      "feed_url": "https://example.com/feed1.xml"
    }
  ],
  "item_count": 1,
  "processed_at": "2025-01-09T12:00:00Z",
  "errors": [],
  "config": {}
}
```

## 错误处理

| 错误类型 | 说明 |
|----------|------|
| `feed_fetch_error` | 无法获取 RSS 源 |
| `feed_parse_error` | RSS 格式解析失败 |
| `empty_feed_error` | RSS 源无内容 |

多源模式下，单个源失败不会中断整体处理，错误会记录在 `errors` 数组中。

## 在 Dataflow 中使用

```yaml
nodes:
  - id: news-processor
    path: dynamic
    inputs:
      rss_request: rss-input/rss_request
    outputs:
      - processed_feed
```

## 数据模型

参见 `specs/001-rss-newscaster-script/data-model.md` 中的 `ProcessedFeed` 和 `NewsItem` 定义。
