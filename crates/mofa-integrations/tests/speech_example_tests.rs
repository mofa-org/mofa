//! Example / integration tests for cloud speech adapters.
//!
//! These tests demonstrate how to use TTS and ASR adapters in an end-to-end
//! workflow.  Mock-only tests run in CI; live-API tests are `#[ignore]`d and
//! require vendor API keys via environment variables.
//!
//! Run all mock tests:
//! ```sh
//! cargo test -p mofa-integrations --features openai-speech,elevenlabs,deepgram \
//!            --test speech_example_tests
//! ```
//!
//! Run live-API tests (requires keys):
//! ```sh
//! OPENAI_API_KEY=sk-… cargo test -p mofa-integrations --features openai-speech \
//!            --test speech_example_tests -- --ignored
//! ```

// ============================================================================
// OpenAI adapter examples
// ============================================================================

#[cfg(feature = "openai-speech")]
mod openai_examples {
    use mofa_integrations::speech::openai::{
        OpenAiAsrAdapter, OpenAiSpeechConfig, OpenAiTtsAdapter, OpenAiTtsModel,
    };
    use mofa_kernel::speech::{AsrAdapter, AsrConfig, AudioFormat, TtsAdapter, TtsConfig};

    // ---- Mock-safe tests (always run) ----

    #[tokio::test]
    async fn openai_tts_list_voices_returns_six() {
        let adapter = OpenAiTtsAdapter::new(OpenAiSpeechConfig::new().with_api_key("fake"));
        let voices = adapter.list_voices().await.unwrap();

        assert_eq!(voices.len(), 6, "OpenAI TTS should expose 6 voices");

        let ids: Vec<&str> = voices.iter().map(|v| v.id.as_str()).collect();
        for expected in ["alloy", "echo", "fable", "onyx", "nova", "shimmer"] {
            assert!(ids.contains(&expected), "missing voice: {}", expected);
        }
    }

    #[tokio::test]
    async fn openai_whisper_supported_languages_covers_major_ones() {
        let adapter = OpenAiAsrAdapter::new(OpenAiSpeechConfig::new().with_api_key("fake"));
        let langs = adapter.supported_languages();

        for expected in ["en", "zh", "es", "fr", "de", "ja", "ko"] {
            assert!(
                langs.contains(&expected.to_string()),
                "Whisper should support {}",
                expected
            );
        }
    }

    #[tokio::test]
    async fn openai_tts_health_check_with_key_set() {
        let adapter = OpenAiTtsAdapter::new(OpenAiSpeechConfig::new().with_api_key("sk-fake"));
        assert!(
            adapter.health_check().await.unwrap(),
            "health check should pass when key is set"
        );
    }

    #[test]
    fn openai_tts_hd_model_selection() {
        let adapter = OpenAiTtsAdapter::new(OpenAiSpeechConfig::new().with_api_key("fake"))
            .with_model(OpenAiTtsModel::Tts1Hd);
        assert_eq!(adapter.name(), "openai-tts");
    }

    // ---- Live-API tests (run with --ignored) ----

    #[tokio::test]
    #[ignore = "Requires OPENAI_API_KEY env var and makes real API calls"]
    async fn openai_tts_synthesize_real_api() {
        let adapter = OpenAiTtsAdapter::new(OpenAiSpeechConfig::new());

        let config = TtsConfig::new().with_format(AudioFormat::Mp3);
        let output = adapter
            .synthesize("Hello from MoFA voice pipeline!", "alloy", &config)
            .await
            .expect("TTS synthesis should succeed");

        assert!(!output.data.is_empty(), "audio data should not be empty");
        assert_eq!(output.format, AudioFormat::Mp3);
        println!(
            "✅ OpenAI TTS produced {} bytes of audio",
            output.data.len()
        );
    }

    #[tokio::test]
    #[ignore = "Requires OPENAI_API_KEY env var and makes real API calls"]
    async fn openai_whisper_transcribe_real_api() {
        // Generate a tiny WAV header with silence for testing
        let wav_header = generate_silent_wav(1, 16000, 16000); // 1 sec of silence
        let adapter = OpenAiAsrAdapter::new(OpenAiSpeechConfig::new());

        let config = AsrConfig::new().with_language("en");
        let result = adapter.transcribe(&wav_header, &config).await;
        // Whisper may return empty text for silence, which is OK
        println!("✅ OpenAI Whisper result: {:?}", result);
    }

    /// Generate a minimal WAV file with silence (for testing).
    fn generate_silent_wav(channels: u16, sample_rate: u32, num_samples: u32) -> Vec<u8> {
        let bits_per_sample: u16 = 16;
        let byte_rate = sample_rate * channels as u32 * bits_per_sample as u32 / 8;
        let block_align = channels * bits_per_sample / 8;
        let data_size = num_samples * channels as u32 * bits_per_sample as u32 / 8;
        let file_size = 36 + data_size;

        let mut buf = Vec::with_capacity(file_size as usize + 8);
        // RIFF header
        buf.extend_from_slice(b"RIFF");
        buf.extend_from_slice(&file_size.to_le_bytes());
        buf.extend_from_slice(b"WAVE");
        // fmt chunk
        buf.extend_from_slice(b"fmt ");
        buf.extend_from_slice(&16u32.to_le_bytes()); // chunk size
        buf.extend_from_slice(&1u16.to_le_bytes()); // PCM format
        buf.extend_from_slice(&channels.to_le_bytes());
        buf.extend_from_slice(&sample_rate.to_le_bytes());
        buf.extend_from_slice(&byte_rate.to_le_bytes());
        buf.extend_from_slice(&block_align.to_le_bytes());
        buf.extend_from_slice(&bits_per_sample.to_le_bytes());
        // data chunk (silence)
        buf.extend_from_slice(b"data");
        buf.extend_from_slice(&data_size.to_le_bytes());
        buf.resize(buf.len() + data_size as usize, 0); // zeros = silence
        buf
    }
}

// ============================================================================
// ElevenLabs adapter examples
// ============================================================================

#[cfg(feature = "elevenlabs")]
mod elevenlabs_examples {
    use mofa_integrations::speech::elevenlabs::{ElevenLabsConfig, ElevenLabsTtsAdapter};
    use mofa_kernel::speech::TtsAdapter;

    #[tokio::test]
    async fn elevenlabs_health_check_with_key() {
        let adapter = ElevenLabsTtsAdapter::new(ElevenLabsConfig::new().with_api_key("fake"));
        assert!(adapter.health_check().await.unwrap());
    }

    #[test]
    fn elevenlabs_adapter_name() {
        let adapter = ElevenLabsTtsAdapter::new(ElevenLabsConfig::new().with_api_key("fake"));
        assert_eq!(adapter.name(), "elevenlabs");
    }

    // ---- Live-API test ----

    #[tokio::test]
    #[ignore = "Requires ELEVENLABS_API_KEY env var and makes real API calls"]
    async fn elevenlabs_list_voices_real_api() {
        let adapter = ElevenLabsTtsAdapter::new(ElevenLabsConfig::new());
        let voices = adapter.list_voices().await.expect("should list voices");
        assert!(
            !voices.is_empty(),
            "ElevenLabs should have at least one voice"
        );
        println!("✅ Found {} ElevenLabs voices", voices.len());
    }
}

// ============================================================================
// Deepgram adapter examples
// ============================================================================

#[cfg(feature = "deepgram")]
mod deepgram_examples {
    use mofa_integrations::speech::deepgram::{DeepgramAsrAdapter, DeepgramConfig};
    use mofa_kernel::speech::AsrAdapter;

    #[tokio::test]
    async fn deepgram_health_check_with_key() {
        let adapter = DeepgramAsrAdapter::new(DeepgramConfig::new().with_api_key("fake"));
        assert!(adapter.health_check().await.unwrap());
    }

    #[test]
    fn deepgram_adapter_name() {
        let adapter = DeepgramAsrAdapter::new(DeepgramConfig::new().with_api_key("fake"));
        assert_eq!(adapter.name(), "deepgram");
    }
}
