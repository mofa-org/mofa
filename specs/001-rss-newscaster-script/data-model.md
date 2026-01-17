# Data Model: RSS to Multi-Newscaster Script Dataflow

**Feature**: 001-rss-newscaster-script
**Date**: 2026-01-09
**Status**: Complete

## Entity Definitions

### 1. RSSInput

Input entity for the dataflow, containing RSS feed URL(s).

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| urls | string[] | Yes | One or more RSS feed URLs to process |
| config | PersonaConfig | No | Optional persona customization |

**Validation Rules**:
- `urls` must contain at least one valid HTTP/HTTPS URL
- Maximum 10 URLs per request (prevents overload)

### 2. NewsItem

Individual article extracted from an RSS feed.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| id | string | Yes | Unique identifier (typically feed item GUID or link hash) |
| title | string | Yes | Article headline |
| description | string | No | Article summary or full content |
| published_date | datetime | No | Publication timestamp |
| source | string | No | Feed title or author name |
| link | string | No | Original article URL |
| feed_url | string | Yes | Source RSS feed URL |

**Validation Rules**:
- `title` must not be empty
- If `description` is empty, script generation uses title only

### 3. ProcessedFeed

Output from news-processor node, containing structured news data.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| feed_title | string | Yes | Title of the RSS feed |
| feed_url | string | Yes | Source URL |
| items | NewsItem[] | Yes | Array of extracted news items |
| item_count | integer | Yes | Total number of items |
| processed_at | datetime | Yes | Processing timestamp |
| errors | string[] | No | Any parsing errors encountered |

**State Transitions**:
- `pending` → `processing` → `completed` | `failed`

### 4. Persona

Newscaster role definition.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| id | string | Yes | Persona identifier: `male_anchor`, `female_anchor`, `commentator` |
| name | string | Yes | Display name (e.g., "张明", "李华", "王教授") |
| role | string | Yes | Role description |
| style | string | Yes | Speaking style characteristics |
| focus | string | No | Content focus area (for commentator) |

**Default Personas**:

| ID | Default Name | Role | Style |
|----|--------------|------|-------|
| male_anchor | 张明 | 男主播 | 清晰、权威的新闻播报 |
| female_anchor | 李华 | 女主播 | 亲和、引人入胜的新闻播报 |
| commentator | 王教授 | 资深评论员 | 分析性、提供背景和专家视角 |

### 5. PersonaConfig

Optional configuration for customizing personas.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| male_anchor | PersonaOverride | No | Override for male anchor |
| female_anchor | PersonaOverride | No | Override for female anchor |
| commentator | PersonaOverride | No | Override for commentator |
| tone | string | No | Overall tone: `formal`, `casual`, `neutral` |
| language | string | No | Output language (default: follows source) |

### 6. PersonaOverride

Override settings for a single persona.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| name | string | No | Custom display name |
| style | string | No | Custom speaking style |
| focus | string | No | Custom focus area |

### 7. ScriptSegment

A portion of the broadcast script assigned to one persona.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| position | integer | Yes | Order in the broadcast (1-based) |
| speaker | string | Yes | Persona ID |
| speaker_label | string | Yes | Display label (e.g., "【男主播】") |
| content | string | Yes | The script text for this segment |
| news_item_id | string | No | Reference to source news item |
| segment_type | string | Yes | Type: `intro`, `news`, `transition`, `analysis`, `outro` |

### 8. BroadcastScript

Complete output script containing all segments.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| id | string | Yes | Unique script identifier |
| title | string | Yes | Broadcast title (e.g., "新闻播报 - 2026-01-09") |
| generated_at | datetime | Yes | Generation timestamp |
| segments | ScriptSegment[] | Yes | Ordered list of script segments |
| segment_count | integer | Yes | Total number of segments |
| source_feeds | string[] | Yes | List of source RSS feed URLs |
| news_item_count | integer | Yes | Number of news items covered |
| personas | Persona[] | Yes | Personas used in this script |
| metadata | object | No | Additional metadata |

**Validation Rules**:
- `segments` must contain at least one segment
- All three personas should appear in the script (unless feed has very few items)
- Segments must be in logical order with smooth transitions

### 9. ErrorResponse

Error output for failed operations.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| error | boolean | Yes | Always `true` for error responses |
| error_type | string | Yes | Error category |
| message | string | Yes | Human-readable error message |
| details | object | No | Additional error context |
| partial_result | object | No | Any partial results before failure |

**Error Types**:
- `feed_fetch_error`: Unable to retrieve RSS feed
- `feed_parse_error`: Invalid RSS/Atom format
- `empty_feed_error`: No items in feed
- `llm_error`: Script generation failed
- `config_error`: Invalid configuration

## Entity Relationships

```
RSSInput (1) ──────> (N) NewsItem
                           │
                           ▼
               ProcessedFeed (contains N NewsItem)
                           │
                           ▼
                  BroadcastScript
                     │    │
                     │    └──> (3) Persona
                     ▼
              (N) ScriptSegment
```

## Data Flow Between Nodes

### Node 1: rss-input → news-processor

**Output**: `RSSInput` as JSON string via pyarrow

```python
# Example output
{
  "urls": ["https://example.com/rss.xml"],
  "config": null  # Optional
}
```

### Node 2: news-processor → script-generator

**Output**: `ProcessedFeed` as JSON string via pyarrow

```python
# Example output
{
  "feed_title": "Tech News Daily",
  "feed_url": "https://example.com/rss.xml",
  "items": [
    {
      "id": "item-001",
      "title": "AI Breakthrough Announced",
      "description": "Scientists reveal new AI capabilities...",
      "published_date": "2026-01-09T10:00:00Z",
      "source": "Tech News Daily",
      "link": "https://example.com/article/001"
    }
  ],
  "item_count": 1,
  "processed_at": "2026-01-09T12:00:00Z",
  "errors": []
}
```

### Node 3: script-generator → output

**Output**: `BroadcastScript` as JSON string via pyarrow

```python
# Example output
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
