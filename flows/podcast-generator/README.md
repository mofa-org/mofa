# Podcast Generator

Generate two-person podcast audio from markdown scripts using Dora's PrimeSpeech TTS.

## Overview

This example demonstrates:
- Sequential TTS generation with two different voices (大牛 with Luo Xiang voice, 一帆 with Doubao voice)
- Intelligent text segmentation for long passages (maintains sentence completeness)
- Automatic random 1-3 second silence padding between speaker changes
- Markdown-based script format for easy editing
- Dynamic node orchestration with Dora
- Real-time monitoring with viewer node

## Architecture

```
┌─────────────────────┐
│ script_segmenter.py │ (dynamic node)
│  - Parse markdown   │
│  - Split long text  │
│  - Send segments    │
└──────────┬──────────┘
           │
    ┌──────┴──────┐
    │             │
    ▼             ▼
┌─────────┐  ┌─────────┐
│ daniu   │  │ yifan   │ (PrimeSpeech TTS)
│ TTS     │  │ TTS     │ (static nodes)
└────┬────┘  └────┬────┘
     │            │
     │   audio +  │
     │  segment_  │
     │  complete  │
     └─────┬──────┘
           ▼
  ┌─────────────────┐
  │ voice_output.py │ (dynamic node)
  │  - Concatenate  │
  │  - Random 1-3s  │
  │    silence      │
  │  - Write WAV    │
  └─────────────────┘
```

## TTS Options

This example supports two TTS engines:

### Option 1: PrimeSpeech (Local, GPU-based)
- **Dataflow:** `dataflow.yml`
- **Pros:** Free, offline, high quality
- **Cons:** Requires GPU, model downloads (~5GB)
- **Voices:** Luo Xiang (大牛), Doubao (一帆)

### Option 2: MiniMax T2A (Cloud API)
- **Dataflow:** `dataflow-minimax.yml`
- **Pros:** No GPU needed, fast startup, no model downloads
- **Cons:** API costs, requires internet
- **Voices:** Liu Xiang (大牛), Doubao (一帆)
- **Setup:** Requires `MINIMAX_API_KEY` environment variable

## Node Inventory

### PrimeSpeech Nodes (dataflow.yml)

| Node ID | Type | Role | Inputs | Outputs |
|---------|------|------|--------|---------|
| `script-segmenter` | Dynamic | Parse markdown, apply intelligent text segmentation, and orchestrate TTS generation | `daniu_segment_complete`, `yifan_segment_complete` | `daniu_text`, `yifan_text`, `script_complete`, `log` |
| `primespeech-daniu` | Static | TTS for 大牛 (Luo Xiang voice) | `text` | `audio`, `segment_complete`, `log` |
| `primespeech-yifan` | Static | TTS for 一帆 (Doubao voice) | `text` | `audio`, `segment_complete`, `log` |
| `voice-output` | Dynamic | Concatenate audio with silence padding, write WAV | `daniu_audio`, `yifan_audio`, `daniu_segment_complete`, `yifan_segment_complete`, `script_complete` | `log` |
| `viewer` | Dynamic | Monitor logs and events (optional) | All logs and text events | none |

### MiniMax T2A Nodes (dataflow-minimax.yml)

| Node ID | Type | Role | Inputs | Outputs |
|---------|------|------|--------|---------|
| `script-segmenter` | Dynamic | Parse markdown, apply intelligent text segmentation, and orchestrate TTS generation | `daniu_segment_complete`, `yifan_segment_complete` | `daniu_text`, `yifan_text`, `script_complete`, `log` |
| `minimax-daniu` | Static | TTS for 大牛 (Liu Xiang voice via MiniMax API) | `text` | `audio`, `segment_complete`, `log` |
| `minimax-yifan` | Static | TTS for 一帆 (Doubao voice via MiniMax API) | `text` | `audio`, `segment_complete`, `log` |
| `voice-output` | Dynamic | Concatenate audio with silence padding, write WAV (with input queues for reliability) | `daniu_audio`, `yifan_audio`, `daniu_segment_complete`, `yifan_segment_complete`, `script_complete` | `log` |
| `viewer` | Dynamic | Monitor logs and events (optional) | All logs and text events | none |

## Prerequisites
Refer to the `mac-aec-chat` example for environment setup and base dependencies before running this project.

## Quick Start (MiniMax 默认推荐)

1. 准备 MiniMax API Key：
   ```bash
   export MINIMAX_API_KEY="your-api-key-here"
   ```
   获取地址：https://platform.minimax.io/user-center/basic-information/interface-key
2. 在三个终端中依次执行以下命令（保持同一 Conda/虚拟环境）：
   - **终端 1** – 启动数据流（静态节点）  
     ```bash
     cd mofa/flows/podcast-generator
     dora start dataflow-minimax.yml
     ```
   - **终端 2** – 启动语音合成输出（动态节点）  
     ```bash
     cd mofa/flows/podcast-generator
     voice-output --output-file out/podcast_minimax.wav
     ```
   - **终端 3** – 启动脚本分段器（动态节点）  
     ```bash
     cd mofa/flows/podcast-generator
     script-segmenter --input-file scripts/agentcomp.md
     ```
   - **终端 4（可选）** – 启动可视化监控  
     ```bash
     cd mofa/flows/podcast-generator
     viewer
     ```
3. 想要快速复现上述流程，可使用项目自带脚本（默认使用 MiniMax dataflow）：  
   ```bash
   cd mofa/flows/podcast-generator
   ./run_podcast.sh scripts/example_podcast.md out/podcast_minimax.wav dataflow-minimax.yml
   ```

### Using PrimeSpeech (dataflow.yml)

PrimeSpeech 版本依赖本地 GPU 及模型，可按需运行：

- **终端 1**  
  ```bash
  cd mofa/flows/podcast-generator
  dora start dataflow.yml
  ```
- **终端 2**  
  ```bash
  cd mofa/flows/podcast-generator
  voice-output --output-file out/podcast_primespeech.wav
  ```
- **终端 3**  
  ```bash
  cd mofa/flows/podcast-generator
  script-segmenter --input-file scripts/agentcomp.md
  ```
- **终端 4（可选）**  
  ```bash
  cd mofa/flows/podcast-generator
  viewer
  ```

### Using MiniMax T2A Trio (dataflow-minimax-trio.yml)

三人对话流程在第二步改为 `dora start dataflow-minimax-trio.yml`，其余命令保持一致，脚本需包含【博宇】片段以触发第三位角色。

### ⏰ Launch Order Tips

- 建议先启动 `voice-output`，再启动 `script-segmenter`，避免漏收音频段。
- 所有动态节点需在线直到 `script-segmenter` 输出 `script_complete`。

## Script Format

Create markdown files with speaker tags:

```markdown
# Your Podcast Title

【大牛】Text spoken by Daniu using Luo Xiang voice.

【一帆】Text spoken by Yifan using Doubao voice.

【大牛】More text from Daniu...

【一帆】More text from Yifan...
```

### Rules
- Speaker tags must be `【大牛】` or `【一帆】`
- Text accumulates from a speaker tag until the next speaker tag appears
- All lines between speaker tags are combined into one segment
- Empty lines and lines without tags (headers, etc.) are ignored
- Long segments are automatically split at punctuation marks (see Text Segmentation)
- Segments are processed sequentially in order
- Supports both plain `【大牛】` and markdown bold `**【大牛】**` formats

## Configuration

### Text Segmentation for Long Passages

Long text segments are automatically split into smaller chunks to prevent overly long TTS generation. The segmentation preserves sentence completeness by splitting at punctuation marks.

**Environment Variables:**
```bash
# Maximum segment duration (default: 10 seconds)
export MAX_SEGMENT_DURATION=10.0

# TTS speaking speed estimation (default: 4.5 chars/second for Chinese)
export TTS_CHARS_PER_SECOND=4.5

# Punctuation marks for intelligent splitting (default includes Chinese and English)
export PUNCTUATION_MARKS="。！？.!?，,、；;：:"
```

**How it works:**
- Converts `MAX_SEGMENT_DURATION` to character count using `TTS_CHARS_PER_SECOND`
- Default: 10 seconds × 4.5 chars/sec = 45 characters max per segment
- Splits at punctuation boundaries to maintain sentence completeness
- Falls back to whitespace if no punctuation found
- Logs splitting activity for monitoring

**Example:**
```bash
# Allow longer segments (20 seconds max)
MAX_SEGMENT_DURATION=20.0 script-segmenter --input-file scripts/agentcomp.md

# Adjust for faster speech (6 chars/second)
TTS_CHARS_PER_SECOND=6.0 script-segmenter --input-file scripts/agentcomp.md

# Use only sentence-ending punctuation for splits
PUNCTUATION_MARKS="。！？.!?" script-segmenter --input-file scripts/agentcomp.md
```

### Change Voices

#### PrimeSpeech Voices (dataflow.yml)
Edit `dataflow.yml` to modify voice selection:

```yaml
env:
  VOICE_NAME: "Luo Xiang"  # Options: Doubao, Luo Xiang, Yang Mi, Zhou Jielun, Ma Yun, Maple, Cove
```

#### MiniMax T2A Voices (dataflow-minimax.yml)

To use different voices:
1. Visit the [MiniMax Audio Portal](https://www.minimax.io/audio/text-to-speech)
2. Browse and preview available voices
3. Copy the voice ID for your chosen voice
4. Update `dataflow-minimax.yml` with the voice ID:

```yaml
env:
  MINIMAX_VOICE_ID: "your-voice-id-here"  # Replace with voice ID from MiniMax portal
```

- **Current configuration in** `dataflow-minimax.yml`
  - **大牛 (Daniu):** `ttv-voice-2025103011222725-sg8dZxUP`
  - **一帆 (Yifan):** `moss_audio_aaa1346a-7ce7-11f0-8e61-2e6e3c7ee85d`

- **Current configuration in** `dataflow-minimax-trio.yml`
  - **大牛 (Daniu):** `ttv-voice-2025103011222725-sg8dZxUP`
  - **一帆 (Yifan):** `moss_audio_aaa1346a-7ce7-11f0-8e61-2e6e3c7ee85d`
  - **博宇 (Boyu):** `moss_audio_9c223de9-7ce1-11f0-9b9f-463feaa3106a`

**Additional voice parameters:**

- Each MiniMax node accepts the following knobs:

  ```yaml
  env:
    MINIMAX_SPEED: "<0.5-2.0>"           # Speech speed multiplier
    MINIMAX_VOL: "<0-2.0>"              # Output loudness
    MINIMAX_PITCH: "<-12-12>"           # Semitone shift
    ENABLE_ENGLISH_NORMALIZATION: "true"  # Toggles MiniMax english_normalization flag
    BATCH_DURATION_MS: "2000"           # Leave at 2000ms to avoid packet drops
  ```

- Current values in this repo:
  - `dataflow-minimax.yml`
    - Daniu: speed `1.0`, volume `1.0`, pitch `-1`
    - Yifan: speed `1.0`, volume `1.0`, pitch `0`
  - `dataflow-minimax-trio.yml`
    - Daniu: speed `1.0`, volume `1.0`, pitch `-1`
    - Yifan: speed `1.0`, volume `1.0`, pitch `0`
    - Boyu: speed `1.0`, volume `1.1`, pitch `1`

#### Preventing Audio Packet Loss (MiniMax Only)

The MiniMax dataflow uses two mechanisms to prevent packet loss:

**1. Audio Batching (`BATCH_DURATION_MS: "2000"`):**
- Accumulates audio chunks into 2-second batches before sending to Dora
- Reduces messages from ~200 to ~3-4 per synthesis
- Prevents shared memory exhaustion

**2. Input Audio Queues (`queue_size: 1000`):**
- Configured in `dataflow-minimax.yml` for voice-output node
- Buffers up to 1000 audio messages per speaker
- Prevents dropped packets when audio arrives in bursts

Without these settings, you may experience:
- Missing audio fragments (gaps in fragment numbers)
- 50%+ silence in the output WAV file
- Choppy, interrupted speech

These settings are already configured in `dataflow-minimax.yml` and don't need to be changed unless you experience issues.

### Change Silence Duration
The silence between speaker changes is randomized between 1-3 seconds by default. To customize, edit `voice_output.py`:

```python
silence_min = 1.0  # minimum silence in seconds
silence_max = 3.0  # maximum silence in seconds
```

For fixed silence duration, set both to the same value:
```python
silence_min = 2.0  # fixed 2 seconds
silence_max = 2.0  # fixed 2 seconds
```

### Change Sample Rate
PrimeSpeech outputs at 32kHz by default. To change, edit `mofa/agents/voice-output/voice_output/main.py`:

```python
sample_rate = 32000  # Default PrimeSpeech sample rate
```

### Custom Script
Create your own markdown script and pass it to the segmenter:

```bash
script-segmenter --input-file scripts/my_custom_script.md
```

### Custom Output File
Specify a different output file:

```bash
voice-output --output-file out/my_podcast.wav
```

## Output

Generated audio: `out/podcast_output.wav`
- **Format:** 16-bit PCM WAV
- **Sample rate:** 32kHz (PrimeSpeech default)
- **Channels:** Mono
- **Silence:** Random 1-3 seconds between speaker changes (大牛 ↔ 一帆) for natural pacing

## How It Works

### 1. Text Segmentation
- `script_segmenter` parses markdown and extracts character segments
- Long segments are automatically split into smaller chunks (default: 10 seconds / ~45 characters)
- Splitting respects sentence boundaries using punctuation marks
- Each chunk is processed sequentially through TTS

### 2. Sequential Processing
- `script_segmenter` sends one text segment at a time
- Waits for `segment_complete` signal before sending next segment
- This ensures audio arrives at `voice_output` in correct order

### 3. Silence Padding
- `voice_output` tracks the last speaker
- When speaker changes (大牛 → 一帆 or 一帆 → 大牛):
  - Receives `segment_complete` from previous speaker
  - Adds random 1-3 seconds of silence
  - Receives audio from new speaker
- No silence is added:
  - Before the first speaker
  - Between consecutive segments from the same speaker

### 4. Completion Signal
- After sending all segments, `script_segmenter` sends `script_complete`
- `voice_output` receives this signal, writes final WAV file, and exits
- All nodes then stop gracefully

## Viewer Output

The optional viewer displays:
- **Color-coded logs:**
  - 🔴 RED for ERROR
  - 🟡 YELLOW for WARNING
  - 🔵 CYAN for INFO
- **Node-specific icons:**
  - 📝 Script Segmenter
  - 🎤 大牛 TTS (Luo Xiang)
  - 🎙️ 一帆 TTS (Doubao)
  - 🔊 Voice Output
- **Real-time events:**
  - Text segments being sent to each TTS
  - Audio segments being received
  - Silence padding being added
  - Final completion status

### Example Viewer Output
```
======================================================================
🎙️ Podcast Generator Viewer
======================================================================
Monitoring pipeline events...

[15:30:45.123] 📝 SCRIPT-SEGMENTER: [INFO] Script Segmenter started
[15:30:45.234] 📝 SCRIPT-SEGMENTER: [INFO] Text segmentation config: max_duration=10.0s, chars_per_second=4.5, max_length=45 chars
[15:30:45.345] 📝 SCRIPT-SEGMENTER: [INFO] Loaded 7 segments from scripts/agentcomp.md
[15:30:45.456] 📝 SCRIPT-SEGMENTER: [INFO] After text segmentation: 7 total segments to process
[15:30:45.567] 🎤 大牛: 大家好，欢迎来到今天的技术分享。我是大牛。
[15:30:47.678] 🔊 VOICE-OUTPUT: [INFO] Received audio from 大牛 (48000 samples)
[15:30:47.789] 🎙️ 一帆: 大家好，我是一帆。今天我们聊聊人工智能的最新进展。
[15:30:48.890] 🔊 VOICE-OUTPUT: [INFO] Added 2.47s silence (大牛 → 一帆)
[15:30:50.123] 🔊 VOICE-OUTPUT: [INFO] Received audio from 一帆 (52000 samples)
[15:30:51.234] 🎤 大牛: 没错，最近AI领域确实有很多激动人心的突破。
[15:30:52.345] 🔊 VOICE-OUTPUT: [INFO] Added 1.83s silence (一帆 → 大牛)
...
[15:31:15.456] 📝 SCRIPT-SEGMENTER: [INFO] All segments processed. Sending script_complete.
[15:31:15.567] 🔊 VOICE-OUTPUT: [INFO] Podcast saved: out/podcast_output.wav (45.32s)

[15:31:15.678] ✅ PODCAST GENERATION COMPLETE!
```

## Troubleshooting

### Dynamic nodes must stay running
If a dynamic node exits early, the dataflow will stall. Keep all terminals open until you see "PODCAST GENERATION COMPLETE" in the viewer.

### Audio segments out of order
This shouldn't happen due to sequential processing. If it does, check that:
- Only one `script_segmenter` instance is running
- The segmenter is receiving `segment_complete` signals correctly

### No audio output
Check:
1. PrimeSpeech models are installed: `ls ~/.dora/models/primespeech`
2. Both TTS nodes started successfully in Terminal 1
3. No errors in viewer logs

### Stop a running dataflow
```bash
dora stop
```

This stops the static nodes. You'll need to manually stop the dynamic nodes (Ctrl+C in each terminal).

## Example Use Cases

1. **Educational podcasts:** Create teaching content with two voices
2. **Story narration:** Different voices for different characters
3. **Interview simulation:** Question-and-answer format
4. **Language learning:** Dialogue practice with native-sounding voices
5. **Audio book production:** Multiple narrator voices

## Advanced Usage

### Batch Processing
Process multiple scripts in sequence:

```bash
for script in scripts/*.md; do
    output="out/$(basename "$script" .md).wav"
    # Start dynamic nodes with custom args
    script-segmenter --input-file "$script" &
    voice-output --output-file "$output" &
    wait
done
```

### Custom Markdown Parser
Extend `parse_markdown()` in `mofa/agents/script-segmenter/script_segmenter/main.py` to support:
- More than two speakers
- Emotion tags: `【大牛:excited】`
- Pause controls: `【pause:2s】`
- Background music markers

## License

Part of the Dora AI framework. See main repository for license information.

## Credits

- **Dora Framework:** https://github.com/dora-rs/dora
- **PrimeSpeech TTS:** Chinese voice synthesis
- **Example adapted from:** mac-aec-chat example

Enjoy creating your podcasts with MoFA & Dora! 🎙️
