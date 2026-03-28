//! Capability Discovery Protocol (CDP) for `mofa-kernel`.
//!
//! A backend announces what it supports — models, modalities, maximum context
//! window, accepted tool-schema formats, and hardware class — via a structured
//! [`CapabilityManifest`].  A [`CapabilityRegistry`] provides an in-memory
//! index so the routing layer can select compatible backends **without**
//! sending a live request to every provider.
//!
//! # Complexity guarantees
//!
//! | Operation | Complexity |
//! |---|---|
//! | [`CapabilityRegistry::get`] | *O*(1) |
//! | [`CapabilityRegistry::register`] | *O*(1) amortised |
//! | [`CapabilityRegistry::unregister`] | *O*(1) |
//! | [`CapabilityRegistry::query`] | *O*(n × m) where *n* = number of registered providers, *m* = avg models per provider |
//!
//! # Quick start
//!
//! ```rust
//! use mofa_kernel::llm::cdp::{
//!     CapabilityManifest, CapabilityRegistry, CapabilityFilter,
//!     ModelEntry, Modality, HardwareClass,
//! };
//!
//! let entry = ModelEntry::builder("gpt-4o")
//!     .input_modalities([Modality::Text, Modality::Image])
//!     .output_modalities([Modality::Text])
//!     .max_context_tokens(128_000)
//!     .supports_tool_calling(true)
//!     .supports_streaming(true)
//!     .build();
//!
//! let manifest = CapabilityManifest::builder("openai", "1.0.0", HardwareClass::Cloud)
//!     .add_model(entry)
//!     .build();
//!
//! let mut registry = CapabilityRegistry::new();
//! registry.register(manifest).expect("first registration must succeed");
//!
//! // O(1) lookup
//! assert!(registry.get("openai").is_some());
//!
//! // Capability-filtered lookup
//! let filter = CapabilityFilter::new()
//!     .require_input_modality(Modality::Image)
//!     .min_context_tokens(100_000);
//!
//! let matches = registry.query(&filter);
//! assert_eq!(matches[0].0, "openai");
//! assert_eq!(matches[0].1, "gpt-4o");
//! ```

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

// ─── Error type ─────────────────────────────────────────────────────────────

/// Errors that can be raised by [`CapabilityRegistry`] operations.
#[derive(Debug, Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum CdpError {
    /// A provider with the given name was not found in the registry.
    #[error("provider not found: '{0}'")]
    ProviderNotFound(String),

    /// A provider with the given name is already registered.
    /// Use [`CapabilityRegistry::update`] to replace an existing entry.
    #[error("provider already registered: '{0}' — use update() to replace")]
    DuplicateProvider(String),
}

// ─── Hardware class ──────────────────────────────────────────────────────────

/// Coarse classification of the hardware environment running the model.
///
/// Used by the routing layer to prefer or exclude backends based on
/// hardware availability or latency requirements.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum HardwareClass {
    /// Consumer / server CPU — high latency, low throughput.
    Cpu,
    /// NVIDIA / AMD GPU — balanced throughput & latency.
    Gpu,
    /// Google / custom TPU — optimised for transformer workloads.
    Tpu,
    /// Hosted cloud API (provider manages hardware).
    Cloud,
    /// Hardware class is unknown or does not apply.
    Unknown,
}

// ─── Modality ────────────────────────────────────────────────────────────────

/// A data modality that a model can consume or produce.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Modality {
    /// Plain or structured text.
    Text,
    /// Static or animated images.
    Image,
    /// Audio waveforms or speech.
    Audio,
    /// Video streams.
    Video,
    /// Dense vector embeddings.
    Embedding,
    /// Source code (syntax-aware completion / analysis).
    Code,
}

// ─── Tool schema format ──────────────────────────────────────────────────────

/// The JSON-schema dialect used to describe function / tool parameters.
///
/// Different providers expect subtly different formats; this enum lets the
/// router select a backend that natively understands the schema the agent
/// already has in hand.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ToolSchemaFormat {
    /// OpenAI function-calling schema (JSON Schema subset).
    OpenAi,
    /// Anthropic tool-use schema.
    Anthropic,
    /// Custom schema format identified by name.
    Custom(String),
}

// ─── ModelEntry ──────────────────────────────────────────────────────────────

/// Describes the capabilities of a **single model** offered by a provider.
///
/// Build via [`ModelEntry::builder`] to benefit from compile-time defaults.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelEntry {
    /// Canonical model identifier (e.g. `"gpt-4o"`, `"claude-3-5-sonnet-20241022"`).
    pub model_id: String,

    /// Modalities the model can **receive** as input.
    pub input_modalities: Vec<Modality>,

    /// Modalities the model can **produce** as output.
    pub output_modalities: Vec<Modality>,

    /// Maximum supported context window expressed in tokens, if known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_context_tokens: Option<u32>,

    /// Whether the model supports OpenAI-style tool / function calling.
    pub supports_tool_calling: bool,

    /// Whether the model supports server-sent-event streaming responses.
    pub supports_streaming: bool,

    /// Schema formats the model's tool-calling interface accepts.
    pub tool_schema_formats: Vec<ToolSchemaFormat>,
}

impl ModelEntry {
    /// Begin constructing a [`ModelEntry`] with required fields.
    ///
    /// # Arguments
    /// * `model_id` — Canonical model identifier.
    pub fn builder(model_id: impl Into<String>) -> ModelEntryBuilder {
        ModelEntryBuilder::new(model_id)
    }
}

// ── ModelEntryBuilder ────────────────────────────────────────────────────────

/// Step-by-step builder for [`ModelEntry`].
///
/// Sensible defaults are provided for every optional field so callers only
/// need to override what differs from the common case.
#[derive(Debug)]
pub struct ModelEntryBuilder {
    model_id: String,
    input_modalities: Vec<Modality>,
    output_modalities: Vec<Modality>,
    max_context_tokens: Option<u32>,
    supports_tool_calling: bool,
    supports_streaming: bool,
    tool_schema_formats: Vec<ToolSchemaFormat>,
}

impl ModelEntryBuilder {
    fn new(model_id: impl Into<String>) -> Self {
        Self {
            model_id: model_id.into(),
            input_modalities: vec![Modality::Text],
            output_modalities: vec![Modality::Text],
            max_context_tokens: None,
            supports_tool_calling: false,
            supports_streaming: false,
            tool_schema_formats: vec![],
        }
    }

    /// Set the input modalities, replacing the default `[Text]`.
    pub fn input_modalities(mut self, m: impl IntoIterator<Item = Modality>) -> Self {
        self.input_modalities = m.into_iter().collect();
        self
    }

    /// Set the output modalities, replacing the default `[Text]`.
    pub fn output_modalities(mut self, m: impl IntoIterator<Item = Modality>) -> Self {
        self.output_modalities = m.into_iter().collect();
        self
    }

    /// Set the maximum supported context window in tokens.
    pub fn max_context_tokens(mut self, tokens: u32) -> Self {
        self.max_context_tokens = Some(tokens);
        self
    }

    /// Declare whether the model supports tool / function calling.
    pub fn supports_tool_calling(mut self, v: bool) -> Self {
        self.supports_tool_calling = v;
        self
    }

    /// Declare whether the model supports streaming responses.
    pub fn supports_streaming(mut self, v: bool) -> Self {
        self.supports_streaming = v;
        self
    }

    /// Append a supported tool-schema format.
    pub fn add_tool_schema_format(mut self, fmt: ToolSchemaFormat) -> Self {
        self.tool_schema_formats.push(fmt);
        self
    }

    /// Consume the builder and produce a [`ModelEntry`].
    pub fn build(self) -> ModelEntry {
        ModelEntry {
            model_id: self.model_id,
            input_modalities: self.input_modalities,
            output_modalities: self.output_modalities,
            max_context_tokens: self.max_context_tokens,
            supports_tool_calling: self.supports_tool_calling,
            supports_streaming: self.supports_streaming,
            tool_schema_formats: self.tool_schema_formats,
        }
    }
}

// ─── CapabilityManifest ──────────────────────────────────────────────────────

/// The complete capability declaration of a single provider backend.
///
/// A backend populates this struct at start-up and hands it to
/// [`CapabilityRegistry::register`].  The routing layer then consults
/// the registry to select a backend without making a live network call.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityManifest {
    /// Unique provider identifier (e.g. `"openai"`, `"ollama-local"`).
    /// Used as the primary, O(1)-lookup key in [`CapabilityRegistry`].
    pub provider_name: String,

    /// Provider or deployment version string (semver or arbitrary label).
    pub provider_version: String,

    /// Coarse description of the hardware backing this provider.
    pub hardware_class: HardwareClass,

    /// List of models this provider exposes.
    pub models: Vec<ModelEntry>,

    /// Arbitrary provider-specific metadata (base URL, region, tier, …).
    /// Stored as a JSON object; defaults to `{}` when not set.
    #[serde(default)]
    pub metadata: serde_json::Value,
}

impl CapabilityManifest {
    /// Begin building a [`CapabilityManifest`].
    ///
    /// # Arguments
    /// * `provider_name`    — unique provider key.
    /// * `provider_version` — version or deployment label.
    /// * `hardware_class`   — coarse hardware description.
    pub fn builder(
        provider_name: impl Into<String>,
        provider_version: impl Into<String>,
        hardware_class: HardwareClass,
    ) -> CapabilityManifestBuilder {
        CapabilityManifestBuilder::new(provider_name, provider_version, hardware_class)
    }
}

// ── CapabilityManifestBuilder ────────────────────────────────────────────────

/// Builder for [`CapabilityManifest`].
#[derive(Debug)]
pub struct CapabilityManifestBuilder {
    provider_name: String,
    provider_version: String,
    hardware_class: HardwareClass,
    models: Vec<ModelEntry>,
    metadata: serde_json::Value,
}

impl CapabilityManifestBuilder {
    fn new(
        provider_name: impl Into<String>,
        provider_version: impl Into<String>,
        hardware_class: HardwareClass,
    ) -> Self {
        Self {
            provider_name: provider_name.into(),
            provider_version: provider_version.into(),
            hardware_class,
            models: vec![],
            metadata: serde_json::Value::Object(serde_json::Map::new()),
        }
    }

    /// Append a [`ModelEntry`] to the manifest.
    pub fn add_model(mut self, model: ModelEntry) -> Self {
        self.models.push(model);
        self
    }

    /// Attach arbitrary JSON metadata.
    pub fn metadata(mut self, meta: serde_json::Value) -> Self {
        self.metadata = meta;
        self
    }

    /// Consume the builder and produce a [`CapabilityManifest`].
    pub fn build(self) -> CapabilityManifest {
        CapabilityManifest {
            provider_name: self.provider_name,
            provider_version: self.provider_version,
            hardware_class: self.hardware_class,
            models: self.models,
            metadata: self.metadata,
        }
    }
}

// ─── CapabilityFilter ────────────────────────────────────────────────────────

/// Predicate applied during a [`CapabilityRegistry::query`] scan.
///
/// Every `Some` field acts as an **AND** constraint: a model entry must
/// satisfy **all** supplied constraints to be included in the results.
///
/// # Examples
///
/// ```rust
/// use mofa_kernel::llm::cdp::{CapabilityFilter, Modality, HardwareClass};
///
/// let filter = CapabilityFilter::new()
///     .require_input_modality(Modality::Image)
///     .require_tool_calling()
///     .min_context_tokens(32_000);
/// ```
#[derive(Debug, Default, Clone)]
pub struct CapabilityFilter {
    /// Input modalities that *all* must be present.
    required_input_modalities: Vec<Modality>,
    /// Output modalities that *all* must be present.
    required_output_modalities: Vec<Modality>,
    /// Minimum context window size in tokens.
    min_context_tokens: Option<u32>,
    /// If `true`, only models with tool-calling support are matched.
    requires_tool_calling: Option<bool>,
    /// If `true`, only models that support streaming are matched.
    requires_streaming: Option<bool>,
    /// If set, only providers running on this hardware class are matched.
    hardware_class: Option<HardwareClass>,
    /// If set, only models accepting this tool-schema format are matched.
    tool_schema_format: Option<ToolSchemaFormat>,
}

impl CapabilityFilter {
    /// Create an empty filter that matches everything.
    pub fn new() -> Self {
        Self::default()
    }

    /// Require a specific input modality (cumulative; call multiple times).
    pub fn require_input_modality(mut self, m: Modality) -> Self {
        self.required_input_modalities.push(m);
        self
    }

    /// Require a specific output modality (cumulative; call multiple times).
    pub fn require_output_modality(mut self, m: Modality) -> Self {
        self.required_output_modalities.push(m);
        self
    }

    /// Require at least this many tokens in the context window.
    pub fn min_context_tokens(mut self, tokens: u32) -> Self {
        self.min_context_tokens = Some(tokens);
        self
    }

    /// Require tool / function-calling support.
    pub fn require_tool_calling(mut self) -> Self {
        self.requires_tool_calling = Some(true);
        self
    }

    /// Require streaming-response support.
    pub fn require_streaming(mut self) -> Self {
        self.requires_streaming = Some(true);
        self
    }

    /// Restrict results to a specific hardware class.
    pub fn hardware_class(mut self, hw: HardwareClass) -> Self {
        self.hardware_class = Some(hw);
        self
    }

    /// Restrict results to models that accept a specific tool-schema format.
    pub fn tool_schema_format(mut self, fmt: ToolSchemaFormat) -> Self {
        self.tool_schema_format = Some(fmt);
        self
    }

    // ── Private predicate helpers ─────────────────────────────────────────

    /// Returns `true` when `manifest` and `entry` satisfy every constraint.
    pub(crate) fn matches(&self, manifest: &CapabilityManifest, entry: &ModelEntry) -> bool {
        // Hardware class guard — matches at the manifest level.
        if let Some(ref hw) = self.hardware_class {
            if &manifest.hardware_class != hw {
                return false;
            }
        }

        // Input modalities.
        for m in &self.required_input_modalities {
            if !entry.input_modalities.contains(m) {
                return false;
            }
        }

        // Output modalities.
        for m in &self.required_output_modalities {
            if !entry.output_modalities.contains(m) {
                return false;
            }
        }

        // Minimum context window.
        if let Some(min_ctx) = self.min_context_tokens {
            match entry.max_context_tokens {
                Some(max_ctx) if max_ctx >= min_ctx => {}
                _ => return false,
            }
        }

        // Tool-calling support.
        if let Some(required_tc) = self.requires_tool_calling {
            if entry.supports_tool_calling != required_tc {
                return false;
            }
        }

        // Streaming support.
        if let Some(required_stream) = self.requires_streaming {
            if entry.supports_streaming != required_stream {
                return false;
            }
        }

        // Tool-schema format.
        if let Some(ref fmt) = self.tool_schema_format {
            if !entry.tool_schema_formats.contains(fmt) {
                return false;
            }
        }

        true
    }
}

// ─── CapabilityRegistry ──────────────────────────────────────────────────────

/// In-memory registry of provider [`CapabilityManifest`]s.
///
/// # Thread safety
///
/// [`CapabilityRegistry`] is **not** `Sync`; wrap it in an `Arc<RwLock<_>>`
/// when sharing across async tasks.
///
/// # Example
///
/// ```rust
/// use mofa_kernel::llm::cdp::{CapabilityRegistry, CapabilityManifest, HardwareClass};
///
/// let mut registry = CapabilityRegistry::new();
/// let manifest = CapabilityManifest::builder("my-provider", "0.1.0", HardwareClass::Gpu).build();
/// registry.register(manifest).unwrap();
/// assert_eq!(registry.len(), 1);
/// registry.unregister("my-provider").unwrap();
/// assert!(registry.is_empty());
/// ```
#[derive(Debug, Default)]
pub struct CapabilityRegistry {
    /// Primary index: provider_name → manifest.
    /// O(1) access by provider name.
    store: HashMap<String, CapabilityManifest>,
}

impl CapabilityRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    // ── CRUD ──────────────────────────────────────────────────────────────

    /// Register a new provider manifest.
    ///
    /// # Errors
    /// Returns [`CdpError::DuplicateProvider`] if a manifest for
    /// `manifest.provider_name` is already registered.  Use
    /// [`Self::update`] to replace an existing entry.
    pub fn register(&mut self, manifest: CapabilityManifest) -> Result<(), CdpError> {
        let key = manifest.provider_name.clone();
        if self.store.contains_key(&key) {
            return Err(CdpError::DuplicateProvider(key));
        }
        self.store.insert(key, manifest);
        Ok(())
    }

    /// Replace an existing manifest, or insert it if not yet registered.
    ///
    /// Unlike [`Self::register`], this method never fails on a duplicate.
    pub fn update(&mut self, manifest: CapabilityManifest) {
        self.store.insert(manifest.provider_name.clone(), manifest);
    }

    /// Remove a provider manifest from the registry.
    ///
    /// # Errors
    /// Returns [`CdpError::ProviderNotFound`] if no manifest with the given
    /// `provider_name` is registered.
    pub fn unregister(&mut self, provider_name: &str) -> Result<CapabilityManifest, CdpError> {
        self.store
            .remove(provider_name)
            .ok_or_else(|| CdpError::ProviderNotFound(provider_name.to_owned()))
    }

    // ── Lookups ───────────────────────────────────────────────────────────

    /// Look up a provider manifest by name.
    ///
    /// **Complexity**: *O*(1).
    pub fn get(&self, provider_name: &str) -> Option<&CapabilityManifest> {
        self.store.get(provider_name)
    }

    /// Look up a provider manifest by name and return a mutable reference.
    ///
    /// **Complexity**: *O*(1).
    pub fn get_mut(&mut self, provider_name: &str) -> Option<&mut CapabilityManifest> {
        self.store.get_mut(provider_name)
    }

    /// Return all manifests whose models satisfy `filter`.
    ///
    /// Each result is a `(provider_name, model_id)` pair so the caller can
    /// immediately identify which model to target.
    ///
    /// **Complexity**: *O*(n × m) where *n* = registered providers,
    /// *m* = average models per provider.
    pub fn query(&self, filter: &CapabilityFilter) -> Vec<(&str, &str)> {
        let mut results = Vec::new();
        for manifest in self.store.values() {
            for entry in &manifest.models {
                if filter.matches(manifest, entry) {
                    results.push((manifest.provider_name.as_str(), entry.model_id.as_str()));
                }
            }
        }
        results
    }

    /// Return all manifests wholesale (no filtering).
    pub fn all(&self) -> impl Iterator<Item = &CapabilityManifest> {
        self.store.values()
    }

    // ── Utility ───────────────────────────────────────────────────────────

    /// Number of providers currently registered.
    pub fn len(&self) -> usize {
        self.store.len()
    }

    /// `true` when no providers are registered.
    pub fn is_empty(&self) -> bool {
        self.store.is_empty()
    }

    /// Remove all registered manifests.
    pub fn clear(&mut self) {
        self.store.clear();
    }
}

// ─── Unit tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Helpers ──────────────────────────────────────────────────────────

    fn text_model(id: &str, ctx: u32) -> ModelEntry {
        ModelEntry::builder(id)
            .input_modalities([Modality::Text])
            .output_modalities([Modality::Text])
            .max_context_tokens(ctx)
            .supports_tool_calling(true)
            .supports_streaming(true)
            .add_tool_schema_format(ToolSchemaFormat::OpenAi)
            .build()
    }

    fn vision_model(id: &str, ctx: u32) -> ModelEntry {
        ModelEntry::builder(id)
            .input_modalities([Modality::Text, Modality::Image])
            .output_modalities([Modality::Text])
            .max_context_tokens(ctx)
            .supports_tool_calling(true)
            .supports_streaming(true)
            .add_tool_schema_format(ToolSchemaFormat::OpenAi)
            .build()
    }

    fn make_openai_manifest() -> CapabilityManifest {
        CapabilityManifest::builder("openai", "1.0.0", HardwareClass::Cloud)
            .add_model(text_model("gpt-3.5-turbo", 16_385))
            .add_model(vision_model("gpt-4o", 128_000))
            .build()
    }

    fn make_local_manifest() -> CapabilityManifest {
        CapabilityManifest::builder("ollama", "0.3.0", HardwareClass::Gpu)
            .add_model(text_model("mistral", 8_192))
            .build()
    }

    fn make_anthropic_manifest() -> CapabilityManifest {
        CapabilityManifest::builder("anthropic", "2.0.0", HardwareClass::Cloud)
            .add_model(
                ModelEntry::builder("claude-3-5-sonnet-20241022")
                    .input_modalities([Modality::Text, Modality::Image])
                    .output_modalities([Modality::Text])
                    .max_context_tokens(200_000)
                    .supports_tool_calling(true)
                    .supports_streaming(true)
                    .add_tool_schema_format(ToolSchemaFormat::Anthropic)
                    .build(),
            )
            .build()
    }

    // ── Registry CRUD ────────────────────────────────────────────────────

    #[test]
    fn register_and_get() {
        let mut reg = CapabilityRegistry::new();
        assert!(reg.is_empty());

        reg.register(make_openai_manifest()).unwrap();
        assert_eq!(reg.len(), 1);

        let m = reg.get("openai").expect("should find openai");
        assert_eq!(m.provider_name, "openai");
        assert_eq!(m.models.len(), 2);
    }

    #[test]
    fn register_duplicate_returns_error() {
        let mut reg = CapabilityRegistry::new();
        reg.register(make_openai_manifest()).unwrap();
        let err = reg.register(make_openai_manifest()).unwrap_err();
        assert!(matches!(err, CdpError::DuplicateProvider(ref n) if n == "openai"));
    }

    #[test]
    fn update_replaces_existing() {
        let mut reg = CapabilityRegistry::new();
        reg.register(make_openai_manifest()).unwrap();

        let updated = CapabilityManifest::builder("openai", "2.0.0", HardwareClass::Cloud)
            .add_model(text_model("gpt-4-turbo", 128_000))
            .build();
        reg.update(updated);

        let m = reg.get("openai").unwrap();
        assert_eq!(m.provider_version, "2.0.0");
        assert_eq!(m.models[0].model_id, "gpt-4-turbo");
    }

    #[test]
    fn update_inserts_when_absent() {
        let mut reg = CapabilityRegistry::new();
        reg.update(make_openai_manifest());
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn unregister_removes_entry() {
        let mut reg = CapabilityRegistry::new();
        reg.register(make_openai_manifest()).unwrap();
        let removed = reg.unregister("openai").unwrap();
        assert_eq!(removed.provider_name, "openai");
        assert!(reg.is_empty());
    }

    #[test]
    fn unregister_missing_returns_error() {
        let mut reg = CapabilityRegistry::new();
        let err = reg.unregister("ghost").unwrap_err();
        assert!(matches!(err, CdpError::ProviderNotFound(ref n) if n == "ghost"));
    }

    #[test]
    fn clear_empties_registry() {
        let mut reg = CapabilityRegistry::new();
        reg.register(make_openai_manifest()).unwrap();
        reg.register(make_local_manifest()).unwrap();
        assert_eq!(reg.len(), 2);
        reg.clear();
        assert!(reg.is_empty());
    }

    #[test]
    fn get_mut_allows_in_place_modification() {
        let mut reg = CapabilityRegistry::new();
        reg.register(make_openai_manifest()).unwrap();
        {
            let m = reg.get_mut("openai").unwrap();
            m.provider_version = "99.0.0".to_owned();
        }
        assert_eq!(reg.get("openai").unwrap().provider_version, "99.0.0");
    }

    // ── Query / filter ───────────────────────────────────────────────────

    #[test]
    fn query_no_filter_returns_all_models() {
        let mut reg = CapabilityRegistry::new();
        reg.register(make_openai_manifest()).unwrap();
        reg.register(make_local_manifest()).unwrap();

        let results = reg.query(&CapabilityFilter::new());
        // openai has 2 models, ollama has 1 → total 3
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn query_by_input_modality_image() {
        let mut reg = CapabilityRegistry::new();
        reg.register(make_openai_manifest()).unwrap();
        reg.register(make_local_manifest()).unwrap();
        reg.register(make_anthropic_manifest()).unwrap();

        let filter = CapabilityFilter::new().require_input_modality(Modality::Image);
        let results = reg.query(&filter);

        // gpt-4o and claude-3-5-sonnet both accept images; gpt-3.5-turbo and mistral do not.
        assert_eq!(results.len(), 2);
        let model_ids: Vec<&str> = results.iter().map(|(_, m)| *m).collect();
        assert!(model_ids.contains(&"gpt-4o"));
        assert!(model_ids.contains(&"claude-3-5-sonnet-20241022"));
    }

    #[test]
    fn query_by_min_context_tokens() {
        let mut reg = CapabilityRegistry::new();
        reg.register(make_openai_manifest()).unwrap();
        reg.register(make_local_manifest()).unwrap();
        reg.register(make_anthropic_manifest()).unwrap();

        let filter = CapabilityFilter::new().min_context_tokens(100_000);
        let results = reg.query(&filter);

        // gpt-4o (128k) and claude-3-5-sonnet (200k) qualify; others do not.
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn query_by_hardware_class() {
        let mut reg = CapabilityRegistry::new();
        reg.register(make_openai_manifest()).unwrap();
        reg.register(make_local_manifest()).unwrap();

        let filter = CapabilityFilter::new().hardware_class(HardwareClass::Gpu);
        let results = reg.query(&filter);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, "ollama");
    }

    #[test]
    fn query_requires_tool_calling() {
        let mut reg = CapabilityRegistry::new();
        // Add a model that does NOT support tool calling.
        let no_tool_manifest = CapabilityManifest::builder("tinyml", "0.1.0", HardwareClass::Cpu)
            .add_model(
                ModelEntry::builder("phi-1.5")
                    .supports_tool_calling(false)
                    .supports_streaming(false)
                    .build(),
            )
            .build();
        reg.register(no_tool_manifest).unwrap();
        reg.register(make_openai_manifest()).unwrap();

        let filter = CapabilityFilter::new().require_tool_calling();
        let results = reg.query(&filter);

        // Only gpt-3.5-turbo and gpt-4o qualify (phi-1.5 does not).
        assert_eq!(results.len(), 2);
        let providers: Vec<&str> = results.iter().map(|(p, _)| *p).collect();
        assert!(providers.iter().all(|&p| p == "openai"));
    }

    #[test]
    fn query_by_tool_schema_format() {
        let mut reg = CapabilityRegistry::new();
        reg.register(make_openai_manifest()).unwrap();
        reg.register(make_anthropic_manifest()).unwrap();

        let filter = CapabilityFilter::new().tool_schema_format(ToolSchemaFormat::Anthropic);
        let results = reg.query(&filter);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, "anthropic");
    }

    #[test]
    fn query_combined_filters() {
        let mut reg = CapabilityRegistry::new();
        reg.register(make_openai_manifest()).unwrap();
        reg.register(make_local_manifest()).unwrap();
        reg.register(make_anthropic_manifest()).unwrap();

        // Vision + large context + streaming — only gpt-4o and claude qualify.
        let filter = CapabilityFilter::new()
            .require_input_modality(Modality::Image)
            .min_context_tokens(64_000)
            .require_streaming();

        let results = reg.query(&filter);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn query_no_match_returns_empty() {
        let mut reg = CapabilityRegistry::new();
        reg.register(make_local_manifest()).unwrap();

        // mistral has 8 192 tokens; requiring 1M will match nothing.
        let filter = CapabilityFilter::new().min_context_tokens(1_000_000);
        assert!(reg.query(&filter).is_empty());
    }

    // ── Serialization ────────────────────────────────────────────────────

    #[test]
    fn manifest_round_trips_json() {
        let original = make_openai_manifest();
        let json = serde_json::to_string(&original).expect("serialize");
        let restored: CapabilityManifest = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(original, restored);
    }

    #[test]
    fn manifest_json_contains_expected_fields() {
        let manifest = make_openai_manifest();
        let json = serde_json::to_string(&manifest).unwrap();
        assert!(json.contains("\"provider_name\""));
        assert!(json.contains("\"openai\""));
        assert!(json.contains("\"hardware_class\""));
        assert!(json.contains("\"cloud\""));
        assert!(json.contains("\"gpt-4o\""));
    }

    #[test]
    fn model_entry_round_trips_json() {
        let entry = vision_model("gpt-4o", 128_000);
        let json = serde_json::to_string(&entry).expect("serialize");
        let restored: ModelEntry = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(entry, restored);
    }

    #[test]
    fn hardware_class_serializes_snake_case() {
        let json = serde_json::to_string(&HardwareClass::Cloud).unwrap();
        assert_eq!(json, "\"cloud\"");

        let json = serde_json::to_string(&HardwareClass::Tpu).unwrap();
        assert_eq!(json, "\"tpu\"");
    }

    #[test]
    fn modality_serializes_snake_case() {
        let json = serde_json::to_string(&Modality::Embedding).unwrap();
        assert_eq!(json, "\"embedding\"");
    }

    // ── Edge cases ───────────────────────────────────────────────────────

    #[test]
    fn manifest_with_no_models_is_valid() {
        let manifest = CapabilityManifest::builder("empty", "0.0.1", HardwareClass::Unknown).build();
        let mut reg = CapabilityRegistry::new();
        reg.register(manifest).unwrap();

        let results = reg.query(&CapabilityFilter::new());
        assert!(results.is_empty());
    }

    #[test]
    fn model_without_context_window_excluded_by_min_ctx_filter() {
        let entry = ModelEntry::builder("no-ctx-model")
            .supports_tool_calling(false)
            .supports_streaming(false)
            .build(); // max_context_tokens = None

        let manifest =
            CapabilityManifest::builder("unknown-provider", "1.0.0", HardwareClass::Cpu)
                .add_model(entry)
                .build();

        let mut reg = CapabilityRegistry::new();
        reg.register(manifest).unwrap();

        let filter = CapabilityFilter::new().min_context_tokens(1); // any positive value
        assert!(reg.query(&filter).is_empty());
    }

    #[test]
    fn all_iterator_yields_every_manifest() {
        let mut reg = CapabilityRegistry::new();
        reg.register(make_openai_manifest()).unwrap();
        reg.register(make_local_manifest()).unwrap();

        let names: Vec<&str> = reg.all().map(|m| m.provider_name.as_str()).collect();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"openai"));
        assert!(names.contains(&"ollama"));
    }

    #[test]
    fn cdp_error_display_messages() {
        let dup = CdpError::DuplicateProvider("openai".to_owned());
        assert!(dup.to_string().contains("openai"));

        let missing = CdpError::ProviderNotFound("ghost".to_owned());
        assert!(missing.to_string().contains("ghost"));
    }

    #[test]
    fn custom_tool_schema_format_roundtrips() {
        let fmt = ToolSchemaFormat::Custom("my-schema-v3".to_owned());
        let json = serde_json::to_string(&fmt).unwrap();
        let restored: ToolSchemaFormat = serde_json::from_str(&json).unwrap();
        assert_eq!(fmt, restored);
    }
}
