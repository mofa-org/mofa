//! Config-driven speech adapter construction.
//!
//! Defines [`SpeechConfig`] / [`SpeechProviderConfig`] for TOML/YAML/JSON
//! configuration of cloud TTS and ASR adapters.  The actual registration
//! into a [`SpeechAdapterRegistry`] is performed by
//! `mofa_sdk::speech::register_speech_adapters`, which has access to both
//! this crate and `mofa-foundation`.

use serde::{Deserialize, Serialize};

// ============================================================================
// Config types
// ============================================================================

/// Configuration for a single cloud speech provider.
///
/// # Fields
///
/// - `provider` — vendor name: `"openai"`, `"elevenlabs"`, or `"deepgram"`.
/// - `api_key` — vendor API key.
/// - `default_tts` — make this provider the default TTS adapter.
/// - `default_asr` — make this provider the default ASR adapter.
///
/// # Example (TOML)
///
/// ```toml
/// [[speech.providers]]
/// provider    = "openai"
/// api_key     = "sk-..."
/// default_tts = true
/// default_asr = true
///
/// [[speech.providers]]
/// provider    = "elevenlabs"
/// api_key     = "..."
/// default_tts = false
/// default_asr = false
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeechProviderConfig {
    /// Vendor name: `"openai"`, `"elevenlabs"`, or `"deepgram"`.
    pub provider: String,
    /// API key for the vendor.
    pub api_key: String,
    /// If `true`, designate this adapter as the default TTS adapter.
    #[serde(default)]
    pub default_tts: bool,
    /// If `true`, designate this adapter as the default ASR adapter.
    #[serde(default)]
    pub default_asr: bool,
}

/// Top-level speech configuration consumed by `register_speech_adapters`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SpeechConfig {
    /// List of provider entries to register.
    #[serde(default)]
    pub providers: Vec<SpeechProviderConfig>,
}

impl SpeechConfig {
    /// Create an empty configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a provider entry.
    pub fn with_provider(mut self, entry: SpeechProviderConfig) -> Self {
        self.providers.push(entry);
        self
    }
}

impl SpeechProviderConfig {
    /// Convenience constructor.
    pub fn new(provider: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self {
            provider: provider.into(),
            api_key: api_key.into(),
            default_tts: false,
            default_asr: false,
        }
    }

    /// Mark this entry as the default TTS adapter.
    pub fn as_default_tts(mut self) -> Self {
        self.default_tts = true;
        self
    }

    /// Mark this entry as the default ASR adapter.
    pub fn as_default_asr(mut self) -> Self {
        self.default_asr = true;
        self
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_empty() {
        let cfg = SpeechConfig::new();
        assert!(cfg.providers.is_empty());
    }

    #[test]
    fn builder_adds_providers() {
        let cfg = SpeechConfig::new()
            .with_provider(SpeechProviderConfig::new("openai", "sk-test").as_default_tts())
            .with_provider(SpeechProviderConfig::new("deepgram", "dg-test").as_default_asr());

        assert_eq!(cfg.providers.len(), 2);
        assert_eq!(cfg.providers[0].provider, "openai");
        assert!(cfg.providers[0].default_tts);
        assert!(!cfg.providers[0].default_asr);
        assert_eq!(cfg.providers[1].provider, "deepgram");
        assert!(cfg.providers[1].default_asr);
    }

    #[test]
    fn deserialize_from_json() {
        let json = r#"{
            "providers": [
                { "provider": "openai",    "api_key": "sk-x", "default_tts": true, "default_asr": true },
                { "provider": "elevenlabs","api_key": "el-x" },
                { "provider": "deepgram",  "api_key": "dg-x", "default_asr": true }
            ]
        }"#;

        let cfg: SpeechConfig = serde_json::from_str(json).unwrap();
        assert_eq!(cfg.providers.len(), 3);

        assert!(cfg.providers[0].default_tts);
        assert!(cfg.providers[0].default_asr);

        // default_tts/default_asr should default to false when absent
        assert!(!cfg.providers[1].default_tts);
        assert!(!cfg.providers[1].default_asr);

        assert!(cfg.providers[2].default_asr);
    }

    #[test]
    fn serialize_roundtrip() {
        let original = SpeechConfig::new()
            .with_provider(SpeechProviderConfig::new("openai", "key").as_default_tts());

        let json = serde_json::to_string(&original).unwrap();
        let deserialized: SpeechConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.providers.len(), 1);
        assert_eq!(deserialized.providers[0].provider, "openai");
        assert_eq!(deserialized.providers[0].api_key, "key");
        assert!(deserialized.providers[0].default_tts);
    }
}
