# Multimodal & TTS

Examples demonstrating multimodal capabilities including text-to-speech.

## TTS Plugin Integration

Text-to-speech is provided by `TTSPlugin`, which wraps any engine implementing
the `TTSEngine` trait. The plugin system keeps the agent unaware of the
underlying speech provider.

```rust
use mofa_sdk::plugins::{OpenAITTS, TTSPlugin, TTSPluginConfig};

// Build an OpenAI TTS engine (requires OPENAI_API_KEY)
let config = TTSPluginConfig::default();
let engine = OpenAITTS::new(config)?;

// Create the TTS plugin with a default voice
let tts_plugin = TTSPlugin::with_engine("tts", engine, Some("alloy"));

// Add to agent builder
let agent = LLMAgentBuilder::new()
    .with_provider(provider)
    .with_plugin(tts_plugin)
    .build();

// Speak a text response
agent.tts_speak("Hello, world").await?;

// Stream with TTS
agent.chat_with_tts_callback(&session_id, input, |audio| {
    // Handle audio chunk
    player.play(audio);
}).await?;
```

### Key APIs

| API | Description |
|-----|-------------|
| `TTSEngine` | Engine trait: `synthesize`, `synthesize_stream`, `list_voices` |
| `OpenAITTS` | OpenAI text-to-speech engine |
| `MockTTSEngine` | Silent engine for tests |
| `TTSPlugin::with_engine` | Attach an engine to a plugin |
| `play_audio` / `play_audio_async` | WAV playback (requires the `rodio` feature) |

### Agent TTS Methods

- `tts_speak(text)` — synthesize and play a full response
- `tts_speak_streaming(text, callback)` — stream audio chunks as they are synthesized
- `tts_speak_f32_stream(text, callback)` — stream native f32 samples
- `chat_with_tts(session_id, message)` — chat and speak the reply
- `chat_with_tts_callback(session_id, message, callback)` — chat and stream audio chunks
- `interrupt_tts()` — stop current playback and cancel ongoing synthesis

### Custom Engine

Implement `TTSEngine` to plug in any speech provider:

```rust
use mofa_plugins::tts::{TTSEngine, TTSPluginConfig, VoiceInfo};

struct MyEngine;

#[async_trait::async_trait]
impl TTSEngine for MyEngine {
    fn name(&self) -> &str { "my-engine" }
    // ... implement synthesis methods
}
```

## See Also

- [Plugins](plugins.md) — Plugin system overview
- [API Reference: Plugins](../api-reference/plugins/README.md) — Plugin API
