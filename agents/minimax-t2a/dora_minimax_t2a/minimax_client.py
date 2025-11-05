"""
MiniMax T2A WebSocket client for async text-to-speech synthesis.
"""

import asyncio
import json
import ssl
from typing import AsyncGenerator, Tuple, Callable, Optional
import websockets

from .config import MinimaxT2AConfig


class MinimaxWebSocketClient:
    """Async WebSocket client for MiniMax T2A API"""

    def __init__(self, config: MinimaxT2AConfig, logger_func: Optional[Callable] = None):
        """
        Initialize MiniMax WebSocket client.

        Args:
            config: Configuration object
            logger_func: Optional logging function with signature (level: str, message: str)
        """
        self.config = config
        self.logger = logger_func or self._default_logger
        self.url = "wss://api.minimax.io/ws/v1/t2a_v2"
        self.ws = None

    def _default_logger(self, level: str, message: str):
        """Default logger that prints to stdout"""
        print(f"[{level}] {message}")

    async def connect(self) -> bool:
        """
        Establish WebSocket connection.

        Returns:
            True if connection successful, False otherwise
        """
        headers = {"Authorization": f"Bearer {self.config.API_KEY}"}

        ssl_context = ssl.create_default_context()
        ssl_context.check_hostname = False
        ssl_context.verify_mode = ssl.CERT_NONE

        try:
            self.ws = await websockets.connect(self.url, additional_headers=headers, ssl=ssl_context)
            connected = json.loads(await self.ws.recv())
            if connected.get("event") == "connected_success":
                self.logger("DEBUG", "MiniMax WebSocket connected successfully")
                return True
            else:
                self.logger("ERROR", f"Unexpected connection response: {connected}")
                return False
        except Exception as e:
            self.logger("ERROR", f"Connection failed: {e}")
            self.ws = None
            return False

    async def start_task(self) -> bool:
        """
        Send task start request.

        Returns:
            True if task started successfully, False otherwise
        """
        if not self.ws:
            self.logger("ERROR", "Cannot start task: not connected")
            return False

        start_msg = {
            "event": "task_start",
            "model": self.config.MODEL,
            "voice_setting": {
                "voice_id": self.config.VOICE_ID,
                "speed": self.config.SPEED,
                "vol": self.config.VOL,
                "pitch": self.config.PITCH,
                "english_normalization": self.config.ENABLE_ENGLISH_NORMALIZATION,
            },
            "audio_setting": {
                "sample_rate": self.config.SAMPLE_RATE,
                "bitrate": self.config.AUDIO_BITRATE,
                "format": self.config.AUDIO_FORMAT,
                "channel": self.config.AUDIO_CHANNEL,
            },
        }

        try:
            await self.ws.send(json.dumps(start_msg))
            response = json.loads(await self.ws.recv())
            if response.get("event") == "task_started":
                self.logger("DEBUG", "Task started successfully")
                return True
            else:
                self.logger("ERROR", f"Unexpected task_start response: {response}")
                return False
        except Exception as e:
            self.logger("ERROR", f"Failed to start task: {e}")
            return False

    async def synthesize_streaming(self, text: str) -> AsyncGenerator[Tuple[int, bytes], None]:
        """
        Synthesize text and stream audio chunks.

        Args:
            text: Text to synthesize

        Yields:
            Tuple of (sample_rate, audio_bytes)
        """
        if not self.ws:
            self.logger("ERROR", "Cannot synthesize: not connected")
            return

        # Send task_continue with text
        try:
            await self.ws.send(json.dumps({"event": "task_continue", "text": text}))
        except Exception as e:
            self.logger("ERROR", f"Failed to send task_continue: {e}")
            return

        chunk_counter = 0

        # Stream audio chunks
        while True:
            try:
                response = json.loads(await self.ws.recv())

                # Check for audio data
                if "data" in response and "audio" in response["data"]:
                    audio_hex = response["data"]["audio"]
                    if audio_hex:
                        chunk_counter += 1
                        audio_bytes = bytes.fromhex(audio_hex)
                        self.logger("DEBUG", f"Received audio chunk #{chunk_counter}, size: {len(audio_bytes)} bytes")
                        yield (self.config.SAMPLE_RATE, audio_bytes)

                # Check if synthesis is complete
                if response.get("is_final"):
                    self.logger("DEBUG", f"Synthesis completed: {chunk_counter} chunks total")
                    break

            except Exception as e:
                self.logger("ERROR", f"Error during streaming: {e}")
                break

    async def close(self):
        """Close WebSocket connection"""
        if self.ws:
            try:
                await self.ws.send(json.dumps({"event": "task_finish"}))
                await self.ws.close()
                self.logger("DEBUG", "WebSocket connection closed")
            except Exception as e:
                self.logger("ERROR", f"Error closing connection: {e}")
            finally:
                self.ws = None
