//! Automatic Speech Recognition (ASR) Plugin Module
//!
//! Provides ASR capabilities using a generic ASR engine interface.
//! This module is designed to work with multiple ASR backends.
//!

pub mod openai;

use crate::{AgentPlugin, PluginContext, PluginMetadata, PluginResult, PluginState, PluginType};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info, warn};

// ============================================================================
// ASR Plugin Configuration
// ============================================================================

/// ASR plugin configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ASRPluginConfig {
    /// Default language code for transcription (e.g., "en")
    pub default_language: String,
    /// Default model to use
    pub default_model: String,
}

impl Default for ASRPluginConfig {
    fn default() -> Self {
        Self {
            default_language: "en".to_string(),
            default_model: "whisper-1".to_string(),
        }
    }
}

// ============================================================================
// ASR Engine Trait
// ============================================================================

/// Abstract ASR engine trait for extensibility
#[async_trait::async_trait]
pub trait ASREngine: Send + Sync {
    /// Transcribe audio data to text
    async fn transcribe(&self, audio: &[u8], language: Option<&str>) -> PluginResult<String>;

    /// Get engine name
    fn name(&self) -> &str;

    /// Get as Any for downcasting to engine-specific types
    fn as_any(&self) -> &dyn std::any::Any;
}

// ============================================================================
// Mock ASR Engine (Placeholder)
// ============================================================================

/// A mock ASR engine for testing and development.
pub struct MockASREngine {
    config: ASRPluginConfig,
}

impl MockASREngine {
    /// Create a new mock ASR engine
    pub fn new(config: ASRPluginConfig) -> Self {
        Self { config }
    }
}

#[async_trait::async_trait]
impl ASREngine for MockASREngine {
    async fn transcribe(&self, audio: &[u8], _language: Option<&str>) -> PluginResult<String> {
        debug!("[MockASR] Transcribing {} bytes of audio...", audio.len());

        // Simulate processing delay based on audio size
        let delay_ms = std::cmp::min(1000, audio.len() as u64 / 100);
        tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;

        Ok("Mock transcription result: The Quick Brown Fox Jumps Over The Lazy Dog.".to_string())
    }

    fn name(&self) -> &str {
        "MockASR"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

// ============================================================================
// ASR Plugin
// ============================================================================

/// ASR Plugin implementing AgentPlugin
pub struct ASRPlugin {
    metadata: PluginMetadata,
    state: PluginState,
    config: ASRPluginConfig,
    engine: Option<Arc<dyn ASREngine>>,
    transcription_count: u64,
}

impl ASRPlugin {
    /// Create a new ASR plugin
    pub fn new(plugin_id: &str) -> Self {
        let metadata = PluginMetadata::new(plugin_id, "ASR Plugin", PluginType::Tool)
            .with_description(
                "Automatic Speech Recognition plugin with support for multiple backends",
            )
            .with_capability("audio_transcription")
            .with_capability("speech_to_text");

        Self {
            metadata,
            state: PluginState::Unloaded,
            config: ASRPluginConfig::default(),
            engine: None,
            transcription_count: 0,
        }
    }

    /// Create an ASR plugin with engine
    pub fn with_engine<E: ASREngine + 'static>(plugin_id: &str, engine: E) -> Self {
        let mut plugin = Self::new(plugin_id);
        plugin.engine = Some(Arc::new(engine));
        plugin
    }

    /// Set the plugin configuration
    pub fn with_config(mut self, config: ASRPluginConfig) -> Self {
        self.config = config;
        self
    }

    /// Get the ASR engine
    pub fn engine(&self) -> Option<&Arc<dyn ASREngine>> {
        self.engine.as_ref()
    }

    /// Transcribe audio bytes to text
    pub async fn transcribe(&mut self, audio: &[u8]) -> PluginResult<String> {
        let engine = self.engine.as_ref().ok_or_else(|| {
            mofa_kernel::plugin::PluginError::ExecutionFailed(
                "ASR engine not initialized".to_string(),
            )
        })?;

        self.transcription_count += 1;
        engine
            .transcribe(audio, Some(&self.config.default_language))
            .await
    }

    /// Get usage statistics
    pub fn stats(&self) -> HashMap<String, serde_json::Value> {
        let mut stats = HashMap::new();
        stats.insert(
            "transcription_count".to_string(),
            serde_json::json!(self.transcription_count),
        );
        stats.insert(
            "default_language".to_string(),
            serde_json::json!(self.config.default_language),
        );
        if let Some(engine) = &self.engine {
            stats.insert("engine".to_string(), serde_json::json!(engine.name()));
        }
        stats
    }
}

#[async_trait::async_trait]
impl AgentPlugin for ASRPlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    fn state(&self) -> PluginState {
        self.state.clone()
    }

    async fn load(&mut self, ctx: &PluginContext) -> PluginResult<()> {
        self.state = PluginState::Loading;
        info!("Loading ASR plugin: {}", self.metadata.id);

        if let Some(lang) = ctx.config.get_string("default_language") {
            self.config.default_language = lang;
        }

        self.state = PluginState::Loaded;
        Ok(())
    }

    async fn init_plugin(&mut self) -> PluginResult<()> {
        info!("Initializing ASR plugin: {}", self.metadata.id);

        if self.engine.is_none() {
            warn!("No explicit ASR engine bound; falling back to mock engine");
            self.engine = Some(Arc::new(MockASREngine::new(self.config.clone())));
        }

        Ok(())
    }

    async fn start(&mut self) -> PluginResult<()> {
        self.state = PluginState::Running;
        info!("ASR plugin {} started", self.metadata.id);
        Ok(())
    }

    async fn stop(&mut self) -> PluginResult<()> {
        self.state = PluginState::Paused;
        info!("ASR plugin {} stopped", self.metadata.id);
        Ok(())
    }

    async fn unload(&mut self) -> PluginResult<()> {
        self.engine = None;
        self.state = PluginState::Unloaded;
        info!("ASR plugin {} unloaded", self.metadata.id);
        Ok(())
    }

    async fn execute(&mut self, _input: String) -> PluginResult<String> {
        Err(mofa_kernel::plugin::PluginError::ExecutionFailed(
            "Direct string execution not supported for ASRPlugin. Use transcribe() directly."
                .to_string(),
        ))
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any> {
        self
    }
}
