#!/usr/bin/env python3
"""
RSS Input Agent - Accepts RSS feed URLs for processing.

This agent serves as the entry point for the RSS to Multi-Newscaster Script dataflow.
It accepts RSS feed URLs from user input or environment variables and outputs them
in a structured JSON format for downstream processing.
"""
import argparse
import json
import os
import sys

import pyarrow as pa
from dora import Node


def clean_string(input_string: str) -> str:
    """Clean and validate input string."""
    return input_string.encode('utf-8', 'replace').decode('utf-8').strip()


def create_rss_request(urls: list, config: dict = None) -> dict:
    """
    Create RSSInput JSON structure per data-model.md.

    Args:
        urls: List of RSS feed URLs to process
        config: Optional persona configuration

    Returns:
        RSSInput dict with urls and optional config
    """
    return {
        "urls": urls,
        "config": config
    }


def send_request_and_receive_response(node: Node, urls: list, config: dict = None):
    """Send RSS request and wait for broadcast script response."""
    TIMEOUT = 300  # 5 minutes timeout for LLM processing

    # Create and send the RSS request
    rss_request = create_rss_request(urls, config)
    request_json = json.dumps(rss_request, ensure_ascii=False)
    node.send_output("rss_request", pa.array([request_json]))

    print(f"[rss-input] Sent request for {len(urls)} RSS feed(s)")
    print(f"[rss-input] Waiting for broadcast script...")

    # Wait for response
    event = node.next(timeout=TIMEOUT)
    if event is not None:
        while True:
            if event is not None and event["type"] == "INPUT":
                event_id = event["id"]
                if event_id == "broadcast_script":
                    result = event["value"].to_pylist()[0]
                    try:
                        script_data = json.loads(result)
                        print("\n" + "=" * 60)
                        print("BROADCAST SCRIPT GENERATED")
                        print("=" * 60)

                        # Check for error response
                        if script_data.get("error"):
                            print(f"\nError: {script_data.get('error_type')}")
                            print(f"Message: {script_data.get('message')}")
                        else:
                            # Print formatted script
                            print(f"\nTitle: {script_data.get('title', 'Untitled')}")
                            print(f"Generated: {script_data.get('generated_at', 'Unknown')}")
                            print(f"News Items: {script_data.get('news_item_count', 0)}")
                            print("\n" + "-" * 60 + "\n")

                            for segment in script_data.get("segments", []):
                                speaker_label = segment.get("speaker_label", "")
                                content = segment.get("content", "")
                                print(f"{speaker_label}{content}\n")

                        print("=" * 60)
                    except json.JSONDecodeError:
                        print(f"\n{result}")

                    break

            event = node.next(timeout=TIMEOUT)
            if event is None:
                print("[rss-input] Timeout waiting for response")
                break


def interactive_mode(node: Node):
    """Run in interactive mode, prompting user for RSS URLs."""
    print("\n" + "=" * 60)
    print("RSS to Multi-Newscaster Script Generator")
    print("=" * 60)
    print("Enter RSS feed URL(s) to generate a news script.")
    print("For multiple URLs, separate with commas.")
    print("Type 'quit' or 'exit' to stop.\n")

    while True:
        try:
            user_input = input("Enter RSS URL(s): ").strip()

            if user_input.lower() in ('quit', 'exit', 'q'):
                print("Goodbye!")
                break

            if not user_input:
                print("Please enter a valid RSS URL.")
                continue

            # Parse URLs (comma-separated)
            urls = [clean_string(url) for url in user_input.split(',') if url.strip()]

            if not urls:
                print("No valid URLs provided.")
                continue

            # Validate URLs have http/https prefix
            valid_urls = []
            for url in urls:
                if not url.startswith(('http://', 'https://')):
                    print(f"Warning: '{url}' doesn't look like a valid URL. Skipping.")
                else:
                    valid_urls.append(url)

            if not valid_urls:
                print("No valid URLs to process.")
                continue

            send_request_and_receive_response(node, valid_urls)
            print()  # Empty line before next prompt

        except KeyboardInterrupt:
            print("\nGoodbye!")
            break
        except EOFError:
            print("\nGoodbye!")
            break


def main():
    """Main entry point for RSS Input agent."""
    parser = argparse.ArgumentParser(
        description="RSS Input Agent - Accept RSS feed URLs for news script generation",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  Interactive mode:
    python -m rss_input --name rss-input

  With URL from environment:
    RSS_URLS="https://example.com/feed.xml" python -m rss_input --name rss-input

  With multiple URLs:
    RSS_URLS="https://feed1.xml,https://feed2.xml" python -m rss_input --name rss-input
        """
    )

    parser.add_argument(
        "--name",
        type=str,
        required=False,
        default="rss-input",
        help="The name of the node in the dataflow (default: rss-input)"
    )

    parser.add_argument(
        "--urls",
        type=str,
        required=False,
        default=None,
        help="Comma-separated RSS feed URLs to process"
    )

    args = parser.parse_args()

    # Check for URLs from environment or arguments
    urls_str = os.getenv("RSS_URLS", args.urls)

    # Initialize Dora node
    node = Node(args.name)

    if urls_str:
        # Non-interactive mode: process URLs and exit
        urls = [clean_string(url) for url in urls_str.split(',') if url.strip()]
        if urls:
            send_request_and_receive_response(node, urls)
        else:
            print("Error: No valid URLs provided")
            sys.exit(1)
    else:
        # Interactive mode
        interactive_mode(node)


if __name__ == "__main__":
    main()
