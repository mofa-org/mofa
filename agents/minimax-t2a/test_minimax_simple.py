#!/usr/bin/env python3
"""
Simple test script for MiniMax T2A WebSocket API.
Tests the basic functionality without Dora framework.
"""

import os
import sys
import asyncio
import numpy as np
import wave

# Add the module to path
sys.path.insert(0, os.path.dirname(__file__))

from dora_minimax_t2a.config import MinimaxT2AConfig
from dora_minimax_t2a.minimax_client import MinimaxWebSocketClient


async def test_synthesis(text: str, output_file: str = "test_output.wav"):
    """Test synthesis with MiniMax API"""

    # Create config
    config = MinimaxT2AConfig()

    # Validate config
    is_valid, error_msg = config.validate()
    if not is_valid:
        print(f"❌ Configuration error: {error_msg}")
        print(f"   Make sure MINIMAX_API_KEY is set in environment")
        return False

    print("✓ Configuration valid")
    print(f"  Model: {config.MODEL}")
    print(f"  Voice: {config.VOICE_ID}")
    print(f"  Sample Rate: {config.SAMPLE_RATE} Hz")
    print(f"  Speed: {config.SPEED}x\n")

    # Create client
    client = MinimaxWebSocketClient(
        config=config,
        logger_func=lambda level, msg: print(f"[{level}] {msg}")
    )

    try:
        print(f"Synthesizing: \"{text}\"\n")

        # Connect
        print("Connecting to MiniMax WebSocket...")
        if not await client.connect():
            print("❌ Connection failed")
            return False
        print("✓ Connected\n")

        # Start task
        print("Starting synthesis task...")
        if not await client.start_task():
            print("❌ Task start failed")
            return False
        print("✓ Task started\n")

        # Stream and collect audio
        print("Streaming audio chunks...")
        audio_chunks = []
        fragment_count = 0
        sample_rate = config.SAMPLE_RATE

        async for sr, audio_bytes in client.synthesize_streaming(text):
            fragment_count += 1
            sample_rate = sr

            # Convert PCM bytes to int16 numpy array
            audio_int16 = np.frombuffer(audio_bytes, dtype=np.int16)
            audio_chunks.append(audio_int16)

            print(f"  Chunk {fragment_count}: {len(audio_bytes)} bytes ({len(audio_int16)} samples)")

        if fragment_count == 0:
            print("❌ No audio chunks received")
            return False

        print(f"\n✓ Received {fragment_count} chunks\n")

        # Concatenate all chunks
        full_audio = np.concatenate(audio_chunks)
        duration = len(full_audio) / sample_rate

        print(f"Total audio: {len(full_audio)} samples, {duration:.2f} seconds\n")

        # Save to WAV file
        print(f"Saving to {output_file}...")
        with wave.open(output_file, 'wb') as wav_file:
            wav_file.setnchannels(config.AUDIO_CHANNEL)
            wav_file.setsampwidth(2)  # 16-bit
            wav_file.setframerate(sample_rate)
            wav_file.writeframes(full_audio.tobytes())

        print(f"✓ Audio saved to {output_file}\n")

        return True

    except Exception as e:
        print(f"❌ Error: {e}")
        import traceback
        traceback.print_exc()
        return False

    finally:
        # Close connection
        await client.close()
        print("Connection closed")


def main():
    """Main test function"""

    # Check for API key
    if not os.getenv("MINIMAX_API_KEY"):
        print("❌ Error: MINIMAX_API_KEY environment variable not set")
        print("\nUsage:")
        print("  export MINIMAX_API_KEY='your-api-key-here'")
        print("  python test_minimax_simple.py")
        return 1

    # Test text
    test_text = "Hello, this is a test of the MiniMax text-to-speech API. The quality should be very high."

    # Run test
    print("=" * 70)
    print("MiniMax T2A Simple Test")
    print("=" * 70)
    print()

    success = asyncio.run(test_synthesis(test_text))

    print("=" * 70)
    if success:
        print("✓ Test PASSED")
        print("\nYou can play the audio with:")
        print("  ffplay test_output.wav")
        print("  or")
        print("  mpv test_output.wav")
        return 0
    else:
        print("❌ Test FAILED")
        return 1


if __name__ == "__main__":
    sys.exit(main())
