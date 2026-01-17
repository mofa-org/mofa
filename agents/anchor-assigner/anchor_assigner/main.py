#!/usr/bin/env python3
"""
Anchor Assigner Agent - Assigns news scripts to male and female anchors.

This agent takes news scripts JSON and assigns each script to either
a male or female anchor, alternating between them. Outputs a list of
(script, anchor) pairs.
"""
import json
import os
from datetime import datetime
from typing import Any

from mofa.agent_build.base.base_agent import MofaAgent, run_agent


# Default anchor configurations
DEFAULT_ANCHORS = {
    "male": {
        "id": "male_anchor",
        "name": os.getenv("MALE_ANCHOR_NAME", "张明"),
        "gender": "male",
        "role": "男主播",
    },
    "female": {
        "id": "female_anchor",
        "name": os.getenv("FEMALE_ANCHOR_NAME", "李华"),
        "gender": "female",
        "role": "女主播",
    }
}


def create_error_response(error_type: str, message: str, details: dict = None) -> dict:
    """Create standardized error response."""
    return {
        "error": True,
        "error_type": error_type,
        "message": message,
        "details": details or {},
    }


def get_anchors() -> dict:
    """
    Get anchor configurations from environment variables.

    Returns:
        Dict with male and female anchor configs
    """
    return {
        "male": {
            "id": "male_anchor",
            "name": os.getenv("MALE_ANCHOR_NAME", DEFAULT_ANCHORS["male"]["name"]),
            "gender": "male",
            "role": "男主播",
        },
        "female": {
            "id": "female_anchor",
            "name": os.getenv("FEMALE_ANCHOR_NAME", DEFAULT_ANCHORS["female"]["name"]),
            "gender": "female",
            "role": "女主播",
        }
    }


def assign_anchors_to_scripts(scripts_data: dict) -> dict:
    """
    Assign anchors to scripts, alternating between male and female.

    Args:
        scripts_data: News scripts data from link-content-scripter or feed-to-scripts

    Returns:
        Dict with list of (script, anchor) pairs
    """
    # Check for error in input
    if scripts_data.get("error"):
        return scripts_data

    scripts = scripts_data.get("scripts", [])
    if not scripts:
        return create_error_response(
            "empty_scripts",
            "No scripts found in input data",
            {}
        )

    anchors = get_anchors()
    anchor_order = ["female", "male"]  # Start with female anchor

    assigned_pairs = []

    for i, script in enumerate(scripts):
        # Alternate between anchors
        anchor_key = anchor_order[i % 2]
        anchor = anchors[anchor_key]

        pair = {
            "script": script.get("script", ""),
            "script_id": script.get("id", f"script_{i}"),
            "title": script.get("title", ""),
            "anchor": anchor,
            "position": i + 1,
        }
        assigned_pairs.append(pair)

    return {
        "error": False,
        "feed_title": scripts_data.get("feed_title", ""),
        "feed_url": scripts_data.get("feed_url", ""),
        "pairs": assigned_pairs,
        "pair_count": len(assigned_pairs),
        "anchors": anchors,
        "assigned_at": datetime.utcnow().isoformat() + "Z",
    }


def parse_input(input_data: Any) -> dict:
    """
    Parse input data to extract scripts dict.

    Args:
        input_data: Raw input (string or dict)

    Returns:
        Scripts data dict
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
    scripts_data = parse_input(input_data)

    if scripts_data.get("error"):
        return json.dumps(scripts_data, ensure_ascii=False)

    result = assign_anchors_to_scripts(scripts_data)
    return json.dumps(result, ensure_ascii=False)


@run_agent
def run(agent: MofaAgent):
    """Main agent run loop."""
    agent.write_log("Anchor Assigner agent started")

    input_data = agent.receive_parameter('news_scripts')

    result_json = process_request(input_data)

    agent.send_output(agent_output_name='anchor_pairs', agent_result=result_json)

    agent.write_log("Anchor Assigner agent completed")


def main():
    """Main entry point."""
    agent = MofaAgent(agent_name="anchor-assigner", is_write_log=True)
    run(agent=agent)


if __name__ == "__main__":
    main()
