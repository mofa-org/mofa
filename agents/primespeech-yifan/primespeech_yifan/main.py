#!/usr/bin/env python3
"""
PrimeSpeech TTS wrapper for the Yifan (Doubao) voice.
Reuses the Zhao Daniu implementation with different environment defaults.
"""

import os

DEFAULT_ENV = {
    "VOICE_NAME": "Doubao",
    "TEXT_LANG": "zh",
    "SPEED_FACTOR": "0.5",
}

for key, value in DEFAULT_ENV.items():
    os.environ.setdefault(key, value)

from primespeech_daniu.main import main as _base_main  # noqa: E402


def main() -> None:
    """Entry point for the Yifan PrimeSpeech agent."""
    for key, value in DEFAULT_ENV.items():
        os.environ.setdefault(key, value)
    _base_main()


if __name__ == "__main__":
    main()
