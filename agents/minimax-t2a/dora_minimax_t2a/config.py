"""
Configuration for MiniMax T2A TTS node.
"""

import os
from dataclasses import dataclass


@dataclass
class MinimaxT2AConfig:
    """MiniMax T2A configuration class"""

    # API Configuration
    API_KEY: str = os.getenv("MINIMAX_API_KEY", "")
    MODEL: str = os.getenv("MINIMAX_MODEL", "speech-2.5-hd-preview")

    # Voice Configuration
    VOICE_ID: str = os.getenv("MINIMAX_VOICE_ID", "male-qn-qingse")
    SPEED: float = float(os.getenv("MINIMAX_SPEED", "1.0"))
    VOL: float = float(os.getenv("MINIMAX_VOL", "1.0"))
    PITCH: int = int(os.getenv("MINIMAX_PITCH", "0"))

    # Audio Configuration
    SAMPLE_RATE: int = int(os.getenv("SAMPLE_RATE", "32000"))
    AUDIO_BITRATE: int = int(os.getenv("AUDIO_BITRATE", "128000"))
    AUDIO_FORMAT: str = "pcm"  # Always PCM for streaming
    AUDIO_CHANNEL: int = int(os.getenv("AUDIO_CHANNEL", "1"))

    # Processing Configuration
    ENABLE_ENGLISH_NORMALIZATION: bool = os.getenv("ENABLE_ENGLISH_NORMALIZATION", "false").lower() == "true"
    TEXT_LANG: str = os.getenv("TEXT_LANG", "auto")  # Kept for compatibility, not used by MiniMax

    # Batching Configuration (to avoid shared memory exhaustion)
    BATCH_DURATION_MS: int = int(os.getenv("BATCH_DURATION_MS", "500"))  # Accumulate audio chunks into batches of this duration

    # Logging
    LOG_LEVEL: str = os.getenv("LOG_LEVEL", "INFO").upper()

    def validate(self) -> tuple[bool, str]:
        """Validate configuration. Returns (is_valid, error_message)"""
        if not self.API_KEY:
            return False, "MINIMAX_API_KEY environment variable is required"

        if self.SAMPLE_RATE not in [8000, 16000, 24000, 32000]:
            return False, f"SAMPLE_RATE must be 8000, 16000, 24000, or 32000, got {self.SAMPLE_RATE}"

        if self.SPEED < 0.5 or self.SPEED > 2.0:
            return False, f"MINIMAX_SPEED must be between 0.5 and 2.0, got {self.SPEED}"

        if self.VOL < 0 or self.VOL > 2.0:
            return False, f"MINIMAX_VOL must be between 0 and 2.0, got {self.VOL}"

        if self.PITCH < -12 or self.PITCH > 12:
            return False, f"MINIMAX_PITCH must be between -12 and 12, got {self.PITCH}"

        if self.AUDIO_CHANNEL not in [1, 2]:
            return False, f"AUDIO_CHANNEL must be 1 or 2, got {self.AUDIO_CHANNEL}"

        if self.BATCH_DURATION_MS <= 0:
            return False, f"BATCH_DURATION_MS must be positive, got {self.BATCH_DURATION_MS}"

        return True, ""
