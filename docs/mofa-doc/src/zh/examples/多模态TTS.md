# 多模态 TTS

多模态能力示例，包括文本转语音。

## LLM + TTS 流式

流式 LLM 响应并自动 TTS 播放。

**位置：** `examples/llm_tts_streaming/`

```rust
use mofa_sdk::llm::{LLMAgentBuilder, openai_from_env};
use mofa_sdk::plugins::{KokoroTTS, TTSPlugin};
use rodio::{OutputStream, Sink, buffer::SamplesBuffer};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化 TTS 引擎
    let kokoro_engine = KokoroTTS::new(&model_path, &voice_path).await?;

    // 创建带 TTS 插件的智能体
    let agent = Arc::new(
        LLMAgentBuilder::new()
            .with_id(Uuid::new_v4().to_string())
            .with_name("Chat TTS Agent")
            .with_provider(Arc::new(openai_from_env()?))
            .with_system_prompt("你是一个友好的 AI 助手。")
            .with_temperature(0.7)
            .with_plugin(TTSPlugin::with_engine("tts", kokoro_engine, Some("zf_088")))
            .build()
    );

    // 音频输出设置
    let (_output_stream, stream_handle) = OutputStream::try_default()?;
    let audio_sink = Arc::new(Sink::try_new(&stream_handle)?);

    loop {
        let input = read_user_input()?;

        // 中断当前 TTS 播放
        agent.interrupt_tts().await?;
        audio_sink.stop();

        // 带 TTS 回调的流式 LLM 响应
        let sink_clone = audio_sink.clone();
        agent.chat_with_tts_callback(
            &session_id,
            &input,
            move |audio_f32| {
                sink_clone.append(SamplesBuffer::new(1, 24000, audio_f32));
            }
        ).await?;

        // 开始播放（非阻塞）
        audio_sink.play();
    }
}
```

### 特性

- **句子分割**：自动句子检测实现自然 TTS
- **非阻塞播放**：LLM 继续流式传输时音频播放
- **中断支持**：用户发送新消息时停止当前 TTS
- **语音选择**：多种语音选项可用

## Kokoro TTS 演示

直接使用 TTS 引擎，无需 LLM。

**位置：** `examples/kokoro_tts_demo/`

```rust
use mofa_sdk::plugins::{KokoroTTS, TTSPlugin};

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化 Kokoro TTS
    let engine = KokoroTTS::new(
        "path/to/kokoro-v1.1-zh.onnx",
        "path/to/voices-v1.1-zh.bin",
    ).await?;

    // 列出可用语音
    let voices = engine.list_voices();
    println!("可用语音: {:?}", voices);

    // 生成语音
    let text = "你好，这是 Kokoro TTS 引擎的测试。";
    let audio = engine.synthesize(text, "zf_088").await?;

    // 保存或播放音频
    std::fs::write("output.wav", &audio)?;
    println!("音频已保存到 output.wav");

    Ok(())
}
```

### Kokoro 配置

```rust
// 环境变量
export KOKORO_MODEL_PATH="/path/to/kokoro-v1.1-zh.onnx"
export KOKORO_VOICE_PATH="/path/to/voices-v1.1-zh.bin"

// 或编程配置
let engine = KokoroTTS::builder()
    .model_path("kokoro-v1.1-zh.onnx")
    .voice_path("voices-v1.1-zh.bin")
    .default_voice("zf_088")
    .sample_rate(24000)
    .build()
    .await?;
```

## TTS 插件集成

将 TTS 用作智能体插件：

```rust
// 创建 TTS 插件
let tts_plugin = TTSPlugin::with_engine("tts", engine, Some("zf_088"));

// 添加到智能体构建器
let agent = LLMAgentBuilder::new()
    .with_provider(provider)
    .with_plugin(tts_plugin)
    .build();

// 带 TTS 流式传输
agent.chat_with_tts_callback(&session_id, input, |audio| {
    // 处理音频块
    player.play(audio);
}).await?;
```

## 运行示例

```bash
# 设置必需的环境变量
export OPENAI_API_KEY=sk-xxx
export KOKORO_MODEL_PATH=/path/to/kokoro-v1.1-zh.onnx
export KOKORO_VOICE_PATH=/path/to/voices-v1.1-zh.bin

# 运行 LLM + TTS 流式
cargo run -p llm_tts_streaming

# 运行 Kokoro TTS 演示
cargo run -p kokoro_tts_demo
```

## 可用示例

| 示例 | 描述 |
|------|------|
| `llm_tts_streaming` | LLM 流式与 TTS 播放 |
| `kokoro_tts_demo` | 独立 Kokoro TTS 演示 |

## 相关链接

- [插件](插件.md) — 插件系统概述
- [API 参考：插件](../api-reference/plugins/README.md) — 插件 API
