#!/usr/bin/env python3
"""
Script Generator Agent - Generates multi-persona news scripts using LLM.

This agent receives processed news feed data and uses an LLM to generate
broadcast-ready scripts featuring three distinct newscaster personas:
- Male anchor: Clear, authoritative news delivery
- Female anchor: Engaging, personable news delivery
- Commentator: Analytical, providing context and expert perspective
"""
import json
import os
import uuid
from datetime import datetime
from typing import Optional

import pyarrow as pa
from dotenv import load_dotenv
from mofa.agent_build.base.base_agent import MofaAgent, run_agent


# Default persona configurations
DEFAULT_PERSONAS = {
    "male_anchor": {
        "id": "male_anchor",
        "name": os.getenv("MALE_ANCHOR_NAME", "张明"),
        "role": "男主播",
        "style": os.getenv("MALE_ANCHOR_STYLE", "清晰、权威的新闻播报"),
        "focus": ""
    },
    "female_anchor": {
        "id": "female_anchor",
        "name": os.getenv("FEMALE_ANCHOR_NAME", "李华"),
        "role": "女主播",
        "style": os.getenv("FEMALE_ANCHOR_STYLE", "亲和、引人入胜的新闻播报"),
        "focus": ""
    },
    "commentator": {
        "id": "commentator",
        "name": os.getenv("COMMENTATOR_NAME", "王教授"),
        "role": "资深评论员",
        "style": os.getenv("COMMENTATOR_STYLE", "分析性、提供背景和专家视角"),
        "focus": os.getenv("COMMENTATOR_FOCUS", "")
    }
}


def create_error_response(error_type: str, message: str, details: dict = None) -> dict:
    """Create ErrorResponse per data-model.md."""
    return {
        "error": True,
        "error_type": error_type,
        "message": message,
        "details": details or {},
        "partial_result": None
    }


def get_personas(config: dict = None) -> dict:
    """
    Get persona configurations, with environment variable and config overrides.

    Args:
        config: Optional PersonaConfig dict from RSSInput

    Returns:
        Dict of persona configurations
    """
    personas = {}
    config = config or {}

    for key, default in DEFAULT_PERSONAS.items():
        # Start with environment variable overrides
        persona = {
            "id": default["id"],
            "name": os.getenv(f"{key.upper()}_NAME", default["name"]),
            "role": default["role"],
            "style": os.getenv(f"{key.upper()}_STYLE", default["style"]),
            "focus": os.getenv(f"{key.upper()}_FOCUS", default.get("focus", ""))
        }

        # Apply config overrides if provided (PersonaOverride from RSSInput)
        persona_override = config.get(key, {})
        if persona_override:
            if persona_override.get("name"):
                persona["name"] = persona_override["name"]
            if persona_override.get("style"):
                persona["style"] = persona_override["style"]
            if persona_override.get("focus"):
                persona["focus"] = persona_override["focus"]

        personas[key] = persona

    return personas


def validate_config(config: dict) -> tuple:
    """
    Validate PersonaConfig structure.

    Args:
        config: PersonaConfig dict to validate

    Returns:
        Tuple of (is_valid, error_message)
    """
    if not config:
        return True, None

    if not isinstance(config, dict):
        return False, "Config must be a dictionary"

    # Validate tone if present
    valid_tones = ["formal", "casual", "neutral"]
    if "tone" in config and config["tone"] not in valid_tones:
        return False, f"Invalid tone '{config['tone']}'. Must be one of: {valid_tones}"

    # Validate persona overrides
    valid_personas = ["male_anchor", "female_anchor", "commentator"]
    for key in config:
        if key in valid_personas:
            override = config[key]
            if not isinstance(override, dict):
                return False, f"Persona override for '{key}' must be a dictionary"

    return True, None


def build_script_prompt(processed_feed: dict, personas: dict) -> str:
    """
    Build the LLM prompt for generating a multi-persona news script.

    Args:
        processed_feed: ProcessedFeed dict with news items
        personas: Dict of persona configurations

    Returns:
        Prompt string for LLM
    """
    # Get broadcast tone
    tone = os.getenv("BROADCAST_TONE", "formal")
    tone_desc = {
        "formal": "正式、严肃的新闻播报风格",
        "casual": "轻松、亲切的早间节目风格",
        "neutral": "中性、客观的新闻播报风格"
    }.get(tone, "正式、严肃的新闻播报风格")

    # Check if we have multiple sources
    source_feeds = processed_feed.get("source_feeds", [])
    is_multi_source = len(source_feeds) > 1

    # Build source info for prompt
    source_info = ""
    if is_multi_source:
        source_info = f"\n本期新闻综合了 {len(source_feeds)} 个新闻来源。"

    # Format news items for prompt
    news_items_text = ""
    for i, item in enumerate(processed_feed.get("items", [])[:20], 1):  # Limit to 20 items
        # Include feed URL for multi-source identification
        feed_source = ""
        if is_multi_source and item.get('feed_url'):
            feed_source = f"\n来源订阅: {item.get('feed_url')}"

        news_items_text += f"""
新闻 {i}:
标题: {item.get('title', '')}
摘要: {item.get('description', '无摘要')}
来源: {item.get('source', '未知')}{feed_source}
发布时间: {item.get('published_date', '未知')}
---
"""

    # Build prompt
    prompt = f"""你是一位专业的新闻稿件编辑。请根据以下新闻内容，生成一份适合三位新闻主播的播出稿件。

## 主播角色

1. **{personas['male_anchor']['name']}** ({personas['male_anchor']['role']})
   - 风格: {personas['male_anchor']['style']}
   - 标签: 【{personas['male_anchor']['name']}】

2. **{personas['female_anchor']['name']}** ({personas['female_anchor']['role']})
   - 风格: {personas['female_anchor']['style']}
   - 标签: 【{personas['female_anchor']['name']}】

3. **{personas['commentator']['name']}** ({personas['commentator']['role']})
   - 风格: {personas['commentator']['style']}
   - 标签: 【{personas['commentator']['name']}】
   {f"- 关注领域: {personas['commentator']['focus']}" if personas['commentator']['focus'] else ""}

## 播报风格
{tone_desc}{source_info}

## 新闻内容
{news_items_text}

## 要求

1. 以开场白开始，由两位主播轮流介绍
2. 每条新闻由主播播报事实，评论员提供分析
3. 在发言者之间添加自然的过渡语
4. 保持每位主播独特的说话风格
5. 结尾要有完整的收尾语
6. **重要**: 不要编造任何新闻内容，只基于提供的新闻素材

## 输出格式

请直接输出稿件文本，每段以发言者标签开头，例如：

【{personas['male_anchor']['name']}】各位观众朋友，大家好...

【{personas['female_anchor']['name']}】欢迎收看今天的新闻...

【{personas['commentator']['name']}】关于这条新闻，我想补充几点...

请生成完整的新闻播报稿件：
"""

    return prompt


def parse_script_segments(script_text: str, personas: dict) -> list:
    """
    Parse LLM output into ScriptSegment list per data-model.md.

    Args:
        script_text: Raw script text from LLM
        personas: Dict of persona configurations

    Returns:
        List of ScriptSegment dicts
    """
    import re

    segments = []
    position = 0

    # Build pattern to match speaker labels
    speaker_names = [p["name"] for p in personas.values()]
    pattern = r'【(' + '|'.join(re.escape(name) for name in speaker_names) + r')】'

    # Split by speaker labels
    parts = re.split(f'({pattern})', script_text)

    current_speaker = None
    current_content = []

    for part in parts:
        part = part.strip()
        if not part:
            continue

        # Check if this is a speaker label
        match = re.match(pattern, f'【{part}】')
        if match or part in speaker_names:
            # Save previous segment if exists
            if current_speaker and current_content:
                position += 1
                content = ' '.join(current_content).strip()
                if content:
                    # Determine persona ID from name
                    persona_id = None
                    for pid, pdata in personas.items():
                        if pdata["name"] == current_speaker:
                            persona_id = pid
                            break

                    if persona_id:
                        segments.append({
                            "position": position,
                            "speaker": persona_id,
                            "speaker_label": f"【{current_speaker}】",
                            "content": content,
                            "news_item_id": None,
                            "segment_type": determine_segment_type(content, position, persona_id)
                        })

            # Start new segment
            current_speaker = part
            current_content = []
        else:
            # Add to current content
            if current_speaker:
                current_content.append(part)

    # Don't forget last segment
    if current_speaker and current_content:
        position += 1
        content = ' '.join(current_content).strip()
        if content:
            persona_id = None
            for pid, pdata in personas.items():
                if pdata["name"] == current_speaker:
                    persona_id = pid
                    break

            if persona_id:
                segments.append({
                    "position": position,
                    "speaker": persona_id,
                    "speaker_label": f"【{current_speaker}】",
                    "content": content,
                    "news_item_id": None,
                    "segment_type": determine_segment_type(content, position, persona_id)
                })

    return segments


def determine_segment_type(content: str, position: int, persona_id: str) -> str:
    """Determine the segment type based on content and context."""
    content_lower = content.lower()

    # Check for intro patterns
    intro_keywords = ["欢迎", "大家好", "观众朋友", "收看", "开始"]
    if position <= 2 and any(kw in content for kw in intro_keywords):
        return "intro"

    # Check for outro patterns
    outro_keywords = ["感谢收看", "再见", "下期", "明天见", "晚安", "再会"]
    if any(kw in content for kw in outro_keywords):
        return "outro"

    # Commentator segments are analysis
    if persona_id == "commentator":
        return "analysis"

    # Check for transition patterns
    transition_keywords = ["接下来", "下面", "让我们", "现在"]
    if any(kw in content for kw in transition_keywords) and len(content) < 100:
        return "transition"

    # Default to news
    return "news"


def call_llm(prompt: str, agent: MofaAgent) -> Optional[str]:
    """
    Call LLM API to generate script.

    Args:
        prompt: The prompt for script generation
        agent: MofaAgent for logging

    Returns:
        Generated script text or None if failed
    """
    import openai

    api_key = os.getenv("LLM_API_KEY")
    api_base = os.getenv("LLM_API_BASE")
    model = os.getenv("LLM_MODEL", "gpt-4o")

    if not api_key:
        agent.write_log("LLM_API_KEY not set", level="ERROR")
        return None

    try:
        client = openai.OpenAI(
            api_key=api_key,
            base_url=api_base if api_base else None
        )

        agent.write_log(f"Calling LLM ({model}) for script generation...")

        response = client.chat.completions.create(
            model=model,
            messages=[
                {"role": "system", "content": "你是一位专业的新闻稿件编辑，擅长为多位主播编写播出稿件。"},
                {"role": "user", "content": prompt}
            ],
            temperature=0.7,
            max_tokens=4096
        )

        script_text = response.choices[0].message.content
        agent.write_log(f"LLM response received ({len(script_text)} chars)")

        return script_text

    except openai.APIError as e:
        agent.write_log(f"LLM API error: {str(e)}", level="ERROR")
        return None
    except Exception as e:
        agent.write_log(f"LLM call failed: {str(e)}", level="ERROR")
        return None


def generate_broadcast_script(processed_feed: dict, agent: MofaAgent, config: dict = None) -> dict:
    """
    Generate BroadcastScript from ProcessedFeed.

    Args:
        processed_feed: ProcessedFeed dict with news items
        agent: MofaAgent for logging
        config: Optional PersonaConfig from RSSInput

    Returns:
        BroadcastScript or ErrorResponse dict
    """
    # Check for error in input
    if processed_feed.get("error"):
        return processed_feed  # Pass through error

    # Validate config if provided
    is_valid, error_msg = validate_config(config)
    if not is_valid:
        agent.write_log(f"Invalid config: {error_msg}", level="ERROR")
        return create_error_response(
            "config_error",
            f"Invalid configuration: {error_msg}",
            {"config": config}
        )

    # Apply tone from config if provided
    if config and config.get("tone"):
        os.environ["BROADCAST_TONE"] = config["tone"]

    # Get personas with config overrides
    personas = get_personas(config)

    # Build prompt
    prompt = build_script_prompt(processed_feed, personas)

    # Call LLM
    script_text = call_llm(prompt, agent)

    if not script_text:
        return create_error_response(
            "llm_error",
            "Failed to generate script from LLM",
            {"feed_title": processed_feed.get("feed_title")}
        )

    # Parse script into segments
    segments = parse_script_segments(script_text, personas)

    if not segments:
        # Fallback: return raw script as single segment
        agent.write_log("Failed to parse segments, using raw output", level="WARNING")
        segments = [{
            "position": 1,
            "speaker": "male_anchor",
            "speaker_label": f"【{personas['male_anchor']['name']}】",
            "content": script_text,
            "news_item_id": None,
            "segment_type": "news"
        }]

    # Get source feeds - use source_feeds array if available, fallback to feed_url
    source_feeds = processed_feed.get("source_feeds", [])
    if not source_feeds and processed_feed.get("feed_url"):
        source_feeds = [processed_feed.get("feed_url")]

    # Build BroadcastScript response
    broadcast_script = {
        "id": f"script-{datetime.utcnow().strftime('%Y%m%d%H%M%S')}-{uuid.uuid4().hex[:8]}",
        "title": f"新闻播报 - {datetime.utcnow().strftime('%Y-%m-%d')}",
        "generated_at": datetime.utcnow().isoformat() + "Z",
        "segments": segments,
        "segment_count": len(segments),
        "source_feeds": source_feeds,  # All input RSS feed URLs
        "news_item_count": processed_feed.get("item_count", 0),
        "personas": list(personas.values()),
        "metadata": {
            "feed_title": processed_feed.get("feed_title"),
            "tone": os.getenv("BROADCAST_TONE", "formal"),
            "multi_source": len(source_feeds) > 1
        }
    }

    agent.write_log(f"Generated broadcast script with {len(segments)} segments")

    return broadcast_script


@run_agent
def run(agent: MofaAgent):
    """Main agent run loop."""
    # Load environment variables
    load_dotenv('.env.secret')

    agent.write_log("Script Generator agent started")

    # Use receive_parameter to block and wait for input
    feed_json = agent.receive_parameter('processed_feed')

    try:
        # Parse incoming processed feed
        if isinstance(feed_json, str):
            processed_feed = json.loads(feed_json)
        else:
            processed_feed = feed_json

        agent.write_log(f"Received processed feed: {processed_feed.get('feed_title', 'Unknown')}")

        # Generate broadcast script
        result = generate_broadcast_script(processed_feed, agent)

        # Send output using agent's send_output method
        result_json = json.dumps(result, ensure_ascii=False)
        agent.send_output(agent_output_name='broadcast_script', agent_result=result_json)

        if result.get("error"):
            agent.write_log(f"Error response sent: {result.get('error_type')}", level="ERROR")
        else:
            agent.write_log(f"Broadcast script sent with {result.get('segment_count')} segments")

    except json.JSONDecodeError as e:
        agent.write_log(f"Invalid JSON in input: {str(e)}", level="ERROR")
        error_response = create_error_response(
            "llm_error",
            f"Invalid JSON in input: {str(e)}",
            {}
        )
        agent.send_output(agent_output_name='broadcast_script', agent_result=json.dumps(error_response))

    except Exception as e:
        agent.write_log(f"Unexpected error: {str(e)}", level="ERROR")
        error_response = create_error_response(
            "llm_error",
            f"Unexpected error: {str(e)}",
            {}
        )
        agent.send_output(agent_output_name='broadcast_script', agent_result=json.dumps(error_response))

    agent.write_log("Script Generator agent completed request")


def main():
    """Main entry point."""
    agent = MofaAgent(agent_name="script-generator", is_write_log=True)
    run(agent=agent)


if __name__ == "__main__":
    main()
