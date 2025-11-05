"""
Dora MiniMax T2A Node - Main entry point
High-quality text-to-speech using MiniMax WebSocket API.
"""

import time
import sys
import traceback
import json
import asyncio
import numpy as np
import pyarrow as pa
try:
    from mofa.agent_build.base.base_agent import MofaAgent
    MOFA_AVAILABLE = True
except ImportError:
    from dora import Node
    MOFA_AVAILABLE = False

from .config import MinimaxT2AConfig
from .minimax_client import MinimaxWebSocketClient


def send_log(node, level, message, config_level="INFO"):
    """Send log message through log output channel."""
    LOG_LEVELS = {"DEBUG": 10, "INFO": 20, "WARNING": 30, "ERROR": 40}

    if LOG_LEVELS.get(level, 0) < LOG_LEVELS.get(config_level, 20):
        return

    formatted_message = f"[{level}] {message}"
    # Also print to console so errors show in docker logs
    try:
        print(formatted_message, file=sys.stderr if level in {"ERROR", "WARNING"} else sys.stdout, flush=True)
    except Exception:
        pass

    log_data = {
        "node": "minimax-t2a",
        "level": level,
        "message": formatted_message,
        "timestamp": time.time(),
    }
    node.send_output("log", pa.array([json.dumps(log_data)]))


async def synthesize_segment(client: MinimaxWebSocketClient, text: str, node, segment_index: int, metadata: dict, config):
    """
    Synthesize a text segment and stream audio chunks.

    Returns:
        Tuple of (success: bool, fragment_count: int, total_duration: float)
    """
    fragment_num = 0
    total_audio_duration = 0.0

    # Batch chunks to avoid shared memory exhaustion (os error 24)
    # Accumulate audio until batch duration threshold is reached
    batch_duration_threshold = config.BATCH_DURATION_MS / 1000.0  # Convert ms to seconds
    chunk_buffer = []
    batch_accumulated_duration = 0.0
    batch_num = 0

    try:
        # Connect to WebSocket
        if not await client.connect():
            raise RuntimeError("Failed to connect to MiniMax WebSocket")

        # Start task
        if not await client.start_task():
            raise RuntimeError("Failed to start synthesis task")

        # Stream audio chunks
        async for sample_rate, audio_bytes in client.synthesize_streaming(text):
            fragment_num += 1

            # Convert PCM bytes to numpy float32 array
            # PCM format: 16-bit signed integers, need to normalize to [-1, 1]
            audio_int16 = np.frombuffer(audio_bytes, dtype=np.int16)
            audio_float32 = audio_int16.astype(np.float32) / 32768.0

            fragment_duration = len(audio_float32) / sample_rate
            total_audio_duration += fragment_duration

            # Add to buffer
            chunk_buffer.append(audio_float32)
            batch_accumulated_duration += fragment_duration

            # Send batch when accumulated duration exceeds threshold
            if batch_accumulated_duration >= batch_duration_threshold:
                batch_num += 1
                batched_audio = np.concatenate(chunk_buffer)
                # Calculate actual duration from concatenated array (more accurate)
                actual_batch_duration = len(batched_audio) / sample_rate

                node.send_output(
                    "audio",
                    pa.array([batched_audio]),
                    metadata={
                        "segment_index": segment_index,
                        "segments_remaining": metadata.get("segments_remaining", 0),
                        "question_id": metadata.get("question_id", "default"),
                        "fragment_num": batch_num,
                        "sample_rate": sample_rate,
                        "duration": actual_batch_duration,
                        "is_streaming": True,
                    },
                )
                # Reset buffer
                chunk_buffer = []
                batch_accumulated_duration = 0.0

        # Send remaining chunks in buffer
        if chunk_buffer:
            batch_num += 1
            batched_audio = np.concatenate(chunk_buffer)
            # Calculate actual duration from concatenated array (more accurate)
            actual_batch_duration = len(batched_audio) / sample_rate

            node.send_output(
                "audio",
                pa.array([batched_audio]),
                metadata={
                    "segment_index": segment_index,
                    "segments_remaining": metadata.get("segments_remaining", 0),
                    "question_id": metadata.get("question_id", "default"),
                    "fragment_num": batch_num,
                    "sample_rate": sample_rate,
                    "duration": actual_batch_duration,
                    "is_streaming": True,
                },
            )

        if fragment_num == 0:
            raise RuntimeError("No audio fragments received from MiniMax API")

        return True, batch_num, total_audio_duration

    finally:
        # Always close connection
        await client.close()


def main():
    """Main entry point for MiniMax T2A node"""

    if MOFA_AVAILABLE:
        agent = MofaAgent(agent_name="minimax-t2a", is_write_log=True)
        node = agent.node
    else:
        node = Node()
    config = MinimaxT2AConfig()

    # Validate configuration
    is_valid, error_msg = config.validate()
    if not is_valid:
        send_log(node, "ERROR", f"Configuration error: {error_msg}", config.LOG_LEVEL)
        send_log(node, "ERROR", "Node cannot start without valid configuration", config.LOG_LEVEL)
        return

    send_log(node, "INFO", "MiniMax T2A Node initialized", config.LOG_LEVEL)
    send_log(node, "INFO", f"Model: {config.MODEL}", config.LOG_LEVEL)
    send_log(node, "INFO", f"Voice ID: {config.VOICE_ID}", config.LOG_LEVEL)
    send_log(node, "INFO", f"Sample Rate: {config.SAMPLE_RATE} Hz", config.LOG_LEVEL)
    send_log(node, "INFO", f"Speed: {config.SPEED}x, Volume: {config.VOL}, Pitch: {config.PITCH}", config.LOG_LEVEL)
    send_log(node, "INFO", f"Batch Duration: {config.BATCH_DURATION_MS}ms (reduces shared memory usage)", config.LOG_LEVEL)

    # Statistics
    total_syntheses = 0
    total_duration = 0.0

    for event in node:
        send_log(node, "DEBUG", f"EVENT: type={event['type']}, id={event.get('id', 'N/A')}", config.LOG_LEVEL)

        if event["type"] == "INPUT":
            input_id = event["id"]

            if input_id == "text":
                # Get text to synthesize
                text = event["value"][0].as_py()
                metadata = event.get("metadata", {})
                segment_index = metadata.get("segment_index", -1)

                send_log(
                    node,
                    "DEBUG",
                    f"RECEIVED text: '{text}' (len={len(text)}, type={type(text).__name__})",
                    config.LOG_LEVEL,
                )

                # Skip if text is only punctuation or whitespace
                text_stripped = text.strip()
                if not text_stripped or all(c in "。！？.!?,，、；：\"\"''（）【】《》\n\r\t " for c in text_stripped):
                    send_log(node, "DEBUG", f"SKIPPED - text is only punctuation/whitespace: '{text}'", config.LOG_LEVEL)
                    node.send_output("segment_complete", pa.array(["skipped"]), metadata={})
                    continue

                send_log(node, "INFO", f"Processing segment {segment_index + 1} (len={len(text)})", config.LOG_LEVEL)

                # Create client for this synthesis (one connection per synthesis)
                client = MinimaxWebSocketClient(
                    config=config, logger_func=lambda level, msg: send_log(node, level, msg, config.LOG_LEVEL)
                )

                start_time = time.time()

                try:
                    # Run async synthesis (blocks main thread until complete)
                    success, fragment_count, audio_duration = asyncio.run(
                        synthesize_segment(client, text, node, segment_index, metadata, config)
                    )

                    synthesis_time = time.time() - start_time
                    total_syntheses += 1
                    total_duration += audio_duration

                    send_log(
                        node,
                        "INFO",
                        f"Streamed {fragment_count} batches, {audio_duration:.2f}s audio in {synthesis_time:.3f}s",
                        config.LOG_LEVEL,
                    )

                    # Send segment completion signal
                    node.send_output("segment_complete", pa.array(["completed"]), metadata={})
                    send_log(node, "INFO", f"Sent segment_complete for segment {segment_index + 1}", config.LOG_LEVEL)

                except Exception as e:
                    error_details = traceback.format_exc()
                    send_log(node, "ERROR", f"Synthesis error: {e}", config.LOG_LEVEL)
                    send_log(node, "ERROR", f"Traceback: {error_details}", config.LOG_LEVEL)

                    # Send error completion
                    node.send_output(
                        "segment_complete",
                        pa.array(["error"]),
                        metadata={"error": str(e), "error_stage": "synthesis"},
                    )
                    send_log(node, "ERROR", f"Sent error segment_complete for segment {segment_index + 1}", config.LOG_LEVEL)

            elif input_id == "control":
                # Handle control commands
                command = event["value"][0].as_py()

                if command == "reset":
                    send_log(node, "INFO", "[MiniMax T2A] RESET received", config.LOG_LEVEL)
                    send_log(node, "INFO", "[MiniMax T2A] Reset acknowledged", config.LOG_LEVEL)

                elif command == "stats":
                    send_log(node, "INFO", f"Total syntheses: {total_syntheses}", config.LOG_LEVEL)
                    send_log(node, "INFO", f"Total audio duration: {total_duration:.1f}s", config.LOG_LEVEL)

        elif event["type"] == "STOP":
            break

    send_log(node, "INFO", "MiniMax T2A node stopped", config.LOG_LEVEL)


if __name__ == "__main__":
    main()
