use mofa_kernel::AgentPlugin;
use mofa_plugins::asr::openai::OpenAIASR;
use mofa_plugins::asr::{ASRPlugin, ASRPluginConfig};
use mofa_plugins::tts::openai::OpenAITTS;
use mofa_plugins::tts::{TTSPlugin, TTSPluginConfig};
use tracing::{info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_target(false)
        .init();

    info!("Starting Cloud Voice Integration Demo");

    // Load .env if present (good for OPENAI_API_KEY)
    let _ = dotenvy::dotenv();

    if std::env::var("OPENAI_API_KEY").is_err() {
        warn!("OPENAI_API_KEY is not set. The demo will likely fail.");
        warn!("Please export OPENAI_API_KEY before running: `export OPENAI_API_KEY=sk-...`");
    }

    // 1. Test Text-to-Speech (TTS)
    info!("--- Testing OpenAI TTS ---");
    let tts_config = TTSPluginConfig {
        model_version: "tts-1".to_string(),
        default_voice: "alloy".to_string(),
        ..Default::default()
    };
    
    let openai_tts_engine = OpenAITTS::new(tts_config)?;
    let mut tts_plugin = TTSPlugin::with_engine("openai_tts", openai_tts_engine, Some("alloy"));
    
    let sample_text = "Hello from the Microkernel Framework! I am speaking using OpenAI's cloud API.";
    info!("Synthesizing: '{}'", sample_text);
    let audio_bytes = tts_plugin.synthesize_to_audio(sample_text).await?;
    info!("Success! Received {} bytes of WAV/MP3 audio from OpenAI TTS.", audio_bytes.len());

    // 2. Test Automatic Speech Recognition (ASR)
    info!("--- Testing OpenAI ASR (Whisper) ---");
    let asr_config = ASRPluginConfig {
        default_language: "en".to_string(),
        default_model: "whisper-1".to_string(),
    };
    
    let openai_asr_engine = OpenAIASR::new(asr_config)?;
    let mut asr_plugin = ASRPlugin::with_engine("openai_asr", openai_asr_engine);
    
    info!("Uploading the just-synthesized {} bytes back to Whisper...", audio_bytes.len());
    let transcription = asr_plugin.transcribe(&audio_bytes).await?;
    info!("Transcribed Text: '{}'", transcription);

    info!("Demo completed successfully.");
    Ok(())
}
