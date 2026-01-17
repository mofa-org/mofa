"""Tests for rss-fetcher agent."""
import json
import pytest
from unittest.mock import MagicMock, patch


class TestCreateErrorResponse:
    """Tests for create_error_response function."""

    def test_basic_error(self):
        """Test basic error response creation."""
        from rss_fetcher.main import create_error_response

        error = create_error_response(
            "test_error",
            "Test error message"
        )

        assert error["error"] is True
        assert error["error_type"] == "test_error"
        assert error["message"] == "Test error message"
        assert error["details"] == {}

    def test_error_with_details(self):
        """Test error response with details."""
        from rss_fetcher.main import create_error_response

        error = create_error_response(
            "invalid_url",
            "URL is invalid",
            {"provided_url": "bad-url"}
        )

        assert error["error"] is True
        assert error["error_type"] == "invalid_url"
        assert error["details"]["provided_url"] == "bad-url"


class TestParseFeedEntry:
    """Tests for parse_feed_entry function."""

    def test_parse_complete_entry(self):
        """Test parsing an entry with all fields."""
        from rss_fetcher.main import parse_feed_entry

        mock_entry = MagicMock()
        mock_entry.title = "Test Article"
        mock_entry.link = "https://example.com/article"
        mock_entry.description = "Article description"
        mock_entry.published = "2024-01-15"
        mock_entry.published_parsed = (2024, 1, 15, 10, 30, 0, 0, 15, 0)
        mock_entry.updated = None
        mock_entry.updated_parsed = None
        mock_entry.author = "Test Author"
        mock_entry.id = "article-123"

        result = parse_feed_entry(mock_entry)

        assert result["title"] == "Test Article"
        assert result["link"] == "https://example.com/article"
        assert result["description"] == "Article description"
        assert result["published"] == "2024-01-15T10:30:00"
        assert result["author"] == "Test Author"
        assert result["id"] == "article-123"

    def test_parse_minimal_entry(self):
        """Test parsing an entry with minimal fields."""
        from rss_fetcher.main import parse_feed_entry

        mock_entry = MagicMock(spec=[])
        mock_entry.title = "Minimal Article"
        mock_entry.link = "https://example.com/minimal"

        # Use getattr default behavior
        result = parse_feed_entry(mock_entry)

        assert result["title"] == "Minimal Article"
        assert result["link"] == "https://example.com/minimal"


class TestFetchRssFeed:
    """Tests for fetch_rss_feed function."""

    def test_invalid_url_empty(self):
        """Test handling of empty URL."""
        from rss_fetcher.main import fetch_rss_feed

        mock_agent = MagicMock()
        mock_agent.write_log = MagicMock()

        result = fetch_rss_feed("", mock_agent)

        assert result["error"] is True
        assert result["error_type"] == "invalid_url"

    def test_invalid_url_no_protocol(self):
        """Test handling of URL without protocol."""
        from rss_fetcher.main import fetch_rss_feed

        mock_agent = MagicMock()
        mock_agent.write_log = MagicMock()

        result = fetch_rss_feed("example.com/feed.xml", mock_agent)

        assert result["error"] is True
        assert result["error_type"] == "invalid_url"
        assert "http://" in result["message"]

    @patch('rss_fetcher.main.feedparser.parse')
    def test_successful_fetch(self, mock_parse):
        """Test successful feed fetch and parse."""
        from rss_fetcher.main import fetch_rss_feed

        # Mock feed response
        mock_feed = MagicMock()
        mock_feed.bozo = False
        mock_feed.bozo_exception = None
        mock_feed.feed.title = "Test Feed"
        mock_feed.feed.link = "https://example.com"
        mock_feed.feed.description = "A test feed"

        mock_entry = MagicMock()
        mock_entry.title = "Test Entry"
        mock_entry.link = "https://example.com/entry"
        mock_entry.description = "Entry description"
        mock_entry.published_parsed = None
        mock_entry.updated_parsed = None
        mock_entry.author = ""
        mock_entry.id = "entry-1"

        mock_feed.entries = [mock_entry]
        mock_parse.return_value = mock_feed

        mock_agent = MagicMock()
        mock_agent.write_log = MagicMock()

        result = fetch_rss_feed("https://example.com/feed.xml", mock_agent)

        assert result["error"] is False
        assert result["url"] == "https://example.com/feed.xml"
        assert result["feed"]["title"] == "Test Feed"
        assert result["entry_count"] == 1
        assert len(result["entries"]) == 1
        assert result["entries"][0]["title"] == "Test Entry"

    @patch('rss_fetcher.main.feedparser.parse')
    def test_empty_feed(self, mock_parse):
        """Test handling of empty feed."""
        from rss_fetcher.main import fetch_rss_feed

        mock_feed = MagicMock()
        mock_feed.bozo = False
        mock_feed.bozo_exception = None
        mock_feed.entries = []
        mock_parse.return_value = mock_feed

        mock_agent = MagicMock()
        mock_agent.write_log = MagicMock()

        result = fetch_rss_feed("https://example.com/empty.xml", mock_agent)

        assert result["error"] is True
        assert result["error_type"] == "empty_feed"
