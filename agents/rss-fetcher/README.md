# RSS Fetcher

A MoFA agent that fetches and parses RSS feeds from URLs.

## Overview

This agent accepts an RSS feed URL as input, fetches the feed content using feedparser, and outputs the structured feed data as JSON. It handles various RSS and Atom feed formats and provides detailed error handling.

## Inputs

| Input Name | Type | Description |
|------------|------|-------------|
| rss_url | string | RSS feed URL to fetch (plain URL string or JSON with `url` key) |

## Outputs

| Output Name | Type | Description |
|-------------|------|-------------|
| rss_feed | JSON | Parsed feed data including metadata and entries |

### Output Format

Success response:
```json
{
  "error": false,
  "url": "https://example.com/feed.xml",
  "feed": {
    "title": "Example Feed",
    "link": "https://example.com",
    "description": "An example RSS feed"
  },
  "entries": [
    {
      "title": "Article Title",
      "link": "https://example.com/article",
      "description": "Article summary...",
      "published": "2024-01-15T10:30:00",
      "updated": "2024-01-15T10:30:00",
      "author": "Author Name",
      "id": "unique-entry-id"
    }
  ],
  "entry_count": 10,
  "fetched_at": "2024-01-15T12:00:00Z"
}
```

Error response:
```json
{
  "error": true,
  "error_type": "invalid_url",
  "message": "URL must start with http:// or https://",
  "details": {"provided_url": "bad-url"}
}
```

## Configuration

Environment variables:
- `LOG_LEVEL`: Logging level (default: INFO)
- `WRITE_LOG`: Enable logging (default: true)

## Usage

### In a Dataflow

```yaml
nodes:
  - id: rss-fetcher
    build: pip install -e ../../agents/rss-fetcher
    path: rss-fetcher
    inputs:
      rss_url: upstream-node/url_output
    outputs:
      - rss_feed
    env:
      LOG_LEVEL: INFO
```

### Standalone Testing

```bash
cd agents/rss-fetcher
pip install -e .
python -m rss_fetcher
```

## Development

```bash
# Install dependencies
pip install -e .

# Run tests
pytest tests/
```

## Error Types

- `invalid_url`: The provided URL is empty or malformed
- `fetch_error`: Failed to fetch the feed from the URL
- `parse_error`: Failed to parse the feed content
- `empty_feed`: Feed was fetched but contains no entries
- `processing_error`: Unexpected error during processing
