"""
Simplified Dora PrimeSpeech Node - Main entry point
High-quality text-to-speech using GPT-SoVITS technology.
"""

import time
import os
import sys
import traceback
import json
import numpy as np
import pyarrow as pa
try:
    from mofa.agent_build.base.base_agent import MofaAgent, run_agent
    MOFA_AVAILABLE = True
except ImportError:
    from dora import Node
    MOFA_AVAILABLE = False
from pathlib import Path
from typing import Optional

from .config import PrimeSpeechConfig, VOICE_CONFIGS
from .model_manager import ModelManager
from .moyoyo_tts_wrapper_streaming_fix import StreamingMoYoYoTTSWrapper as MoYoYoTTSWrapper, MOYOYO_AVAILABLE


def send_log(node, level, message, config_level="INFO"):
    """Send log message through log output channel."""
    LOG_LEVELS = {
        "DEBUG": 10,
        "INFO": 20,
        "WARNING": 30,
        "ERROR": 40
    }
    
    if LOG_LEVELS.get(level, 0) < LOG_LEVELS.get(config_level, 20):
        return
    
    formatted_message = f"[{level}] {message}"
    # Also print to console so errors show in docker logs
    try:
        print(formatted_message, file=sys.stderr if level in {"ERROR", "WARNING"} else sys.stdout, flush=True)
    except Exception:
        pass
    log_data = {
        "node": "primespeech",
        "level": level,
        "message": formatted_message,
        "timestamp": time.time()
    }
    node.send_output("log", pa.array([json.dumps(log_data)]))


def validate_language_config(lang_code, param_name, node, log_level):
    """Validate language configuration and provide helpful error messages"""
    # Valid language codes for MoYoYo TTS v2
    VALID_LANGUAGES = ["auto", "auto_yue", "en", "zh", "ja", "yue", "ko",
                      "all_zh", "all_ja", "all_yue", "all_ko"]

    if lang_code in VALID_LANGUAGES:
        return lang_code

    # Invalid language code - show error prominently
    error_header = "=" * 70
    print(f"\n{error_header}", flush=True)
    print(f"❌ PRIMESPEECH CONFIGURATION ERROR", flush=True)
    print(f"{error_header}", flush=True)

    main_error = f"INVALID {param_name}: '{lang_code}' is NOT a valid language!"
    print(f"{main_error}", flush=True)
    send_log(node, "ERROR", main_error, log_level)

    # Check for common mistakes and suggest corrections
    if lang_code.lower() == "cn":
        hint = "Did you mean 'zh' for Chinese? Use 'zh' not 'cn'!"
        print(f"💡 HINT: {hint}", flush=True)
        send_log(node, "ERROR", hint, log_level)
    elif lang_code.lower() == "chinese":
        hint = "Use 'zh' for Chinese, not 'chinese'!"
        print(f"💡 HINT: {hint}", flush=True)
        send_log(node, "ERROR", hint, log_level)
    elif lang_code.lower() == "english":
        hint = "Use 'en' for English, not 'english'!"
        print(f"💡 HINT: {hint}", flush=True)
        send_log(node, "ERROR", hint, log_level)

    valid_msg = f"Valid languages: {', '.join(VALID_LANGUAGES)}"
    print(f"✅ {valid_msg}", flush=True)
    send_log(node, "ERROR", valid_msg, log_level)

    print(f"⚠️  TTS will FAIL until you fix {param_name} in your configuration!", flush=True)
    print(f"{error_header}\n", flush=True)
    send_log(node, "ERROR", f"TTS will fail until you fix {param_name}!", log_level)

    # Return the invalid code as-is (will cause TTS to fail with clear error)
    return lang_code


def _validate_models_path(logger, models_env_var="PRIMESPEECH_MODEL_DIR") -> Optional[Path]:
    """Validate that required model directory exists and contains MoYoYo subdir.
    Returns the resolved path if valid, else None.
    """
    raw = os.environ.get(models_env_var)
    if not raw:
        logger("ERROR", f"Missing {models_env_var} environment variable; TTS cannot load models")
        return None
    # Expand env vars (e.g., $HOME) and user (~)
    base = Path(os.path.expanduser(os.path.expandvars(raw)))
    if not base.exists():
        logger("ERROR", f"{models_env_var} points to non-existent path: {base}")
        return None
    moyoyo_dir = base / "moyoyo"
    if not moyoyo_dir.exists():
        logger("WARNING", f"Expected models under: {moyoyo_dir} (directory missing)")
    return base


def main():
    """Main entry point for PrimeSpeech node"""

    if MOFA_AVAILABLE:
        agent = MofaAgent(agent_name="primespeech", is_write_log=True)
        node = agent.node
    else:
        node = Node()
    config = PrimeSpeechConfig()

    # Get voice configuration
    voice_name = config.VOICE_NAME
    if voice_name not in VOICE_CONFIGS:
        send_log(node, "ERROR", f"Unknown voice: {voice_name}. Available: {list(VOICE_CONFIGS.keys())}", config.LOG_LEVEL)
        voice_name = "Doubao"

    voice_config = VOICE_CONFIGS[voice_name]

    # Override with environment variables if provided
    if config.PROMPT_TEXT:
        voice_config["prompt_text"] = config.PROMPT_TEXT

    # Validate and set text language
    send_log(node, "DEBUG", f"TEXT_LANG from env: '{config.TEXT_LANG}'", config.LOG_LEVEL)
    if config.TEXT_LANG:
        validated_text_lang = validate_language_config(
            config.TEXT_LANG, "TEXT_LANG", node, config.LOG_LEVEL)
        voice_config["text_lang"] = validated_text_lang
        send_log(node, "DEBUG", f"Validated TEXT_LANG: '{validated_text_lang}'", config.LOG_LEVEL)

    # Validate and set prompt language
    send_log(node, "DEBUG", f"PROMPT_LANG from env: '{config.PROMPT_LANG}'", config.LOG_LEVEL)
    if config.PROMPT_LANG:
        validated_prompt_lang = validate_language_config(
            config.PROMPT_LANG, "PROMPT_LANG", node, config.LOG_LEVEL)
        voice_config["prompt_lang"] = validated_prompt_lang
        send_log(node, "DEBUG", f"Validated PROMPT_LANG: '{validated_prompt_lang}'", config.LOG_LEVEL)
    
    # Add inference parameters
    effective_speed_factor = (
        config.SPEED_FACTOR
        if config.SPEED_FACTOR is not None
        else voice_config.get("speed_factor", 1.0)
    )

    if config.SPEED_FACTOR is not None:
        send_log(
            node,
            "INFO",
            f"Overriding speed_factor via env to {effective_speed_factor}",
            config.LOG_LEVEL,
        )

    voice_config.update({
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
    })
    
    # Initialize model manager
    model_manager = ModelManager(config.get_models_dir())
    
    send_log(node, "INFO", "PrimeSpeech Node initialized", config.LOG_LEVEL)
    
    if MOYOYO_AVAILABLE:
        send_log(node, "INFO", "✓ MoYoYo TTS engine available", config.LOG_LEVEL)
    else:
        send_log(node, "WARNING", "⚠️  MoYoYo TTS not fully available", config.LOG_LEVEL)
    
    # Log the configuration being used
    send_log(node, "INFO", f"Voice: {voice_name}", config.LOG_LEVEL)
    send_log(node, "INFO", f"Text Language: {voice_config.get('text_lang', 'auto')} (configured: {config.TEXT_LANG})", config.LOG_LEVEL)
    send_log(node, "INFO", f"Prompt Language: {voice_config.get('prompt_lang', 'auto')} (configured: {config.PROMPT_LANG})", config.LOG_LEVEL)

    # Print to stdout for immediate visibility
    speed_factor_value = voice_config.get('speed_factor')
    print(f"[PRIMESPEECH SPEED_FACTOR] Voice: {voice_name}, Speed: {speed_factor_value}, Env Override: {config.SPEED_FACTOR_OVERRIDE}", flush=True)

    send_log(node, "INFO", f"Speed Factor: {speed_factor_value} (env override: {config.SPEED_FACTOR_OVERRIDE is not None})", config.LOG_LEVEL)
    send_log(node, "INFO", f"Device: {config.DEVICE}", config.LOG_LEVEL)

    # Validate the final configuration
    final_text_lang = voice_config.get('text_lang', 'auto')
    final_prompt_lang = voice_config.get('prompt_lang', 'auto')

    VALID_LANGUAGES = ["auto", "auto_yue", "en", "zh", "ja", "yue", "ko",
                      "all_zh", "all_ja", "all_yue", "all_ko"]

    if final_text_lang not in VALID_LANGUAGES:
        send_log(node, "ERROR",
                f"CRITICAL: text_lang '{final_text_lang}' is not valid! "
                f"This will cause TTS to fail. Please fix your configuration.",
                config.LOG_LEVEL)

    if final_prompt_lang not in VALID_LANGUAGES:
        send_log(node, "ERROR",
                f"CRITICAL: prompt_lang '{final_prompt_lang}' is not valid! "
                f"This will cause TTS to fail. Please fix your configuration.",
                config.LOG_LEVEL)
    
    # Initialize TTS engine
    tts_engine: Optional[MoYoYoTTSWrapper] = None
    model_loaded = False
    
    # Statistics
    total_syntheses = 0
    total_duration = 0
    
    for event in node:
        if event["type"] == "INPUT":
            input_id = event["id"]
            
            if input_id == "text":
                # Get text to synthesize
                text = event["value"][0].as_py()
                metadata = event.get("metadata", {})

                # DEBUG: Log what we received
                send_log(node, "DEBUG", f"RECEIVED text: '{text}' (len={len(text)}, repr={repr(text)}, type={type(text).__name__})", config.LOG_LEVEL)

                segment_index = metadata.get("segment_index", -1)

                # Skip if text is only punctuation or whitespace
                text_stripped = text.strip()
                if not text_stripped or all(c in '。！？.!?,，、；：""''（）【】《》\n\r\t ' for c in text_stripped):
                    send_log(node, "DEBUG", f"SKIPPED - text is only punctuation/whitespace: '{text}'", config.LOG_LEVEL)
                    # Send segment_complete without audio
                    node.send_output(
                        "segment_complete",
                        pa.array(["skipped"]),
                        metadata={}
                    )
                    continue

                send_log(node, "INFO", f"Processing segment {segment_index + 1} (len={len(text)})", config.LOG_LEVEL)
                
                # Load models if not loaded
                if not model_loaded:
                    send_log(node, "INFO", "Loading models for the first time...", config.LOG_LEVEL)
                    # Validate models directory early so failures are visible
                    _validate_models_path(lambda lvl, msg: send_log(node, lvl, msg, config.LOG_LEVEL))

                    try:
                        # Always use PRIMESPEECH_MODEL_DIR
                        send_log(node, "INFO", "Using PRIMESPEECH_MODEL_DIR for models...", config.LOG_LEVEL)
                        # Initialize TTS engine
                        # Convert voice name to lowercase and remove spaces for MoYoYo compatibility
                        moyoyo_voice = voice_name.lower().replace(" ", "")
                        device = "cuda" if config.USE_GPU and config.DEVICE.startswith("cuda") else "cpu"

                        enable_streaming = config.RETURN_FRAGMENT if hasattr(config, 'RETURN_FRAGMENT') else False

                        # Initialize TTS wrapper using PRIMESPEECH_MODEL_DIR
                        tts_engine = MoYoYoTTSWrapper(
                            voice=moyoyo_voice,
                            device=device,
                            enable_streaming=enable_streaming,
                            chunk_duration=0.3,
                            voice_config=voice_config,
                            logger_func=lambda level, msg: send_log(node, level, msg, config.LOG_LEVEL)
                        )

                        # Check if initialization succeeded
                        if tts_engine is None or not hasattr(tts_engine, 'tts') or tts_engine.tts is None:
                            send_log(node, "ERROR", "TTS engine initialization failed!", config.LOG_LEVEL)
                            send_log(node, "ERROR", "TTS wrapper exists but internal TTS is None", config.LOG_LEVEL)
                        else:
                            send_log(node, "INFO", "TTS engine initialized successfully", config.LOG_LEVEL)
                        model_loaded = True
                        send_log(node, "INFO", "TTS engine ready", config.LOG_LEVEL)
                    except Exception as init_err:
                        send_log(node, "ERROR", f"TTS init error: {init_err}", config.LOG_LEVEL)
                        send_log(node, "ERROR", f"Traceback: {traceback.format_exc()}", config.LOG_LEVEL)
                        # Mark as not loaded and send error completion without audio
                        model_loaded = False
                        node.send_output(
                            "segment_complete",
                            pa.array(["error"]),
                            metadata={
                                "session_id": session_id,
                                "request_id": request_id,
                                "segment_index": segment_index,
                                "error": str(init_err),
                                "error_stage": "init"
                            }
                        )
                        # Skip this event since we cannot synthesize
                        continue
                
                # Synthesize speech
                start_time = time.time()
                
                try:
                    # Check if TTS engine is available
                    if tts_engine is None:
                        send_log(node, "ERROR", "Cannot synthesize - TTS engine is None!", config.LOG_LEVEL)
                        raise RuntimeError("TTS engine not initialized")
                    
                    if hasattr(tts_engine, 'tts') and tts_engine.tts is None:
                        send_log(node, "ERROR", "Cannot synthesize - internal TTS is None!", config.LOG_LEVEL)
                        raise RuntimeError("Internal TTS engine not initialized")
                    
                    language = voice_config.get("text_lang", "zh")
                    speed = voice_config.get("speed_factor", 1.0)
                    
                    if hasattr(tts_engine, 'enable_streaming') and tts_engine.enable_streaming:
                        # Streaming synthesis
                        send_log(node, "INFO", "Using streaming synthesis...", config.LOG_LEVEL)
                        fragment_num = 0
                        total_audio_duration = 0
                        
                        for sample_rate, audio_fragment in tts_engine.synthesize_streaming(text, language=language, speed=speed):
                            fragment_num += 1
                            fragment_duration = len(audio_fragment) / sample_rate
                            total_audio_duration += fragment_duration

                            # Guard against empty fragments
                            if audio_fragment is None or len(audio_fragment) == 0:
                                send_log(node, "WARNING", f"Skipping empty audio fragment {fragment_num}", config.LOG_LEVEL)
                            else:
                                # Ensure type is float32 for consistency
                                if audio_fragment.dtype != np.float32:
                                    audio_fragment = audio_fragment.astype(np.float32)
                                node.send_output(
                                    "audio",
                                    pa.array([audio_fragment]),
                                    metadata={
                                        "segment_index": segment_index,
                                        "segments_remaining": metadata.get("segments_remaining", 0),
                                        "question_id": metadata.get("question_id", "default"),  # Pass through question_id
                                        "fragment_num": fragment_num,
                                        "sample_rate": sample_rate,
                                        "duration": fragment_duration,
                                        "is_streaming": True,
                                    }
                                )
                        
                        synthesis_time = time.time() - start_time
                        send_log(node, "INFO", f"Streamed {fragment_num} fragments, {total_audio_duration:.2f}s audio in {synthesis_time:.3f}s", config.LOG_LEVEL)
                        # If nothing was streamed, mark as error to avoid hanging clients
                        if fragment_num == 0:
                            raise RuntimeError("No audio fragments produced during streaming synthesis")
                        
                    else:
                        # Batch synthesis
                        sample_rate, audio_array = tts_engine.synthesize(text, language=language, speed=speed)
                        
                        synthesis_time = time.time() - start_time
                        audio_duration = len(audio_array) / sample_rate
                        if audio_array is None or len(audio_array) == 0:
                            raise RuntimeError("TTS returned empty audio array")
                        # Normalize dtype
                        if audio_array.dtype != np.float32:
                            audio_array = audio_array.astype(np.float32)
                        
                        total_syntheses += 1
                        total_duration += audio_duration
                        
                        send_log(node, "INFO", f"Synthesized: {audio_duration:.2f}s audio in {synthesis_time:.3f}s", config.LOG_LEVEL)
                        
                        # Send audio output with segment counting metadata
                        node.send_output(
                            "audio",
                            pa.array([audio_array]),
                            metadata={
                                "segment_index": segment_index,
                                "segments_remaining": metadata.get("segments_remaining", 0),
                                "question_id": metadata.get("question_id", "default"),  # Pass through question_id
                                "sample_rate": sample_rate,
                                "duration": audio_duration,
                                "is_streaming": False,
                            }
                        )
                    
                    # Send segment completion signal
                    node.send_output(
                        "segment_complete",
                        pa.array(["completed"]),
                        metadata={}
                    )
                    send_log(node, "INFO", f"Sent segment_complete for segment {segment_index + 1}", config.LOG_LEVEL)
                    
                except Exception as e:
                    error_details = traceback.format_exc()

                    # Check for specific language-related errors
                    if "assert text_lang" in str(e) or "assert prompt_lang" in str(e) or "AssertionError" in str(e.__class__.__name__):
                        send_log(node, "ERROR", "="*60, config.LOG_LEVEL)
                        send_log(node, "ERROR", "CRITICAL: Language configuration error detected!", config.LOG_LEVEL)
                        send_log(node, "ERROR", f"TEXT_LANG: '{language}' (from config: '{config.TEXT_LANG}')", config.LOG_LEVEL)
                        send_log(node, "ERROR", f"PROMPT_LANG: '{voice_config.get('prompt_lang', 'auto')}' (from config: '{config.PROMPT_LANG}')", config.LOG_LEVEL)
                        send_log(node, "ERROR", "Valid languages: auto, auto_yue, zh, en, ja, ko, yue, all_zh, all_ja, all_yue, all_ko", config.LOG_LEVEL)
                        send_log(node, "ERROR", "Common mistakes: 'cn' should be 'zh', 'chinese' should be 'zh'", config.LOG_LEVEL)
                        send_log(node, "ERROR", "Fix your configuration and restart!", config.LOG_LEVEL)
                        send_log(node, "ERROR", "="*60, config.LOG_LEVEL)

                    send_log(node, "ERROR", f"Synthesis error: {e}", config.LOG_LEVEL)
                    send_log(node, "ERROR", f"Traceback: {error_details}", config.LOG_LEVEL)
                    
                    # Do NOT send invalid audio on error; only notify completion with error
                    node.send_output(
                        "segment_complete",
                        pa.array(["error"]),
                        metadata={
                            "error": str(e),
                            "error_stage": "synthesis"
                        }
                    )
                    send_log(node, "ERROR", f"Sent error segment_complete for segment {segment_index + 1}", config.LOG_LEVEL)
            
            elif input_id == "control":
                # Handle control commands
                command = event["value"][0].as_py()
                
                if command == "reset":
                    send_log(node, "INFO", "[PrimeSpeech] RESET received", config.LOG_LEVEL)
                    # Note: Can't actually stop ongoing synthesis, but it's OK
                    # because we only process one segment at a time now
                    send_log(node, "INFO", "[PrimeSpeech] Reset acknowledged", config.LOG_LEVEL)
                
                elif command == "stats":
                    send_log(node, "INFO", f"Total syntheses: {total_syntheses}", config.LOG_LEVEL)
                    send_log(node, "INFO", f"Total audio duration: {total_duration:.1f}s", config.LOG_LEVEL)
        
        elif event["type"] == "STOP":
            break
    
    send_log(node, "INFO", "PrimeSpeech node stopped", config.LOG_LEVEL)


if __name__ == "__main__":
    main()
