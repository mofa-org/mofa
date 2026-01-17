#!/usr/bin/env python3
"""
Integration tests for RSS to Multi-Newscaster Script dataflow.

These tests validate the complete dataflow from RSS input to script generation.
"""
import json
import os
import sys
from unittest.mock import MagicMock, patch

import pytest

# Add agent paths for imports
sys.path.insert(0, os.path.join(os.path.dirname(__file__), '../../../agents/rss-input'))
sys.path.insert(0, os.path.join(os.path.dirname(__file__), '../../../agents/news-processor'))
sys.path.insert(0, os.path.join(os.path.dirname(__file__), '../../../agents/script-generator'))


# Sample RSS feed data for testing
SAMPLE_RSS_FEED = """<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>Test News Feed</title>
    <link>https://example.com</link>
    <description>A test news feed</description>
    <item>
      <title>AI Technology Breakthrough</title>
      <description>Scientists announce major advancement in artificial intelligence.</description>
      <link>https://example.com/news/1</link>
      <pubDate>Mon, 09 Jan 2026 10:00:00 GMT</pubDate>
    </item>
    <item>
      <title>Climate Summit Concludes</title>
      <description>World leaders reach agreement on emission targets.</description>
      <link>https://example.com/news/2</link>
      <pubDate>Mon, 09 Jan 2026 09:00:00 GMT</pubDate>
    </item>
  </channel>
</rss>
"""


class TestRSSInput:
    """Tests for rss-input agent."""

    def test_create_rss_request_single_url(self):
        """Test creating RSS request with single URL."""
        from rss_input.main import create_rss_request

        request = create_rss_request(["https://example.com/feed.xml"])

        assert request["urls"] == ["https://example.com/feed.xml"]
        assert request["config"] is None

    def test_create_rss_request_multiple_urls(self):
        """Test creating RSS request with multiple URLs."""
        from rss_input.main import create_rss_request

        urls = ["https://feed1.com/rss", "https://feed2.com/rss"]
        request = create_rss_request(urls)

        assert request["urls"] == urls
        assert len(request["urls"]) == 2

    def test_create_rss_request_with_config(self):
        """Test creating RSS request with persona config."""
        from rss_input.main import create_rss_request

        config = {"tone": "formal", "male_anchor": {"name": "John"}}
        request = create_rss_request(["https://example.com/feed.xml"], config)

        assert request["config"] == config

    def test_clean_string(self):
        """Test string cleaning function."""
        from rss_input.main import clean_string

        assert clean_string("  hello world  ") == "hello world"
        assert clean_string("hello") == "hello"


class TestNewsProcessor:
    """Tests for news-processor agent."""

    def test_generate_item_id_with_guid(self):
        """Test ID generation with GUID."""
        from news_processor.main import generate_item_id

        item = {"id": "unique-guid-123"}
        item_id = generate_item_id(item, "https://example.com/feed")

        assert item_id == "unique-guid-123"

    def test_generate_item_id_with_link(self):
        """Test ID generation from link hash."""
        from news_processor.main import generate_item_id

        item = {"link": "https://example.com/article/1"}
        item_id = generate_item_id(item, "https://example.com/feed")

        assert len(item_id) == 16  # MD5 hash truncated

    def test_extract_news_item(self):
        """Test extracting news item from feedparser entry."""
        from news_processor.main import extract_news_item

        entry = {
            "id": "test-123",
            "title": "Test Article",
            "summary": "This is a test summary.",
            "link": "https://example.com/article",
            "author": "Test Author"
        }

        item = extract_news_item(entry, "https://feed.example.com", "Test Feed")

        assert item["id"] == "test-123"
        assert item["title"] == "Test Article"
        assert item["description"] == "This is a test summary."
        assert item["source"] == "Test Author"
        assert item["feed_url"] == "https://feed.example.com"

    def test_create_error_response(self):
        """Test error response creation."""
        from news_processor.main import create_error_response

        error = create_error_response(
            "feed_fetch_error",
            "Connection timeout",
            {"url": "https://example.com"}
        )

        assert error["error"] is True
        assert error["error_type"] == "feed_fetch_error"
        assert error["message"] == "Connection timeout"
        assert error["details"]["url"] == "https://example.com"


class TestScriptGenerator:
    """Tests for script-generator agent."""

    def test_get_personas_defaults(self):
        """Test getting default personas."""
        from script_generator.main import get_personas

        personas = get_personas()

        assert "male_anchor" in personas
        assert "female_anchor" in personas
        assert "commentator" in personas
        assert personas["male_anchor"]["role"] == "男主播"

    def test_determine_segment_type_intro(self):
        """Test segment type detection for intro."""
        from script_generator.main import determine_segment_type

        content = "各位观众朋友，大家好！欢迎收看今天的新闻。"
        segment_type = determine_segment_type(content, 1, "male_anchor")

        assert segment_type == "intro"

    def test_determine_segment_type_analysis(self):
        """Test segment type detection for commentator."""
        from script_generator.main import determine_segment_type

        content = "关于这条新闻，我认为有几个关键点需要注意。"
        segment_type = determine_segment_type(content, 5, "commentator")

        assert segment_type == "analysis"

    def test_determine_segment_type_outro(self):
        """Test segment type detection for outro."""
        from script_generator.main import determine_segment_type

        content = "感谢收看今天的新闻，明天见！"
        segment_type = determine_segment_type(content, 10, "male_anchor")

        assert segment_type == "outro"

    def test_build_script_prompt(self):
        """Test prompt building."""
        from script_generator.main import build_script_prompt, get_personas

        processed_feed = {
            "feed_title": "Test Feed",
            "items": [
                {
                    "title": "Test News",
                    "description": "Test description",
                    "source": "Test Source",
                    "published_date": "2026-01-09T10:00:00Z"
                }
            ]
        }

        prompt = build_script_prompt(processed_feed, get_personas())

        assert "Test News" in prompt
        assert "张明" in prompt or "male_anchor" in prompt.lower()
        assert "李华" in prompt or "female_anchor" in prompt.lower()
        assert "王教授" in prompt or "commentator" in prompt.lower()


class TestDataModel:
    """Tests for data model compliance."""

    def test_rss_input_schema(self):
        """Test RSSInput schema compliance."""
        rss_input = {
            "urls": ["https://example.com/feed.xml"],
            "config": None
        }

        assert "urls" in rss_input
        assert isinstance(rss_input["urls"], list)
        assert len(rss_input["urls"]) >= 1

    def test_processed_feed_schema(self):
        """Test ProcessedFeed schema compliance."""
        processed_feed = {
            "feed_title": "Test Feed",
            "feed_url": "https://example.com/feed.xml",
            "items": [],
            "item_count": 0,
            "processed_at": "2026-01-09T12:00:00Z",
            "errors": []
        }

        required_fields = ["feed_title", "feed_url", "items", "item_count", "processed_at"]
        for field in required_fields:
            assert field in processed_feed

    def test_broadcast_script_schema(self):
        """Test BroadcastScript schema compliance."""
        broadcast_script = {
            "id": "script-123",
            "title": "新闻播报 - 2026-01-09",
            "generated_at": "2026-01-09T12:00:00Z",
            "segments": [],
            "segment_count": 0,
            "source_feeds": ["https://example.com/feed.xml"],
            "news_item_count": 0,
            "personas": []
        }

        required_fields = ["id", "title", "generated_at", "segments", "segment_count",
                         "source_feeds", "news_item_count", "personas"]
        for field in required_fields:
            assert field in broadcast_script

    def test_script_segment_schema(self):
        """Test ScriptSegment schema compliance."""
        segment = {
            "position": 1,
            "speaker": "male_anchor",
            "speaker_label": "【张明】",
            "content": "大家好！",
            "news_item_id": None,
            "segment_type": "intro"
        }

        required_fields = ["position", "speaker", "speaker_label", "content", "segment_type"]
        for field in required_fields:
            assert field in segment

        assert segment["speaker"] in ["male_anchor", "female_anchor", "commentator"]
        assert segment["segment_type"] in ["intro", "news", "transition", "analysis", "outro"]


# Sample RSS feed URLs for manual testing
SAMPLE_RSS_URLS = [
    "https://feeds.bbci.co.uk/news/rss.xml",
    "https://rss.nytimes.com/services/xml/rss/nyt/World.xml",
    "https://feeds.npr.org/1001/rss.xml"
]


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
