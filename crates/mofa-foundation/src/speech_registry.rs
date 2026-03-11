//! Speech adapter registry — runtime discovery and resolution for TTS/ASR adapters
//!
//! The [`SpeechAdapterRegistry`] provides a central lookup table so the
//! orchestrator and voice pipeline can resolve the best available TTS/ASR
//! adapter at runtime without hard-coding vendor names.

use mofa_kernel::speech::{AsrAdapter, TtsAdapter};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::info;

// ============================================================================
// Registry
// ============================================================================

/// Central registry for TTS and ASR adapters.
///
/// Adapters are registered by name (e.g. `"openai-tts"`, `"elevenlabs"`)
/// and resolved at runtime.  A default adapter can be designated for each
/// modality so callers don't need to know the vendor name.
pub struct SpeechAdapterRegistry {
    tts_adapters: HashMap<String, Arc<dyn TtsAdapter>>,
    asr_adapters: HashMap<String, Arc<dyn AsrAdapter>>,
    default_tts: Option<String>,
    default_asr: Option<String>,
}

impl SpeechAdapterRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            tts_adapters: HashMap::new(),
            asr_adapters: HashMap::new(),
            default_tts: None,
            default_asr: None,
        }
    }

    // ---- TTS ---------------------------------------------------------------

    /// Register a TTS adapter.  If this is the first adapter, it becomes the
    /// default automatically.
    pub fn register_tts(&mut self, adapter: Arc<dyn TtsAdapter>) {
        let name = adapter.name().to_string();
        info!("[speech-registry] registered TTS adapter: {}", name);
        if self.tts_adapters.is_empty() {
            self.default_tts = Some(name.clone());
        }
        self.tts_adapters.insert(name, adapter);
    }

    /// Set the default TTS adapter by name. Returns true if the adapter exists in the registry.
    pub fn set_default_tts(&mut self, name: impl Into<String>) -> bool {
        let name = name.into();
        if self.tts_adapters.contains_key(&name) {
            self.default_tts = Some(name);
            true
        } else {
            false
        }
    }

    /// Get a TTS adapter by name.
    pub fn get_tts(&self, name: &str) -> Option<Arc<dyn TtsAdapter>> {
        self.tts_adapters.get(name).cloned()
    }

    /// Get the default TTS adapter.
    pub fn default_tts(&self) -> Option<Arc<dyn TtsAdapter>> {
        self.default_tts
            .as_ref()
            .and_then(|n| self.tts_adapters.get(n))
            .cloned()
    }

    /// List registered TTS adapter names.
    pub fn list_tts(&self) -> Vec<String> {
        self.tts_adapters.keys().cloned().collect()
    }

    // ---- ASR ---------------------------------------------------------------

    /// Register an ASR adapter.  If this is the first adapter, it becomes the
    /// default automatically.
    pub fn register_asr(&mut self, adapter: Arc<dyn AsrAdapter>) {
        let name = adapter.name().to_string();
        info!("[speech-registry] registered ASR adapter: {}", name);
        if self.asr_adapters.is_empty() {
            self.default_asr = Some(name.clone());
        }
        self.asr_adapters.insert(name, adapter);
    }

    /// Set the default ASR adapter by name. Returns true if the adapter exists in the registry.
    pub fn set_default_asr(&mut self, name: impl Into<String>) -> bool {
        let name = name.into();
        if self.asr_adapters.contains_key(&name) {
            self.default_asr = Some(name);
            true
        } else {
            false
        }
    }

    /// Get an ASR adapter by name.
    pub fn get_asr(&self, name: &str) -> Option<Arc<dyn AsrAdapter>> {
        self.asr_adapters.get(name).cloned()
    }

    /// Get the default ASR adapter.
    pub fn default_asr(&self) -> Option<Arc<dyn AsrAdapter>> {
        self.default_asr
            .as_ref()
            .and_then(|n| self.asr_adapters.get(n))
            .cloned()
    }

    /// List registered ASR adapter names.
    pub fn list_asr(&self) -> Vec<String> {
        self.asr_adapters.keys().cloned().collect()
    }

    // ---- Introspection -----------------------------------------------------

    /// Total number of registered adapters (TTS + ASR).
    pub fn adapter_count(&self) -> usize {
        self.tts_adapters.len() + self.asr_adapters.len()
    }
}

impl Default for SpeechAdapterRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use mofa_kernel::agent::AgentResult;
    use mofa_kernel::speech::*;

    // ---- Mock adapters for testing ----

    struct MockTts;

    #[async_trait]
    impl TtsAdapter for MockTts {
        fn name(&self) -> &str {
            "mock-tts"
        }

        async fn synthesize(
            &self,
            _text: &str,
            _voice: &str,
            _config: &TtsConfig,
        ) -> AgentResult<AudioOutput> {
            Ok(AudioOutput::new(vec![0u8; 10], AudioFormat::Wav, 24000))
        }

        async fn list_voices(&self) -> AgentResult<Vec<VoiceDescriptor>> {
            Ok(vec![VoiceDescriptor::new("default", "Default", "en-US")])
        }
    }

    struct MockAsr;

    #[async_trait]
    impl AsrAdapter for MockAsr {
        fn name(&self) -> &str {
            "mock-asr"
        }

        async fn transcribe(
            &self,
            _audio: &[u8],
            _config: &AsrConfig,
        ) -> AgentResult<TranscriptionResult> {
            Ok(TranscriptionResult::text_only("transcribed text"))
        }
    }

    // ---- Tests ----

    #[test]
    fn empty_registry() {
        let reg = SpeechAdapterRegistry::new();
        assert_eq!(reg.adapter_count(), 0);
        assert!(reg.default_tts().is_none());
        assert!(reg.default_asr().is_none());
        assert!(reg.list_tts().is_empty());
        assert!(reg.list_asr().is_empty());
    }

    #[test]
    fn register_and_get_tts() {
        let mut reg = SpeechAdapterRegistry::new();
        reg.register_tts(Arc::new(MockTts));

        assert_eq!(reg.adapter_count(), 1);
        assert!(reg.get_tts("mock-tts").is_some());
        assert!(reg.get_tts("nonexistent").is_none());

        // First registered becomes default
        let default = reg.default_tts().unwrap();
        assert_eq!(default.name(), "mock-tts");
    }

    #[test]
    fn register_and_get_asr() {
        let mut reg = SpeechAdapterRegistry::new();
        reg.register_asr(Arc::new(MockAsr));

        assert_eq!(reg.adapter_count(), 1);
        assert!(reg.get_asr("mock-asr").is_some());

        let default = reg.default_asr().unwrap();
        assert_eq!(default.name(), "mock-asr");
    }

    #[test]
    fn set_default_tts() {
        let mut reg = SpeechAdapterRegistry::new();
        reg.register_tts(Arc::new(MockTts));
        reg.set_default_tts("mock-tts");
        assert_eq!(reg.default_tts().unwrap().name(), "mock-tts");
    }

    #[test]
    fn list_adapters() {
        let mut reg = SpeechAdapterRegistry::new();
        reg.register_tts(Arc::new(MockTts));
        reg.register_asr(Arc::new(MockAsr));

        assert_eq!(reg.adapter_count(), 2);
        assert!(reg.list_tts().contains(&"mock-tts".to_string()));
        assert!(reg.list_asr().contains(&"mock-asr".to_string()));
    }
}
