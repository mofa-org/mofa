# Multimodal & TTS

Examples demonstrating multimodal capabilities including text-to-speech.

## LLM + TTS Streaming

Stream LLM responses with automatic TTS playback.

**Location:** `examples/llm_tts_streaming/`

```rust
use mofa_sdk::llm::{LLMAgentBuilder, openai_from_env};
use mofa_sdk::plugins::{KokoroTTS, TTSPlugin};
use rodio::{OutputStream, Sink, buffer::SamplesBuffer};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize TTS engine
    let kokoro_engine = KokoroTTS::new(&model_path, &voice_path).await?;

    // Create agent with TTS plugin
    let agent = Arc::new(
        LLMAgentBuilder::new()
            .with_id(Uuid::new_v4().to_string())
            .with_name("Chat TTS Agent")
            .with_provider(Arc::new(openai_from_env()?))
            .with_system_prompt("You are a friendly AI assistant.")
            .with_temperature(0.7)
            .with_plugin(TTSPlugin::with_engine("tts", kokoro_engine, Some("zf_088")))
            .build()
    );

    // Audio output setup
    let (_output_stream, stream_handle) = OutputStream::try_default()?;
    let audio_sink = Arc::new(Sink::try_new(&stream_handle)?);

    loop {
        let input = read_user_input()?;

        // Interrupt current TTS playback
        agent.interrupt_tts().await?;
        audio_sink.stop();

        // Stream LLM response with TTS callback
        let sink_clone = audio_sink.clone();
        agent.chat_with_tts_callback(
            &session_id,
            &input,
            move |audio_f32| {
                sink_clone.append(SamplesBuffer::new(1, 24000, audio_f32));
            }
        ).await?;

        // Start playback (non-blocking)
        audio_sink.play();
    }
}
```

### Features

- **Sentence segmentation**: Automatic sentence detection for natural TTS
- **Non-blocking playback**: Audio plays while LLM continues streaming
- **Interruption support**: Stop current TTS when user sends new message
- **Voice selection**: Multiple voice options available

## Kokoro TTS Demo

Direct TTS engine usage without LLM.

**Location:** `examples/kokoro_tts_demo/`

```rust
use mofa_sdk::plugins::{KokoroTTS, TTSPlugin};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize Kokoro TTS
    let engine = KokoroTTS::new(
        "path/to/kokoro-v1.1-zh.onnx",
        "path/to/voices-v1.1-zh.bin",
    ).await?;

    // List available voices
    let voices = engine.list_voices();
    println!("Available voices: {:?}", voices);

    // Generate speech
    let text = "Hello, this is a test of the Kokoro TTS engine.";
    let audio = engine.synthesize(text, "zf_088").await?;

    // Save or play audio
    std::fs::write("output.wav", &audio)?;
    println!("Audio saved to output.wav");

    Ok(())
}
```

### Kokoro Configuration

```rust
// Environment variables
export KOKORO_MODEL_PATH="/path/to/kokoro-v1.1-zh.onnx"
export KOKORO_VOICE_PATH="/path/to/voices-v1.1-zh.bin"

// Or configure programmatically
let engine = KokoroTTS::builder()
    .model_path("kokoro-v1.1-zh.onnx")
    .voice_path("voices-v1.1-zh.bin")
    .default_voice("zf_088")
    .sample_rate(24000)
    .build()
    .await?;
```

## TTS Plugin Integration

Use TTS as an agent plugin:

```rust
// Create TTS plugin
let tts_plugin = TTSPlugin::with_engine("tts", engine, Some("zf_088"));

// Add to agent builder
let agent = LLMAgentBuilder::new()
    .with_provider(provider)
    .with_plugin(tts_plugin)
    .build();

// Stream with TTS
agent.chat_with_tts_callback(&session_id, input, |audio| {
    // Handle audio chunk
    player.play(audio);
}).await?;
```

## Running Examples

```bash
# Set required environment variables
export OPENAI_API_KEY=sk-xxx
export KOKORO_MODEL_PATH=/path/to/kokoro-v1.1-zh.onnx
export KOKORO_VOICE_PATH=/path/to/voices-v1.1-zh.bin

# Run LLM + TTS streaming
cargo run -p llm_tts_streaming

# Run Kokoro TTS demo
cargo run -p kokoro_tts_demo
```

## Available Examples

| Example | Description |
|---------|-------------|
| `llm_tts_streaming` | LLM streaming with TTS playback |
| `kokoro_tts_demo` | Standalone Kokoro TTS demo |

## See Also

- [Plugins](plugins.md) — Plugin system overview
- [API Reference: Plugins](../api-reference/plugins/README.md) — Plugin API
