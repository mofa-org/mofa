#!/usr/bin/env python3
"""
RSS Fetcher Agent - Fetches and parses RSS feeds from URLs.

This agent accepts an RSS feed URL as input, fetches the feed content,
parses it using feedparser, and outputs the structured feed data as JSON.
"""
import json
from datetime import datetime
from typing import Any

import feedparser

from mofa.agent_build.base.base_agent import MofaAgent, run_agent


def create_error_response(error_type: str, message: str, details: dict = None) -> dict:
    """Create standardized error response."""
    return {
        "error": True,
        "error_type": error_type,
        "message": message,
        "details": details or {},
    }


def parse_feed_entry(entry: Any) -> dict:
    """
    Parse a single feed entry into a structured dict.

    Args:
        entry: A feedparser entry object

    Returns:
        Structured dict with entry data
    """
    # Extract published date
    published = None
    if hasattr(entry, 'published_parsed') and entry.published_parsed:
        try:
            published = datetime(*entry.published_parsed[:6]).isoformat()
        except (TypeError, ValueError):
            published = getattr(entry, 'published', None)
    elif hasattr(entry, 'published'):
        published = entry.published

    # Extract updated date
    updated = None
    if hasattr(entry, 'updated_parsed') and entry.updated_parsed:
        try:
            updated = datetime(*entry.updated_parsed[:6]).isoformat()
        except (TypeError, ValueError):
            updated = getattr(entry, 'updated', None)
    elif hasattr(entry, 'updated'):
        updated = entry.updated

    return {
        "title": getattr(entry, 'title', ''),
        "link": getattr(entry, 'link', ''),
        "description": getattr(entry, 'description', getattr(entry, 'summary', '')),
        "published": published,
        "updated": updated,
        "author": getattr(entry, 'author', ''),
        "id": getattr(entry, 'id', getattr(entry, 'link', '')),
    }


def fetch_rss_feed(url: str) -> dict:
    """
    Fetch and parse an RSS feed from the given URL.

    Args:
        url: The RSS feed URL to fetch

    Returns:
        Parsed feed data as a dict
    """
    # Validate URL
    if not url or not isinstance(url, str):
        return create_error_response(
            "invalid_url",
            "URL must be a non-empty string",
            {"provided_url": str(url)}
        )

    url = url.strip()
    if not url.startswith(('http://', 'https://')):
        return create_error_response(
            "invalid_url",
            "URL must start with http:// or https://",
            {"provided_url": url}
        )

    # Fetch and parse the feed
    try:
        feed = feedparser.parse(url)
    except Exception as e:
        return create_error_response(
            "fetch_error",
            f"Failed to fetch feed: {str(e)}",
            {"url": url}
        )

    # Check for feed errors
    if feed.bozo and feed.bozo_exception:
        error_msg = str(feed.bozo_exception)
        # Some bozo exceptions are recoverable (e.g., CharacterEncodingOverride)
        if not feed.entries:
            return create_error_response(
                "parse_error",
                f"Failed to parse feed: {error_msg}",
                {"url": url}
            )

    # Check if feed has entries
    if not feed.entries:
        return create_error_response(
            "empty_feed",
            "Feed contains no entries",
            {"url": url}
        )

    # Extract feed metadata
    feed_info = feed.feed
    feed_title = getattr(feed_info, 'title', 'Unknown Feed')
    feed_link = getattr(feed_info, 'link', url)
    feed_description = getattr(feed_info, 'description', getattr(feed_info, 'subtitle', ''))

    # Parse all entries
    entries = [parse_feed_entry(entry) for entry in feed.entries]

    return {
        "error": False,
        "url": url,
        "feed": {
            "title": feed_title,
            "link": feed_link,
            "description": feed_description,
        },
        "entries": entries,
        "entry_count": len(entries),
        "fetched_at": datetime.utcnow().isoformat() + "Z",
    }


def parse_input_url(input_data: Any) -> str:
    """
    Parse input data to extract URL string.

    Args:
        input_data: Raw input (string or JSON)

    Returns:
        URL string
    """
    if isinstance(input_data, str):
        try:
            parsed = json.loads(input_data)
            if isinstance(parsed, dict) and 'url' in parsed:
                return parsed['url']
            return input_data
        except json.JSONDecodeError:
            return input_data
    return str(input_data)


def process_rss_request(input_data: Any) -> str:
    """
    Process RSS request and return JSON result.

    Args:
        input_data: Raw input data

    Returns:
        JSON string with result
    """
    url = parse_input_url(input_data)
    result = fetch_rss_feed(url)
    return json.dumps(result, ensure_ascii=False)


@run_agent
def run(agent: MofaAgent):
    """Main agent run loop."""
    agent.write_log("RSS Fetcher agent started")

    input_data = agent.receive_parameter('rss_url')

    result_json = process_rss_request(input_data)

    agent.send_output(agent_output_name='rss_feed', agent_result=result_json)

    agent.write_log("RSS Fetcher agent completed")


def main():
    """Main entry point."""
    agent = MofaAgent(agent_name="rss-fetcher", is_write_log=True)
    run(agent=agent)


if __name__ == "__main__":
    main()
