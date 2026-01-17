"""Tests for anchor-assigner agent."""
import json
import pytest
from unittest.mock import MagicMock, patch


class TestCreateErrorResponse:
    """Tests for create_error_response function."""

    def test_basic_error(self):
        """Test basic error response creation."""
        from anchor_assigner.main import create_error_response

        error = create_error_response(
            "test_error",
            "Test error message"
        )

        assert error["error"] is True
        assert error["error_type"] == "test_error"
        assert error["message"] == "Test error message"


class TestGetAnchors:
    """Tests for get_anchors function."""

    def test_default_anchors(self):
        """Test default anchor configuration."""
        from anchor_assigner.main import get_anchors

        anchors = get_anchors()

        assert "male" in anchors
        assert "female" in anchors
        assert anchors["male"]["gender"] == "male"
        assert anchors["female"]["gender"] == "female"

    @patch.dict('os.environ', {'MALE_ANCHOR_NAME': '王刚', 'FEMALE_ANCHOR_NAME': '张丽'})
    def test_custom_anchors(self):
        """Test custom anchor names from env."""
        from anchor_assigner.main import get_anchors

        anchors = get_anchors()

        assert anchors["male"]["name"] == "王刚"
        assert anchors["female"]["name"] == "张丽"


class TestAssignAnchorsToScripts:
    """Tests for assign_anchors_to_scripts function."""

    def test_empty_scripts(self):
        """Test handling empty scripts."""
        from anchor_assigner.main import assign_anchors_to_scripts

        scripts_data = {
            "error": False,
            "scripts": [],
        }

        result = assign_anchors_to_scripts(scripts_data)

        assert result["error"] is True
        assert result["error_type"] == "empty_scripts"

    def test_error_passthrough(self):
        """Test that error input is passed through."""
        from anchor_assigner.main import assign_anchors_to_scripts

        error_input = {
            "error": True,
            "error_type": "fetch_error",
            "message": "Failed"
        }

        result = assign_anchors_to_scripts(error_input)

        assert result["error"] is True
        assert result["error_type"] == "fetch_error"

    def test_alternating_assignment(self):
        """Test that anchors alternate correctly."""
        from anchor_assigner.main import assign_anchors_to_scripts

        scripts_data = {
            "error": False,
            "feed_title": "Test Feed",
            "scripts": [
                {"id": "1", "title": "News 1", "script": "Script 1"},
                {"id": "2", "title": "News 2", "script": "Script 2"},
                {"id": "3", "title": "News 3", "script": "Script 3"},
                {"id": "4", "title": "News 4", "script": "Script 4"},
            ]
        }

        result = assign_anchors_to_scripts(scripts_data)

        assert result["error"] is False
        assert result["pair_count"] == 4

        # Check alternating pattern (female, male, female, male)
        assert result["pairs"][0]["anchor"]["gender"] == "female"
        assert result["pairs"][1]["anchor"]["gender"] == "male"
        assert result["pairs"][2]["anchor"]["gender"] == "female"
        assert result["pairs"][3]["anchor"]["gender"] == "male"

    def test_pair_structure(self):
        """Test that pairs have correct structure."""
        from anchor_assigner.main import assign_anchors_to_scripts

        scripts_data = {
            "error": False,
            "scripts": [
                {"id": "test-1", "title": "Test News", "script": "Test script content"}
            ]
        }

        result = assign_anchors_to_scripts(scripts_data)

        pair = result["pairs"][0]
        assert "script" in pair
        assert "script_id" in pair
        assert "title" in pair
        assert "anchor" in pair
        assert "position" in pair
        assert pair["position"] == 1


class TestParseInput:
    """Tests for parse_input function."""

    def test_parse_json_string(self):
        """Test parsing JSON string input."""
        from anchor_assigner.main import parse_input

        input_str = '{"scripts": [{"title": "Test"}]}'
        result = parse_input(input_str)

        assert result["scripts"][0]["title"] == "Test"

    def test_parse_dict_input(self):
        """Test passing dict directly."""
        from anchor_assigner.main import parse_input

        input_dict = {"scripts": [{"title": "Test"}]}
        result = parse_input(input_dict)

        assert result["scripts"][0]["title"] == "Test"

    def test_parse_invalid_json(self):
        """Test handling invalid JSON."""
        from anchor_assigner.main import parse_input

        result = parse_input("not valid json")

        assert result["error"] is True
        assert result["error_type"] == "parse_error"
