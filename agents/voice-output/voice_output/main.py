#!/usr/bin/env python3
"""
Voice output agent for the MoFA podcast generator flow.
Concatenates incoming audio streams, adds speaker-switch silences,
and persists the final WAV.
"""

import argparse
import json
import os
import random
import time
from pathlib import Path
from typing import Dict, List, Optional

import numpy as np
import pyarrow as pa
from scipy.io import wavfile

from mofa.agent_build.base.base_agent import MofaAgent, run_agent


def send_log(agent: MofaAgent, level: str, message: str, config_level: str = "INFO") -> None:
    """Send structured logs through the node output."""
    log_levels = {"DEBUG": 10, "INFO": 20, "WARNING": 30, "ERROR": 40}
    if log_levels.get(level, 0) < log_levels.get(config_level, 20):
        return

    formatted = f"[{level}] {message}"
    log_payload = {
        "node": agent.agent_name,
        "level": level,
        "message": formatted,
        "timestamp": time.time(),
    }
    agent.node.send_output("log", pa.array([json.dumps(log_payload)]))


def _ensure_output_directory(output_file: Path, agent: MofaAgent, log_level: str) -> None:
    """Create the output directory when needed."""
    try:
        output_file.parent.mkdir(parents=True, exist_ok=True)
    except Exception as exc:
        send_log(
            agent,
            "ERROR",
            f"Failed to create output directory '{output_file.parent}': {exc}",
            log_level,
        )
        raise


@run_agent
def run(agent: MofaAgent, output_file: Path, log_level: str) -> None:
    sample_rate = int(os.getenv("VOICE_OUTPUT_SAMPLE_RATE", "32000"))
    silence_min = float(os.getenv("VOICE_OUTPUT_SILENCE_MIN", "0.3"))
    silence_max = float(os.getenv("VOICE_OUTPUT_SILENCE_MAX", "1.2"))

    speakers: Dict[str, str] = {
        "daniu": "大牛",
        "yifan": "一帆",
        "boyu": "博宇",
    }

    audio_suffix = "_audio"
    segment_complete_suffix = "_segment_complete"

    audio_buffer: List[np.ndarray] = []
    last_speaker: Optional[str] = None
    segment_count = 0

    _ensure_output_directory(output_file, agent, log_level)
    send_log(agent, "INFO", f"Voice output initialized → {output_file}", log_level)
    send_log(
        agent,
        "INFO",
        f"Sample rate {sample_rate} Hz, silence window {silence_min}-{silence_max}s",
        log_level,
    )

    def process_audio_event(speaker_key: str, event) -> None:
        nonlocal last_speaker, segment_count

        speaker_name = speakers[speaker_key]
        metadata = event.get("metadata", {}) or {}
        fragment_num = metadata.get("fragment_num")
        is_segment_start = fragment_num == 1 if fragment_num is not None else True

        if is_segment_start and last_speaker is not None and last_speaker != speaker_key:
            silence_duration = random.uniform(silence_min, silence_max)
            silence_samples = int(sample_rate * silence_duration)
            silence = np.zeros(silence_samples, dtype=np.int16)
            audio_buffer.append(silence)
            previous_name = speakers.get(last_speaker, last_speaker)
            send_log(
                agent,
                "INFO",
                f"Added {silence_duration:.2f}s silence ({previous_name} → {speaker_name})",
                log_level,
            )

        if is_segment_start:
            last_speaker = speaker_key

        raw_value = event.get("value")
        if not raw_value:
            send_log(agent, "WARNING", f"No audio payload received from {speaker_name}", log_level)
            return

        audio_data = raw_value[0].as_py()
        if not isinstance(audio_data, np.ndarray):
            audio_data = np.array(audio_data, dtype=np.float32)

        original_dtype = audio_data.dtype
        if audio_data.dtype in (np.float32, np.float64):
            audio_data = (audio_data * 32767).astype(np.int16)
        elif audio_data.dtype != np.int16:
            send_log(
                agent,
                "WARNING",
                f"Unexpected dtype {original_dtype} from {speaker_name}, casting to int16",
                log_level,
            )
            audio_data = audio_data.astype(np.int16)

        audio_buffer.append(audio_data)
        segment_count += 1
        fragment_label = fragment_num if fragment_num is not None else "?"
        send_log(
            agent,
            "INFO",
            f"✓ Audio from {speaker_name} (fragment #{fragment_label}, {len(audio_data)} samples)",
            log_level,
        )

    for event in agent.node:
        if event["type"] == "INPUT":
            event_id = event["id"]

            if event_id.endswith(audio_suffix):
                speaker_key = event_id[: -len(audio_suffix)]
                if speaker_key in speakers:
                    process_audio_event(speaker_key, event)

            elif event_id.endswith(segment_complete_suffix):
                speaker_key = event_id[: -len(segment_complete_suffix)]
                if speaker_key in speakers:
                    last_speaker = speaker_key
                    send_log(
                        agent,
                        "DEBUG",
                        f"{speakers[speaker_key]} segment complete, awaiting next speaker",
                        log_level,
                    )

            elif event_id == "script_complete":
                send_log(
                    agent,
                    "INFO",
                    f"Script complete after {segment_count} fragments. Concatenating…",
                    log_level,
                )

                if not audio_buffer:
                    send_log(agent, "ERROR", "No audio received; skipping file write.", log_level)
                else:
                    try:
                        for idx, array in enumerate(audio_buffer):
                            if array.dtype != np.int16:
                                audio_buffer[idx] = array.astype(np.int16)

                        final_audio = np.concatenate(audio_buffer)
                        wavfile.write(output_file, sample_rate, final_audio)

                        try:
                            os.sync()
                        except AttributeError:
                            pass

                        duration = len(final_audio) / sample_rate
                        file_size = output_file.stat().st_size
                        send_log(
                            agent,
                            "INFO",
                            f"Podcast saved → {output_file} ({duration:.2f}s, {file_size} bytes)",
                            log_level,
                        )
                    except Exception as exc:
                        send_log(agent, "ERROR", f"Failed to write audio: {exc}", log_level)

                time.sleep(0.1)
                send_log(agent, "INFO", "Voice output agent exiting", log_level)
                break

        elif event["type"] == "STOP":
            send_log(agent, "INFO", "Received STOP event", log_level)
            break


def _parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="MoFA podcast voice output agent")
    parser.add_argument(
        "--output-file",
        default=os.getenv("PODCAST_OUTPUT_FILE", "out/podcast_output.wav"),
        help="Path to the output WAV file",
    )
    return parser.parse_args()


def main() -> None:
    args = _parse_args()
    output_path = Path(args.output_file).expanduser().resolve()
    log_level = os.getenv("LOG_LEVEL", "INFO")

    agent = MofaAgent(agent_name="voice-output", is_write_log=True)
    run(agent=agent, output_file=output_path, log_level=log_level)


if __name__ == "__main__":
    main()
