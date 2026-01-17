#!/usr/bin/env python3
"""
Link Content Scripter Agent - Fetches content from RSS entry links and generates news scripts.

This agent receives RSS feed data (from rss-fetcher), fetches the actual content
from each entry's link, and generates a news broadcast script based on the
fetched content.
"""
import json
import os
import re
from datetime import datetime
from typing import Any, Optional

import httpx
from bs4 import BeautifulSoup
from dotenv import load_dotenv

from mofa.agent_build.base.base_agent import MofaAgent, run_agent


# Default timeout for HTTP requests (seconds)
DEFAULT_TIMEOUT = 15

# Maximum content length to process (characters)
MAX_CONTENT_LENGTH = 5000


def create_error_response(error_type: str, message: str, details: dict = None) -> dict:
    """Create standardized error response."""
    return {
        "error": True,
        "error_type": error_type,
        "message": message,
        "details": details or {},
    }


def fetch_link_content(url: str) -> Optional[str]:
    """
    Fetch and extract main text content from a URL.

    Args:
        url: The URL to fetch

    Returns:
        Extracted text content or None if failed
    """
    if not url or not url.startswith(('http://', 'https://')):
        return None

    headers = {
        'User-Agent': 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36',
        'Accept': 'text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8',
        'Accept-Language': 'zh-CN,zh;q=0.9,en;q=0.8',
    }

    try:
        with httpx.Client(timeout=DEFAULT_TIMEOUT, follow_redirects=True) as client:
            response = client.get(url, headers=headers)
            response.raise_for_status()

            # Parse HTML
            soup = BeautifulSoup(response.text, 'lxml')

            # Remove script, style, nav, footer, header elements
            for element in soup(['script', 'style', 'nav', 'footer', 'header', 'aside', 'iframe', 'noscript']):
                element.decompose()

            # Try to find main content area
            main_content = None

            # Common content selectors
            content_selectors = [
                'article',
                'main',
                '[role="main"]',
                '.article-content',
                '.post-content',
                '.entry-content',
                '.content',
                '#content',
                '.article-body',
                '.story-body',
            ]

            for selector in content_selectors:
                main_content = soup.select_one(selector)
                if main_content:
                    break

            # Fallback to body if no main content found
            if not main_content:
                main_content = soup.body

            if not main_content:
                return None

            # Extract text
            text = main_content.get_text(separator='\n', strip=True)

            # Clean up text
            # Remove multiple newlines
            text = re.sub(r'\n{3,}', '\n\n', text)
            # Remove very short lines (likely navigation)
            lines = [line for line in text.split('\n') if len(line.strip()) > 20]
            text = '\n'.join(lines)

            # Truncate if too long
            if len(text) > MAX_CONTENT_LENGTH:
                text = text[:MAX_CONTENT_LENGTH] + '...'

            return text if text.strip() else None

    except httpx.TimeoutException:
        return None
    except httpx.HTTPStatusError:
        return None
    except Exception:
        return None


def call_llm(prompt: str, system_prompt: str = None) -> Optional[str]:
    """
    Call LLM API to generate content.

    Args:
        prompt: The user prompt
        system_prompt: Optional system prompt

    Returns:
        Generated text or None if failed
    """
    import openai

    api_key = os.getenv("LLM_API_KEY") or os.getenv("OPENAI_API_KEY")
    api_base = os.getenv("LLM_API_BASE") or os.getenv("OPENAI_API_BASE")
    model = os.getenv("LLM_MODEL", "gpt-4o-mini")

    if not api_key:
        return None

    client = openai.OpenAI(
        api_key=api_key,
        base_url=api_base if api_base else None
    )

    messages = []
    if system_prompt:
        messages.append({"role": "system", "content": system_prompt})
    messages.append({"role": "user", "content": prompt})

    response = client.chat.completions.create(
        model=model,
        messages=messages,
        temperature=0.7,
        max_tokens=512
    )

    return response.choices[0].message.content


def generate_script_from_content(entry: dict, content: str) -> dict:
    """
    Generate a news broadcast script from fetched content.

    Args:
        entry: RSS feed entry dict
        content: Fetched page content

    Returns:
        News script dict
    """
    title = entry.get("title", "")
    link = entry.get("link", "")
    published = entry.get("published", "")
    description = entry.get("description", "")

    system_prompt = """你是一位专业的新闻编辑，擅长将新闻素材改写为适合播报的新闻稿。
请保持客观、准确，基于提供的内容生成简短的新闻播报稿（3-5句话）。
不要编造任何内容，只基于提供的素材。"""

    prompt = f"""请基于以下新闻内容，生成一段简短的新闻播报稿（3-5句话，约100-150字）：

标题：{title}

原文内容摘要：
{content[:3000]}

{f"发布时间：{published}" if published else ""}

要求：
1. 保持新闻的核心信息
2. 使用适合口播的语言风格
3. 开头简洁引入主题
4. 3-5句话概括核心内容
5. 不要包含URL链接
6. 不要编造原文中没有的信息

请直接输出播报稿文本："""

    script_text = call_llm(prompt, system_prompt)

    if not script_text:
        # Fallback: use title and description
        script_text = f"今日要闻：{title}。{description[:200] if description else ''}"

    return {
        "id": entry.get("id", ""),
        "title": title,
        "script": script_text,
        "link": link,
        "published": published,
        "content_fetched": True,
        "content_length": len(content) if content else 0,
        "generated_at": datetime.utcnow().isoformat() + "Z",
    }


def generate_script_from_description(entry: dict) -> dict:
    """
    Generate a news script from entry description when link fetch fails.

    Args:
        entry: RSS feed entry dict

    Returns:
        News script dict
    """
    title = entry.get("title", "")
    description = entry.get("description", "")
    link = entry.get("link", "")
    published = entry.get("published", "")

    # Simple script from description
    script_text = f"今日要闻：{title}。"
    if description:
        # Clean HTML from description
        clean_desc = BeautifulSoup(description, 'lxml').get_text(strip=True)
        if clean_desc and len(clean_desc) > 20:
            script_text += f" {clean_desc[:200]}"

    return {
        "id": entry.get("id", ""),
        "title": title,
        "script": script_text,
        "link": link,
        "published": published,
        "content_fetched": False,
        "content_length": 0,
        "generated_at": datetime.utcnow().isoformat() + "Z",
    }


def process_entry(entry: dict) -> dict:
    """
    Process a single RSS entry: fetch link content and generate script.

    Args:
        entry: RSS feed entry dict

    Returns:
        News script dict
    """
    link = entry.get("link", "")

    # Try to fetch content from link
    content = fetch_link_content(link) if link else None

    if content and len(content) > 100:
        return generate_script_from_content(entry, content)
    else:
        return generate_script_from_description(entry)


def process_feed_to_scripts(feed_data: dict) -> dict:
    """
    Process RSS feed data and generate scripts for each entry.

    Args:
        feed_data: RSS feed data from rss-fetcher

    Returns:
        Dict with scripts collection or error
    """
    # Check for error in input
    if feed_data.get("error"):
        return feed_data

    entries = feed_data.get("entries", [])
    if not entries:
        return create_error_response(
            "empty_feed",
            "No entries found in feed data",
            {}
        )

    feed_info = feed_data.get("feed", {})
    feed_title = feed_info.get("title", "Unknown Feed")

    # Limit entries to process
    max_entries = int(os.getenv("MAX_ENTRIES", "10"))
    entries_to_process = entries[:max_entries]

    scripts = []
    fetch_success_count = 0
    fetch_fail_count = 0

    for entry in entries_to_process:
        script = process_entry(entry)
        scripts.append(script)

        if script.get("content_fetched"):
            fetch_success_count += 1
        else:
            fetch_fail_count += 1

    return {
        "error": False,
        "feed_title": feed_title,
        "feed_url": feed_data.get("url", ""),
        "scripts": scripts,
        "script_count": len(scripts),
        "fetch_success_count": fetch_success_count,
        "fetch_fail_count": fetch_fail_count,
        "generated_at": datetime.utcnow().isoformat() + "Z",
    }


def parse_input(input_data: Any) -> dict:
    """
    Parse input data to extract feed dict.

    Args:
        input_data: Raw input (string or dict)

    Returns:
        Feed data dict
    """
    if isinstance(input_data, str):
        try:
            return json.loads(input_data)
        except json.JSONDecodeError:
            return create_error_response(
                "parse_error",
                "Invalid JSON input",
                {}
            )
    return input_data


def process_request(input_data: Any) -> str:
    """
    Process the request and return JSON result.

    Args:
        input_data: Raw input data

    Returns:
        JSON string with result
    """
    # Load environment variables
    load_dotenv('.env.secret')

    feed_data = parse_input(input_data)

    if feed_data.get("error"):
        return json.dumps(feed_data, ensure_ascii=False)

    result = process_feed_to_scripts(feed_data)
    return json.dumps(result, ensure_ascii=False)


@run_agent
def run(agent: MofaAgent):
    """Main agent run loop."""
    agent.write_log("Link Content Scripter agent started")

    input_data = agent.receive_parameter('rss_feed')

    result_json = process_request(input_data)

    agent.send_output(agent_output_name='news_scripts', agent_result=result_json)

    agent.write_log("Link Content Scripter agent completed")


def main():
    """Main entry point."""
    agent = MofaAgent(agent_name="link-content-scripter", is_write_log=True)
    run(agent=agent)


if __name__ == "__main__":
    main()
