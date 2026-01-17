#!/usr/bin/env python3
"""
News Processor Agent - Fetches and parses RSS feeds.

This agent receives RSS feed URLs, fetches the content, parses it using feedparser,
and outputs structured NewsItem entities for downstream script generation.
"""
import hashlib
import json
import os
from datetime import datetime
from typing import Optional

import feedparser
import pyarrow as pa
from mofa.agent_build.base.base_agent import MofaAgent, run_agent


def create_error_response(error_type: str, message: str, details: dict = None) -> dict:
    """
    Create ErrorResponse per data-model.md.

    Args:
        error_type: One of feed_fetch_error, feed_parse_error, empty_feed_error
        message: Human-readable error message
        details: Optional additional context

    Returns:
        ErrorResponse dict
    """
    return {
        "error": True,
        "error_type": error_type,
        "message": message,
        "details": details or {},
        "partial_result": None
    }


def generate_item_id(item: dict, feed_url: str) -> str:
    """Generate unique ID for a news item."""
    # Use GUID if available, otherwise hash link or title
    if item.get('id'):
        return item['id']
    elif item.get('link'):
        return hashlib.md5(item['link'].encode()).hexdigest()[:16]
    elif item.get('title'):
        return hashlib.md5(f"{feed_url}:{item['title']}".encode()).hexdigest()[:16]
    else:
        return hashlib.md5(str(datetime.now()).encode()).hexdigest()[:16]


def parse_published_date(item: dict) -> Optional[str]:
    """Parse publication date from feed item."""
    # feedparser provides parsed time tuples
    if item.get('published_parsed'):
        try:
            dt = datetime(*item['published_parsed'][:6])
            return dt.isoformat() + "Z"
        except (TypeError, ValueError):
            pass

    if item.get('updated_parsed'):
        try:
            dt = datetime(*item['updated_parsed'][:6])
            return dt.isoformat() + "Z"
        except (TypeError, ValueError):
            pass

    # Fallback to raw string
    return item.get('published') or item.get('updated')


def extract_news_item(item: dict, feed_url: str, feed_title: str) -> dict:
    """
    Extract NewsItem from feedparser entry per data-model.md.

    Args:
        item: feedparser entry dict
        feed_url: Source RSS feed URL
        feed_title: Title of the feed

    Returns:
        NewsItem dict
    """
    # Get description - try multiple fields
    description = ""
    if item.get('summary'):
        description = item['summary']
    elif item.get('description'):
        description = item['description']
    elif item.get('content'):
        # content is a list of dicts
        for content in item['content']:
            if content.get('value'):
                description = content['value']
                break

    # Clean HTML tags from description (basic cleaning)
    import re
    description = re.sub(r'<[^>]+>', '', description).strip()

    return {
        "id": generate_item_id(item, feed_url),
        "title": item.get('title', '').strip(),
        "description": description,
        "published_date": parse_published_date(item),
        "source": item.get('author') or feed_title,
        "link": item.get('link', ''),
        "feed_url": feed_url
    }


def fetch_and_parse_feed(url: str, agent: MofaAgent) -> tuple:
    """
    Fetch and parse a single RSS feed.

    Args:
        url: RSS feed URL
        agent: MofaAgent for logging

    Returns:
        Tuple of (feed_data, error_response)
        If successful, feed_data contains parsed feed, error_response is None
        If failed, feed_data is None, error_response contains error details
    """
    agent.write_log(f"Fetching RSS feed: {url}")

    try:
        feed = feedparser.parse(url)
    except Exception as e:
        agent.write_log(f"Feed fetch exception: {str(e)}", level="ERROR")
        return None, create_error_response(
            "feed_fetch_error",
            f"Failed to fetch RSS feed: {str(e)}",
            {"url": url}
        )

    # Check for HTTP errors
    if hasattr(feed, 'status') and feed.status >= 400:
        agent.write_log(f"Feed fetch HTTP error: {feed.status}", level="ERROR")
        return None, create_error_response(
            "feed_fetch_error",
            f"HTTP error {feed.status} when fetching feed",
            {"url": url, "status": feed.status}
        )

    # Check for bozo (parse errors)
    if feed.bozo and not feed.entries:
        agent.write_log(f"Feed parse error: {feed.bozo_exception}", level="ERROR")
        return None, create_error_response(
            "feed_parse_error",
            f"Failed to parse RSS feed: {str(feed.bozo_exception)}",
            {"url": url}
        )

    # Check for empty feed
    if not feed.entries:
        agent.write_log("Feed is empty", level="WARNING")
        return None, create_error_response(
            "empty_feed_error",
            "RSS feed contains no items",
            {"url": url}
        )

    return feed, None


def deduplicate_items(items: list) -> list:
    """
    Remove duplicate news items based on title similarity.

    Args:
        items: List of NewsItem dicts

    Returns:
        Deduplicated list of NewsItem dicts
    """
    seen_titles = set()
    unique_items = []

    for item in items:
        # Normalize title for comparison
        title = item.get('title', '').strip().lower()

        # Skip if we've seen a very similar title
        if title and title not in seen_titles:
            seen_titles.add(title)
            unique_items.append(item)

    return unique_items


def process_rss_request(rss_request: dict, agent: MofaAgent) -> dict:
    """
    Process RSS request and return ProcessedFeed or ErrorResponse.

    Supports multiple RSS feed URLs with deduplication.

    Args:
        rss_request: RSSInput dict with urls and optional config
        agent: MofaAgent for logging

    Returns:
        ProcessedFeed or ErrorResponse dict
    """
    urls = rss_request.get("urls", [])

    if not urls:
        return create_error_response(
            "feed_fetch_error",
            "No RSS feed URLs provided",
            {}
        )

    all_items = []
    all_errors = []
    feed_titles = []
    source_feeds = []
    successful_feeds = 0

    # Process all URLs (supports multiple feeds)
    for url in urls:
        agent.write_log(f"Processing feed {len(source_feeds) + 1}/{len(urls)}: {url}")

        feed, error = fetch_and_parse_feed(url, agent)

        if error:
            # For multiple feeds, log error but continue with others
            if len(urls) > 1:
                agent.write_log(f"Skipping failed feed: {error.get('message')}", level="WARNING")
                all_errors.append(f"Feed {url}: {error.get('message')}")
                continue
            else:
                # Single feed - return error immediately
                return error

        # Extract feed metadata
        feed_title = feed.feed.get('title', 'Unknown Feed')
        feed_titles.append(feed_title)
        source_feeds.append(url)
        successful_feeds += 1

        # Extract news items
        agent.write_log(f"Processing {len(feed.entries)} items from {feed_title}")

        for entry in feed.entries:
            try:
                news_item = extract_news_item(entry, url, feed_title)

                # Validate required fields
                if news_item.get('title'):
                    all_items.append(news_item)
                else:
                    agent.write_log(f"Skipping item without title", level="WARNING")

            except Exception as e:
                agent.write_log(f"Error extracting item: {str(e)}", level="WARNING")
                all_errors.append(f"Error extracting item from {feed_title}: {str(e)}")

    # Check if we got any items
    if not all_items:
        return create_error_response(
            "empty_feed_error",
            "No valid news items could be extracted from any feed",
            {"urls": urls, "errors": all_errors}
        )

    # Deduplicate items from multiple feeds
    if len(urls) > 1:
        original_count = len(all_items)
        all_items = deduplicate_items(all_items)
        if original_count > len(all_items):
            agent.write_log(f"Deduplicated: {original_count} -> {len(all_items)} items")

    # Sort by published date (newest first) if available
    all_items.sort(
        key=lambda x: x.get('published_date') or '',
        reverse=True
    )

    # Create combined feed title for multiple feeds
    if len(feed_titles) > 1:
        combined_title = f"Combined Feed ({len(feed_titles)} sources)"
    else:
        combined_title = feed_titles[0] if feed_titles else "Unknown Feed"

    # Create ProcessedFeed response
    processed_feed = {
        "feed_title": combined_title,
        "feed_url": source_feeds[0] if source_feeds else urls[0],
        "source_feeds": source_feeds,  # All successful feed URLs
        "items": all_items,
        "item_count": len(all_items),
        "processed_at": datetime.utcnow().isoformat() + "Z",
        "errors": all_errors if all_errors else [],
        "config": rss_request.get("config")  # Pass through config for script-generator
    }

    agent.write_log(f"Successfully processed {len(all_items)} news items from {successful_feeds} feed(s)")

    return processed_feed


@run_agent
def run(agent: MofaAgent):
    """Main agent run loop."""
    agent.write_log("News Processor agent started")

    # Use receive_parameter to block and wait for input
    request_json = agent.receive_parameter('rss_request')

    try:
        # Parse incoming RSS request
        if isinstance(request_json, str):
            rss_request = json.loads(request_json)
        else:
            rss_request = request_json

        agent.write_log(f"Received RSS request for {len(rss_request.get('urls', []))} URL(s)")

        # Process the request
        result = process_rss_request(rss_request, agent)

        # Send output using agent's send_output method
        result_json = json.dumps(result, ensure_ascii=False)
        agent.send_output(agent_output_name='processed_feed', agent_result=result_json)

        if result.get("error"):
            agent.write_log(f"Error response sent: {result.get('error_type')}", level="ERROR")
        else:
            agent.write_log(f"Processed feed sent with {result.get('item_count')} items")

    except json.JSONDecodeError as e:
        agent.write_log(f"Invalid JSON in request: {str(e)}", level="ERROR")
        error_response = create_error_response(
            "feed_parse_error",
            f"Invalid JSON in request: {str(e)}",
            {}
        )
        agent.send_output(agent_output_name='processed_feed', agent_result=json.dumps(error_response))

    except Exception as e:
        agent.write_log(f"Unexpected error: {str(e)}", level="ERROR")
        error_response = create_error_response(
            "feed_fetch_error",
            f"Unexpected error: {str(e)}",
            {}
        )
        agent.send_output(agent_output_name='processed_feed', agent_result=json.dumps(error_response))

    agent.write_log("News Processor agent completed request")


def main():
    """Main entry point."""
    agent = MofaAgent(agent_name="news-processor", is_write_log=True)
    run(agent=agent)


if __name__ == "__main__":
    main()
