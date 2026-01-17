# Anchor Assigner

为新闻播报稿分配男女主播的 MoFA Agent。

## 概述

该 Agent 接收新闻播报稿集合（来自 `link-content-scripter` 或 `feed-to-scripts`），
为每条播报稿分配一个主播（男或女），交替分配，输出 (script, anchor) 配对列表。

## 输入

| 输入名称 | 类型 | 描述 |
|----------|------|------|
| news_scripts | JSON | 来自 link-content-scripter 或 feed-to-scripts 的播报稿集合 |

## 输出

| 输出名称 | 类型 | 描述 |
|----------|------|------|
| anchor_pairs | JSON | (script, anchor) 配对列表 |

输出格式示例：
```json
{
  "error": false,
  "feed_title": "Hacker News",
  "feed_url": "https://news.ycombinator.com/rss",
  "pairs": [
    {
      "script": "今日要闻：...",
      "script_id": "entry-1",
      "title": "News Title",
      "anchor": {
        "id": "female_anchor",
        "name": "李华",
        "gender": "female",
        "role": "女主播"
      },
      "position": 1
    },
    {
      "script": "接下来这条新闻：...",
      "script_id": "entry-2",
      "title": "Another News",
      "anchor": {
        "id": "male_anchor",
        "name": "张明",
        "gender": "male",
        "role": "男主播"
      },
      "position": 2
    }
  ],
  "pair_count": 2,
  "anchors": {
    "male": {"id": "male_anchor", "name": "张明", "gender": "male", "role": "男主播"},
    "female": {"id": "female_anchor", "name": "李华", "gender": "female", "role": "女主播"}
  },
  "assigned_at": "2024-01-15T12:00:00Z"
}
```

## 配置

环境变量：
- `MALE_ANCHOR_NAME`: 男主播名称（默认: 张明）
- `FEMALE_ANCHOR_NAME`: 女主播名称（默认: 李华）

## 使用方法

### 在 Dataflow 中使用

```yaml
nodes:
  - id: rss-fetcher
    inputs:
      rss_url: input/url
    outputs:
      - rss_feed

  - id: link-content-scripter
    inputs:
      rss_feed: rss-fetcher/rss_feed
    outputs:
      - news_scripts

  - id: anchor-assigner
    build: pip install -e ../../agents/anchor-assigner
    path: anchor-assigner
    inputs:
      news_scripts: link-content-scripter/news_scripts
    outputs:
      - anchor_pairs
    env:
      MALE_ANCHOR_NAME: 张明
      FEMALE_ANCHOR_NAME: 李华
```

### 单独测试

```bash
cd agents/anchor-assigner
pip install -e .
mofa run-node anchor-assigner
```

## 分配逻辑

- 从女主播开始，交替分配
- 第 1、3、5... 条新闻由女主播播报
- 第 2、4、6... 条新闻由男主播播报

## 开发

```bash
# 安装依赖
pip install -e .

# 运行测试
pytest tests/
```
