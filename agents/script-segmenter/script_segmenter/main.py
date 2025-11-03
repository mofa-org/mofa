#!/usr/bin/env python3
"""
Script Segmenter - Parse markdown and orchestrate TTS generation.
Streams text segments sequentially to downstream TTS nodes and
includes intelligent text segmentation for long passages.
"""
import argparse
import json
import time
import os
import re
from typing import Dict, List, Optional, Tuple, Iterable
import pyarrow as pa
from mofa.agent_build.base.base_agent import MofaAgent, run_agent


def send_log(node, level, message, config_level="INFO"):
    """Send log message through log output channel."""
    LOG_LEVELS = {"DEBUG": 10, "INFO": 20, "WARNING": 30, "ERROR": 40}

    if LOG_LEVELS.get(level, 0) < LOG_LEVELS.get(config_level, 20):
        return

    formatted_message = f"[{level}] {message}"
    log_data = {
        "node": "script-segmenter",
        "level": level,
        "message": formatted_message,
        "timestamp": time.time()
    }
    node.node.send_output("log", pa.array([json.dumps(log_data)]))


def find_split_index(text: str, max_length: int, split_marks: Iterable[str]) -> int:
    """Find a split index at or before max_length using provided marks or whitespace."""
    if max_length <= 0:
        return -1

    limit = min(len(text), max_length)

    # Try to split at punctuation marks first
    if split_marks:
        for idx in range(limit, 0, -1):
            if text[idx - 1] in split_marks:
                return idx

    # Fall back to whitespace
    for idx in range(limit, 0, -1):
        if text[idx - 1].isspace():
            return idx

    return -1


def split_long_text(
    text: str,
    max_length: int,
    punctuation_marks: str,
    node=None,
    log_level: str = "INFO",
) -> List[str]:
    """Split long text into chunks that respect sentence boundaries."""
    if max_length <= 0 or len(text) <= max_length:
        return [text]

    split_marks = set(punctuation_marks)

    chunks: List[str] = []
    remainder = text

    if node:
        send_log(
            node,
            "DEBUG",
            f"Splitting long text (len={len(text)}) with max={max_length} chars",
            log_level,
        )

    while remainder:
        if len(remainder) <= max_length:
            chunks.append(remainder)
            break

        # Find best split point
        split_idx = find_split_index(remainder, max_length, split_marks)
        if split_idx == -1:
            # No good split point found, force split at max_length
            split_idx = max_length

        chunk = remainder[:split_idx].strip()
        if chunk:
            chunks.append(chunk)
            if node:
                send_log(
                    node,
                    "DEBUG",
                    f"Created chunk: '{chunk[:30]}...' (len={len(chunk)})",
                    log_level,
                )

        remainder = remainder[split_idx:].lstrip()

    return chunks


def parse_markdown(file_path):
    """Parse markdown and extract segments for supported characters."""
    segments: List[Tuple[str, str]] = []
    current_character: Optional[str] = None
    current_text: List[str] = []

    # Map markdown tag content to our channel identifiers
    character_aliases: Dict[str, str] = {
        "大牛": "daniu",
        "一帆": "yifan",
        "博宇": "boyu",
        "Boyu": "boyu",
        "boyu": "boyu",
    }

    def finalize_segment() -> None:
        nonlocal current_character, current_text
        if current_character and current_text:
            combined_text = " ".join(current_text).strip()
            if combined_text:
                segments.append((current_character, combined_text))
        current_text = []

    with open(file_path, 'r', encoding='utf-8') as f:
        for raw_line in f:
            line = raw_line.strip()

            # Skip empty lines and headers
            if not line or line.startswith('#'):
                continue

            # Detect character tags inside the line
            match = re.search(r'【([^】]+)】', line)
            if match:
                finalize_segment()

                tag_content = match.group(1).strip()
                character = character_aliases.get(tag_content)
                if not character:
                    # Unknown character tag; skip this section entirely
                    current_character = None
                    current_text = []
                    continue

                # Extract the text following the tag, removing markdown markers
                remainder = line.split('】', 1)[1] if '】' in line else ''
                remainder = remainder.lstrip('*').strip()

                current_character = character
                current_text = [remainder] if remainder else []
                continue

            if current_character:
                # Continue accumulating text for current character
                clean_line = line.strip('*').strip()
                if clean_line:
                    current_text.append(clean_line)

        finalize_segment()

    return segments


@run_agent
def run(agent: MofaAgent):
    parser = argparse.ArgumentParser()
    parser.add_argument('--input-file', required=True, help='Path to markdown script file')
    args = parser.parse_args()

    log_level = os.getenv("LOG_LEVEL", "INFO")

    # Text segmentation configuration
    max_segment_duration = float(os.getenv("MAX_SEGMENT_DURATION", "10.0"))
    chars_per_second = float(os.getenv("TTS_CHARS_PER_SECOND", "4.5"))
    max_segment_length = int(max_segment_duration * chars_per_second)

    # Punctuation marks for intelligent splitting
    punctuation_marks = os.getenv("PUNCTUATION_MARKS", "。！？.!?，,、；;：:")

    send_log(agent, "INFO", "Script Segmenter started", log_level)
    send_log(
        agent,
        "INFO",
        f"Text segmentation config: max_duration={max_segment_duration}s, "
        f"chars_per_second={chars_per_second}, max_length={max_segment_length} chars, "
        f"punctuation='{punctuation_marks}'",
        log_level
    )

    # Parse script
    try:
        segments = parse_markdown(args.input_file)
        send_log(agent, "INFO", f"Loaded {len(segments)} segments from {args.input_file}", log_level)
    except Exception as e:
        send_log(agent, "ERROR", f"Failed to parse script: {e}", log_level)
        return

    if not segments:
        send_log(agent, "ERROR", "No segments found in script!", log_level)
        return

    # Apply text segmentation to split long segments
    expanded_segments = []
    for character, text in segments:
        # Check if text is too long
        if len(text) > max_segment_length:
            send_log(
                agent,
                "INFO",
                f"Splitting long segment for {character} (len={len(text)}) into chunks",
                log_level
            )
            # Split into smaller chunks
            chunks = split_long_text(text, max_segment_length, punctuation_marks, agent, log_level)
            send_log(
                agent,
                "INFO",
                f"Split into {len(chunks)} chunks: {[len(c) for c in chunks]}",
                log_level
            )
            for chunk in chunks:
                expanded_segments.append((character, chunk))
        else:
            expanded_segments.append((character, text))

    segments = expanded_segments
    send_log(
        agent,
        "INFO",
        f"After text segmentation: {len(segments)} total segments to process",
        log_level
    )

    # State
    current_index = 0
    waiting_for = None

    # Send first segment BEFORE entering event loop
    character, text = segments[0]
    send_log(agent, "INFO", f"Sending segment 1/{len(segments)} to {character}: '{text[:50]}...' (len={len(text)})", log_level)
    agent.node.send_output(f'{character}_text', pa.array([text]))
    waiting_for = character
    current_index = 1
    send_log(agent, "INFO", f"Waiting for {waiting_for}_segment_complete...", log_level)

    # Event loop - wait for segment_complete and send next segments
    send_log(agent, "INFO", "Entering event loop", log_level)
    for event in agent.node:
        send_log(agent, "DEBUG", f"Received event type: {event['type']}", log_level)

        if event["type"] == "INPUT":
            event_id = event["id"]
            send_log(agent, "DEBUG", f"Received INPUT event: {event_id}", log_level)

            if event_id == f"{waiting_for}_segment_complete":
                send_log(agent, "INFO", f"Received {event_id}", log_level)

                if current_index < len(segments):
                    character, text = segments[current_index]
                    send_log(agent, "INFO", f"Sending segment {current_index+1}/{len(segments)} to {character}: '{text[:50]}...' (len={len(text)})", log_level)
                    agent.node.send_output(f'{character}_text', pa.array([text]))
                    waiting_for = character
                    current_index += 1
                else:
                    send_log(agent, "INFO", "All segments processed. Sending script_complete.", log_level)
                    agent.node.send_output('script_complete', pa.array([b'']))
                    send_log(agent, "INFO", "Script segmenter finished", log_level)
                    break

        elif event["type"] == "STOP":
            send_log(agent, "INFO", "Received STOP event", log_level)
            break

    send_log(agent, "INFO", "Event loop exited", log_level)


def main():
    agent = MofaAgent(agent_name="script-segmenter", is_write_log=True)
    run(agent=agent)


if __name__ == "__main__":
    main()
