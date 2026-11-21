# 多模态与 TTS

展示多模态能力（包括文字转语音）的示例。

## TTS 插件集成

文字转语音由 `TTSPlugin` 提供，它封装了任何实现 `TTSEngine` trait 的引擎。
插件系统让智能体无需关心底层语音服务商的实现细节。

```rust
use mofa_sdk::plugins::{OpenAITTS, TTSPlugin, TTSPluginConfig};

// 构建 OpenAI TTS 引擎（需要 OPENAI_API_KEY）
let config = TTSPluginConfig::default();
let engine = OpenAITTS::new(config)?;

// 创建 TTS 插件并指定默认音色
let tts_plugin = TTSPlugin::with_engine("tts", engine, Some("alloy"));

// 添加到智能体构建器
let agent = LLMAgentBuilder::new()
    .with_provider(provider)
    .with_plugin(tts_plugin)
    .build();

// 朗读一段文本
agent.tts_speak("你好，世界").await?;

// 对话并流式输出音频
agent.chat_with_tts_callback(&session_id, input, |audio| {
    // 处理音频块
    player.play(audio);
}).await?;
```

### 核心 API

| API | 说明 |
|-----|------|
| `TTSEngine` | 引擎 trait：`synthesize`、`synthesize_stream`、`list_voices` |
| `OpenAITTS` | OpenAI 文字转语音引擎 |
| `MockTTSEngine` | 用于测试的静音引擎 |
| `TTSPlugin::with_engine` | 将引擎挂载到插件 |
| `play_audio` / `play_audio_async` | WAV 播放（需要 `rodio` feature） |

### 智能体 TTS 方法

- `tts_speak(text)` — 合成并播放完整响应
- `tts_speak_streaming(text, callback)` — 边合成边输出音频块
- `tts_speak_f32_stream(text, callback)` — 输出原生 f32 采样
- `chat_with_tts(session_id, message)` — 对话并朗读回复
- `chat_with_tts_callback(session_id, message, callback)` — 对话并流式回调音频块
- `interrupt_tts()` — 停止当前播放并取消进行中的合成

### 自定义引擎

实现 `TTSEngine` 即可接入任意语音服务商：

```rust
use mofa_plugins::tts::{TTSEngine, TTSPluginConfig, VoiceInfo};

struct MyEngine;

#[async_trait::async_trait]
impl TTSEngine for MyEngine {
    fn name(&self) -> &str { "my-engine" }
    // ... 实现合成方法
}
```

## 另请参阅

- [插件](插件.md) — 插件系统概览
