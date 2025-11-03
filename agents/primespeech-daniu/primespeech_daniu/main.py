#!/usr/bin/env python3
"""
PrimeSpeech TTS agent for MoFA flows.
Wraps the Dora PrimeSpeech node while integrating with the MoFA agent runtime.
"""

import json
import os
import sys
import time
import traceback
from pathlib import Path
from typing import Optional

import numpy as np
import pyarrow as pa

from mofa.agent_build.base.base_agent import MofaAgent, run_agent

# Ensure the default voice is available before PrimeSpeech config is imported.
DEFAULT_VOICE_NAME = "Zhao Daniu"
os.environ.setdefault("VOICE_NAME", DEFAULT_VOICE_NAME)

from dora_primespeech.config import PrimeSpeechConfig, VOICE_CONFIGS  # noqa: E402
from dora_primespeech.model_manager import ModelManager  # noqa: E402
from dora_primespeech.moyoyo_tts_wrapper_streaming_fix import (  # noqa: E402
    StreamingMoYoYoTTSWrapper as MoYoYoTTSWrapper,
    MOYOYO_AVAILABLE,
)


def send_log(agent: MofaAgent, level: str, message: str, config_level: str = "INFO") -> None:
    """Send structured log lines via the Dora log output while mirroring to stdout/stderr."""
    log_levels = {"DEBUG": 10, "INFO": 20, "WARNING": 30, "ERROR": 40}
    if log_levels.get(level, 0) < log_levels.get(config_level, 20):
        return

    formatted_message = f"[{level}] {message}"
    try:
        stream = sys.stderr if level in {"ERROR", "WARNING"} else sys.stdout
        print(formatted_message, file=stream, flush=True)
    except Exception:
        pass

    log_data = {
        "node": agent.agent_name,
        "level": level,
        "message": formatted_message,
        "timestamp": time.time(),
    }
    agent.node.send_output("log", pa.array([json.dumps(log_data)]))


def validate_language_config(
    agent: MofaAgent,
    lang_code: str,
    param_name: str,
    log_level: str,
) -> str:
    """Validate language inputs and surface actionable guidance."""
    valid_languages = [
        "auto",
        "auto_yue",
        "en",
        "zh",
        "ja",
        "yue",
        "ko",
        "all_zh",
        "all_ja",
        "all_yue",
        "all_ko",
    ]

    if lang_code in valid_languages:
        return lang_code

    separator = "=" * 70
    print(f"\n{separator}", flush=True)
    print("❌ PRIMESPEECH CONFIGURATION ERROR", flush=True)
    print(separator, flush=True)

    main_error = f"INVALID {param_name}: '{lang_code}' is NOT a valid language!"
    print(main_error, flush=True)
    send_log(agent, "ERROR", main_error, log_level)

    normalized = lang_code.lower()
    if normalized == "cn":
        hint = "Did you mean 'zh' for Chinese? Use 'zh' not 'cn'."
    elif normalized == "chinese":
        hint = "Use 'zh' for Chinese, not 'chinese'."
    elif normalized == "english":
        hint = "Use 'en' for English, not 'english'."
    else:
        hint = ""

    if hint:
        print(f"💡 HINT: {hint}", flush=True)
        send_log(agent, "ERROR", hint, log_level)

    valid_msg = f"Valid languages: {', '.join(valid_languages)}"
    print(f"✅ {valid_msg}", flush=True)
    send_log(agent, "ERROR", valid_msg, log_level)

    warning = f"TTS will fail until you fix {param_name}!"
    print(f"⚠️  {warning}", flush=True)
    print(f"{separator}\n", flush=True)
    send_log(agent, "ERROR", warning, log_level)

    return lang_code


def validate_models_path(agent: MofaAgent, config: PrimeSpeechConfig) -> Optional[Path]:
    """Ensure the configured PrimeSpeech model directory exists."""
    raw_path = os.environ.get("PRIMESPEECH_MODEL_DIR")
    if not raw_path:
        send_log(agent, "ERROR", "Missing PRIMESPEECH_MODEL_DIR; TTS cannot load models", config.LOG_LEVEL)
        return None

    base_path = Path(os.path.expanduser(os.path.expandvars(raw_path)))
    if not base_path.exists():
        send_log(agent, "ERROR", f"PRIMESPEECH_MODEL_DIR points to missing path: {base_path}", config.LOG_LEVEL)
        return None

    moyoyo_dir = base_path / "moyoyo"
    if not moyoyo_dir.exists():
        send_log(agent, "WARNING", f"Expected models under: {moyoyo_dir} (directory missing)", config.LOG_LEVEL)

    return base_path


def _prepare_voice_configuration(agent: MofaAgent, config: PrimeSpeechConfig) -> tuple[str, dict]:
    """Resolve the effective voice configuration including overrides."""
    voice_name = config.VOICE_NAME or DEFAULT_VOICE_NAME
    if voice_name not in VOICE_CONFIGS:
        available = ", ".join(sorted(VOICE_CONFIGS.keys()))
        send_log(
            agent,
            "ERROR",
            f"Unknown voice '{voice_name}'. Available voices: {available}. Falling back to {DEFAULT_VOICE_NAME}.",
            config.LOG_LEVEL,
        )
        voice_name = DEFAULT_VOICE_NAME

    voice_config = dict(VOICE_CONFIGS[voice_name])

    if config.PROMPT_TEXT:
        voice_config["prompt_text"] = config.PROMPT_TEXT

    send_log(agent, "DEBUG", f"TEXT_LANG from env: '{config.TEXT_LANG}'", config.LOG_LEVEL)
    if config.TEXT_LANG:
        validated = validate_language_config(agent, config.TEXT_LANG, "TEXT_LANG", config.LOG_LEVEL)
        voice_config["text_lang"] = validated
        send_log(agent, "DEBUG", f"Validated TEXT_LANG: '{validated}'", config.LOG_LEVEL)

    send_log(agent, "DEBUG", f"PROMPT_LANG from env: '{config.PROMPT_LANG}'", config.LOG_LEVEL)
    if config.PROMPT_LANG:
        validated = validate_language_config(agent, config.PROMPT_LANG, "PROMPT_LANG", config.LOG_LEVEL)
        voice_config["prompt_lang"] = validated
        send_log(agent, "DEBUG", f"Validated PROMPT_LANG: '{validated}'", config.LOG_LEVEL)

    effective_speed_factor = (
        config.SPEED_FACTOR if config.SPEED_FACTOR is not None else voice_config.get("speed_factor", 1.0)
    )

    if config.SPEED_FACTOR is not None:
        send_log(agent, "INFO", f"Overriding speed_factor via env to {effective_speed_factor}", config.LOG_LEVEL)

    voice_config.update(
        {
            "top_k": config.TOP_K,
            "top_p": config.TOP_P,
            "temperature": config.TEMPERATURE,
            "speed_factor": effective_speed_factor,
            "batch_size": config.BATCH_SIZE,
            "seed": config.SEED,
            "text_split_method": config.TEXT_SPLIT_METHOD,
            "split_bucket": config.SPLIT_BUCKET,
            "return_fragment": config.RETURN_FRAGMENT,
            "fragment_interval": config.FRAGMENT_INTERVAL,
            "use_gpu": config.USE_GPU,
            "device": config.DEVICE,
            "sample_rate": config.SAMPLE_RATE,
        }
    )

    return voice_name, voice_config


@run_agent
def run(agent: MofaAgent) -> None:
    config = PrimeSpeechConfig()
    voice_name, voice_config = _prepare_voice_configuration(agent, config)

    model_manager = ModelManager(config.get_models_dir())
    if not model_manager.check_models_exist(voice_name, voice_config, verbose=False):
        send_log(
            agent,
            "INFO",
            f"PrimeSpeech models for {voice_name} missing locally, will download as needed.",
            config.LOG_LEVEL,
        )

    send_log(agent, "INFO", "PrimeSpeech agent initialized", config.LOG_LEVEL)
    send_log(
        agent,
        "INFO",
        f"Voice: {voice_name} | Text Language: {voice_config.get('text_lang', 'auto')} "
        f"(configured: {config.TEXT_LANG}) | Prompt Language: {voice_config.get('prompt_lang', 'auto')} "
        f"(configured: {config.PROMPT_LANG})",
        config.LOG_LEVEL,
    )
    send_log(agent, "INFO", f"MoYoYo available: {MOYOYO_AVAILABLE}", config.LOG_LEVEL)
    send_log(
        agent,
        "INFO",
        f"Speed Factor: {voice_config.get('speed_factor')} (env override: {config.SPEED_FACTOR is not None})",
        config.LOG_LEVEL,
    )
    send_log(agent, "INFO", f"Device: {config.DEVICE}", config.LOG_LEVEL)

    final_text_lang = voice_config.get("text_lang", "auto")
    final_prompt_lang = voice_config.get("prompt_lang", "auto")
    valid_languages = {
        "auto",
        "auto_yue",
        "en",
        "zh",
        "ja",
        "yue",
        "ko",
        "all_zh",
        "all_ja",
        "all_yue",
        "all_ko",
    }

    if final_text_lang not in valid_languages:
        send_log(
            agent,
            "ERROR",
            f"CRITICAL: text_lang '{final_text_lang}' is invalid. TTS will fail until fixed.",
            config.LOG_LEVEL,
        )
    if final_prompt_lang not in valid_languages:
        send_log(
            agent,
            "ERROR",
            f"CRITICAL: prompt_lang '{final_prompt_lang}' is invalid. TTS will fail until fixed.",
            config.LOG_LEVEL,
        )

    tts_engine: Optional[MoYoYoTTSWrapper] = None
    model_loaded = False
    total_syntheses = 0
    total_duration = 0.0

    for event in agent.node:
        if event["type"] == "INPUT":
            input_id = event["id"]
            if input_id == "text":
                agent.event = event
                text = event["value"][0].as_py()
                metadata = event.get("metadata", {}) or {}
                segment_index = metadata.get("segment_index", -1)

                send_log(
                    agent,
                    "DEBUG",
                    f"Received text segment {segment_index + 1}: '{text}'",
                    config.LOG_LEVEL,
                )

                text_stripped = text.strip()
                if not text_stripped or all(
                    c in '。！？.!?,，、；：""\'（）【】《》\n\r\t ' for c in text_stripped
                ):
                    send_log(
                        agent,
                        "DEBUG",
                        f"Skipping empty/punctuation-only segment {segment_index + 1}",
                        config.LOG_LEVEL,
                    )
                    agent.node.send_output(
                        "segment_complete",
                        pa.array(["skipped"]),
                        metadata={"segment_index": segment_index},
                    )
                    continue

                send_log(
                    agent,
                    "INFO",
                    f"Processing segment {segment_index + 1} (len={len(text)})",
                    config.LOG_LEVEL,
                )

                if not model_loaded:
                    send_log(agent, "INFO", "Loading PrimeSpeech models…", config.LOG_LEVEL)
                    validate_models_path(agent, config)
                    try:
                        moyoyo_voice = voice_name.lower().replace(" ", "")
                        device = "cuda" if config.USE_GPU and config.DEVICE.startswith("cuda") else "cpu"
                        enable_streaming = voice_config.get("return_fragment", config.RETURN_FRAGMENT)

                        tts_engine = MoYoYoTTSWrapper(
                            voice=moyoyo_voice,
                            device=device,
                            enable_streaming=enable_streaming,
                            chunk_duration=0.3,
                            voice_config=voice_config,
                            logger_func=lambda lvl, msg: send_log(agent, lvl, msg, config.LOG_LEVEL),
                        )

                        if tts_engine is None or not hasattr(tts_engine, "tts") or tts_engine.tts is None:
                            send_log(agent, "ERROR", "TTS engine initialization failed", config.LOG_LEVEL)
                        else:
                            send_log(agent, "INFO", "TTS engine initialized successfully", config.LOG_LEVEL)
                        model_loaded = True
                    except Exception as init_err:
                        send_log(agent, "ERROR", f"TTS init error: {init_err}", config.LOG_LEVEL)
                        send_log(agent, "ERROR", f"Traceback: {traceback.format_exc()}", config.LOG_LEVEL)
                        model_loaded = False
                        agent.node.send_output(
                            "segment_complete",
                            pa.array(["error"]),
                            metadata={
                                "segment_index": segment_index,
                                "error": str(init_err),
                                "error_stage": "init",
                            },
                        )
                        continue

                if tts_engine is None:
                    send_log(agent, "ERROR", "TTS engine not available after initialization", config.LOG_LEVEL)
                    agent.node.send_output(
                        "segment_complete",
                        pa.array(["error"]),
                        metadata={
                            "segment_index": segment_index,
                            "error": "tts_engine_unavailable",
                            "error_stage": "pre_synthesis",
                        },
                    )
                    continue

                speed = voice_config.get("speed_factor", 1.0)
                language = voice_config.get("text_lang", "auto")

                start_time = time.time()

                try:
                    if voice_config.get("return_fragment", config.RETURN_FRAGMENT):
                        fragment_num = 0
                        total_audio_duration = 0.0

                        for sample_rate, audio_fragment in tts_engine.synthesize_streaming(
                            text, language=language, speed=speed
                        ):
                            fragment_num += 1
                            if audio_fragment is None or len(audio_fragment) == 0:
                                send_log(
                                    agent,
                                    "WARNING",
                                    f"Skipping empty audio fragment {fragment_num}",
                                    config.LOG_LEVEL,
                                )
                                continue

                            if audio_fragment.dtype != np.float32:
                                audio_fragment = audio_fragment.astype(np.float32)

                            fragment_duration = len(audio_fragment) / sample_rate
                            total_audio_duration += fragment_duration
                            agent.node.send_output(
                                "audio",
                                pa.array([audio_fragment]),
                                metadata={
                                    "segment_index": segment_index,
                                    "segments_remaining": metadata.get("segments_remaining", 0),
                                    "question_id": metadata.get("question_id", "default"),
                                    "fragment_num": fragment_num,
                                    "sample_rate": sample_rate,
                                    "duration": fragment_duration,
                                    "is_streaming": True,
                                },
                            )

                        synthesis_time = time.time() - start_time
                        send_log(
                            agent,
                            "INFO",
                            f"Streamed {fragment_num} fragments ({total_audio_duration:.2f}s audio) "
                            f"in {synthesis_time:.3f}s",
                            config.LOG_LEVEL,
                        )

                        if fragment_num == 0:
                            raise RuntimeError("No audio fragments produced during streaming synthesis")

                    else:
                        sample_rate, audio_array = tts_engine.synthesize(text, language=language, speed=speed)
                        if audio_array is None or len(audio_array) == 0:
                            raise RuntimeError("TTS returned empty audio array")

                        if audio_array.dtype != np.float32:
                            audio_array = audio_array.astype(np.float32)

                        audio_duration = len(audio_array) / sample_rate
                        synthesis_time = time.time() - start_time
                        total_syntheses += 1
                        total_duration += audio_duration

                        send_log(
                            agent,
                            "INFO",
                            f"Synthesized {audio_duration:.2f}s audio in {synthesis_time:.3f}s",
                            config.LOG_LEVEL,
                        )

                        agent.node.send_output(
                            "audio",
                            pa.array([audio_array]),
                            metadata={
                                "segment_index": segment_index,
                                "segments_remaining": metadata.get("segments_remaining", 0),
                                "question_id": metadata.get("question_id", "default"),
                                "sample_rate": sample_rate,
                                "duration": audio_duration,
                                "is_streaming": False,
                            },
                        )

                    agent.node.send_output(
                        "segment_complete",
                        pa.array(["completed"]),
                        metadata={"segment_index": segment_index},
                    )
                    send_log(agent, "INFO", f"Sent segment_complete for segment {segment_index + 1}", config.LOG_LEVEL)

                except Exception as synth_err:
                    error_details = traceback.format_exc()

                    if "assert text_lang" in str(synth_err) or "assert prompt_lang" in str(synth_err):
                        send_log(agent, "ERROR", "=" * 60, config.LOG_LEVEL)
                        send_log(agent, "ERROR", "CRITICAL: Language configuration error detected!", config.LOG_LEVEL)
                        send_log(agent, "ERROR", f"TEXT_LANG: '{language}'", config.LOG_LEVEL)
                        send_log(
                            agent,
                            "ERROR",
                            f"PROMPT_LANG: '{voice_config.get('prompt_lang', 'auto')}'",
                            config.LOG_LEVEL,
                        )
                        send_log(
                            agent,
                            "ERROR",
                            "Valid languages: auto, auto_yue, zh, en, ja, ko, yue, all_zh, all_ja, all_yue, all_ko",
                            config.LOG_LEVEL,
                        )
                        send_log(agent, "ERROR", "Fix your configuration and restart!", config.LOG_LEVEL)
                        send_log(agent, "ERROR", "=" * 60, config.LOG_LEVEL)

                    send_log(agent, "ERROR", f"Synthesis error: {synth_err}", config.LOG_LEVEL)
                    send_log(agent, "ERROR", f"Traceback: {error_details}", config.LOG_LEVEL)

                    agent.node.send_output(
                        "segment_complete",
                        pa.array(["error"]),
                        metadata={
                            "segment_index": segment_index,
                            "error": str(synth_err),
                            "error_stage": "synthesis",
                        },
                    )
                    send_log(
                        agent,
                        "ERROR",
                        f"Sent error segment_complete for segment {segment_index + 1}",
                        config.LOG_LEVEL,
                    )

            elif input_id == "control":
                command = event["value"][0].as_py()
                if command == "reset":
                    send_log(agent, "INFO", "[PrimeSpeech] RESET received", config.LOG_LEVEL)
                elif command == "stats":
                    send_log(agent, "INFO", f"Total syntheses: {total_syntheses}", config.LOG_LEVEL)
                    send_log(agent, "INFO", f"Total audio duration: {total_duration:.1f}s", config.LOG_LEVEL)

        elif event["type"] == "STOP":
            break

    send_log(agent, "INFO", "PrimeSpeech agent stopped", config.LOG_LEVEL)


def main() -> None:
    agent = MofaAgent(agent_name="primespeech-daniu", is_write_log=True)
    run(agent=agent)


if __name__ == "__main__":
    main()
