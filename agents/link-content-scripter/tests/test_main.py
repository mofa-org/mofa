"""Tests for link-content-scripter agent."""
import json
import pytest
from unittest.mock import MagicMock, patch


class TestCreateErrorResponse:
    """Tests for create_error_response function."""

    def test_basic_error(self):
        """Test basic error response creation."""
        from link_content_scripter.main import create_error_response

        error = create_error_response(
            "test_error",
            "Test error message"
        )

        assert error["error"] is True
        assert error["error_type"] == "test_error"
        assert error["message"] == "Test error message"

    def test_error_with_details(self):
        """Test error response with details."""
        from link_content_scripter.main import create_error_response

        error = create_error_response(
            "fetch_error",
            "Failed to fetch",
            {"url": "https://example.com"}
        )

        assert error["details"]["url"] == "https://example.com"


class TestFetchLinkContent:
    """Tests for fetch_link_content function."""

    def test_invalid_url(self):
        """Test handling invalid URL."""
        from link_content_scripter.main import fetch_link_content

        result = fetch_link_content("")
        assert result is None

        result = fetch_link_content("not-a-url")
        assert result is None

    @patch('link_content_scripter.main.httpx.Client')
    def test_successful_fetch(self, mock_client_class):
        """Test successful content fetch."""
        from link_content_scripter.main import fetch_link_content

        mock_response = MagicMock()
        mock_response.text = """
        <html>
        <body>
            <article>
                <p>This is a test article with enough content to pass the length check.</p>
                <p>It contains multiple paragraphs of meaningful text.</p>
            </article>
        </body>
        </html>
        """
        mock_response.raise_for_status = MagicMock()

        mock_client = MagicMock()
        mock_client.get.return_value = mock_response
        mock_client.__enter__ = MagicMock(return_value=mock_client)
        mock_client.__exit__ = MagicMock(return_value=False)
        mock_client_class.return_value = mock_client

        result = fetch_link_content("https://example.com/article")

        assert result is not None
        assert "test article" in result.lower()


class TestGenerateScriptFromDescription:
    """Tests for generate_script_from_description function."""

    def test_generate_from_description(self):
        """Test script generation from description."""
        from link_content_scripter.main import generate_script_from_description

        entry = {
            "id": "test-id",
            "title": "Test News Title",
            "description": "This is a test news description with some content.",
            "link": "https://example.com/news",
            "published": "2024-01-15"
        }

        result = generate_script_from_description(entry)

        assert result["id"] == "test-id"
        assert result["title"] == "Test News Title"
        assert "Test News Title" in result["script"]
        assert result["content_fetched"] is False


class TestProcessFeedToScripts:
    """Tests for process_feed_to_scripts function."""

    def test_empty_feed(self):
        """Test handling empty feed."""
        from link_content_scripter.main import process_feed_to_scripts

        feed_data = {
            "error": False,
            "entries": [],
            "feed": {"title": "Test Feed"}
        }

        result = process_feed_to_scripts(feed_data)

        assert result["error"] is True
        assert result["error_type"] == "empty_feed"

    def test_error_passthrough(self):
        """Test that error input is passed through."""
        from link_content_scripter.main import process_feed_to_scripts

        error_input = {
            "error": True,
            "error_type": "fetch_error",
            "message": "Failed to fetch feed"
        }

        result = process_feed_to_scripts(error_input)

        assert result["error"] is True
        assert result["error_type"] == "fetch_error"

    @patch('link_content_scripter.main.fetch_link_content')
    @patch('link_content_scripter.main.call_llm')
    def test_successful_processing(self, mock_llm, mock_fetch):
        """Test successful script generation."""
        from link_content_scripter.main import process_feed_to_scripts

        mock_fetch.return_value = "This is the fetched article content with enough text."
        mock_llm.return_value = "今日要闻：测试新闻内容。这是一条重要消息。"

        feed_data = {
            "error": False,
            "url": "https://example.com/feed",
            "feed": {"title": "Test Feed"},
            "entries": [
                {
                    "id": "entry-1",
                    "title": "Test News",
                    "description": "Test description",
                    "link": "https://example.com/news",
                    "published": "2024-01-15T10:00:00"
                }
            ]
        }

        result = process_feed_to_scripts(feed_data)

        assert result["error"] is False
        assert result["feed_title"] == "Test Feed"
        assert result["script_count"] == 1
        assert len(result["scripts"]) == 1


class TestParseInput:
    """Tests for parse_input function."""

    def test_parse_json_string(self):
        """Test parsing JSON string input."""
        from link_content_scripter.main import parse_input

        input_str = '{"entries": [{"title": "Test"}]}'
        result = parse_input(input_str)

        assert result["entries"][0]["title"] == "Test"

    def test_parse_dict_input(self):
        """Test passing dict directly."""
        from link_content_scripter.main import parse_input

        input_dict = {"entries": [{"title": "Test"}]}
        result = parse_input(input_dict)

        assert result["entries"][0]["title"] == "Test"

    def test_parse_invalid_json(self):
        """Test handling invalid JSON."""
        from link_content_scripter.main import parse_input

        result = parse_input("not valid json")

        assert result["error"] is True
        assert result["error_type"] == "parse_error"
