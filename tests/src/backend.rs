//! Mock inference backend for deterministic agent testing.

use async_trait::async_trait;
use mofa_foundation::orchestrator::{
    ModelOrchestrator, ModelProviderConfig, ModelType, OrchestratorError, OrchestratorResult,
    PoolStatistics,
};
use std::collections::HashSet;
use std::sync::{Arc, RwLock};

/// Deterministic mock implementation of [`ModelOrchestrator`].
pub struct MockLLMBackend {
    /// Ordered response rules — first match wins
    responses: Arc<RwLock<Vec<(String, String)>>>,
    /// Fallback when nothing matches
    fallback: String,
    /// Registered model IDs
    registered: Arc<RwLock<HashSet<String>>>,
    /// Loaded model IDs
    loaded: Arc<RwLock<HashSet<String>>>,
    /// Memory threshold (bytes)
    memory_threshold: Arc<RwLock<u64>>,
    /// Idle timeout (seconds)
    idle_timeout_secs: Arc<RwLock<u64>>,
}

impl Default for MockLLMBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl MockLLMBackend {
    /// Create a new backend with an empty response table.
    pub fn new() -> Self {
        Self {
            responses: Arc::new(RwLock::new(Vec::new())),
            fallback: "Mock fallback response.".into(),
            registered: Arc::new(RwLock::new(HashSet::new())),
            loaded: Arc::new(RwLock::new(HashSet::new())),
            memory_threshold: Arc::new(RwLock::new(u64::MAX)),
            idle_timeout_secs: Arc::new(RwLock::new(300)),
        }
    }

    /// Append a response rule.  Order determines priority (first match wins).
    pub fn add_response(&self, prompt_substring: &str, response: &str) {
        self.responses
            .write()
            .expect("lock poisoned")
            .push((prompt_substring.to_string(), response.to_string()));
    }

    /// Replace the fallback response returned when no rule matches.
    pub fn set_fallback(&mut self, response: &str) {
        self.fallback = response.to_string();
    }

    /// Look up the response for a given prompt (first-match semantics).
    fn resolve(&self, prompt: &str) -> String {
        let rules = self.responses.read().expect("lock poisoned");
        for (key, value) in rules.iter() {
            if prompt.contains(key.as_str()) {
                return value.clone();
            }
        }
        self.fallback.clone()
    }
}

#[async_trait]
impl ModelOrchestrator for MockLLMBackend {
    fn name(&self) -> &str {
        "MockLLMBackend"
    }

    // -- registration --------------------------------------------------------

    async fn register_model(&self, config: ModelProviderConfig) -> OrchestratorResult<()> {
        self.registered
            .write()
            .expect("lock poisoned")
            .insert(config.model_name);
        Ok(())
    }

    async fn unregister_model(&self, model_id: &str) -> OrchestratorResult<()> {
        self.loaded
            .write()
            .expect("lock poisoned")
            .remove(model_id);
        self.registered
            .write()
            .expect("lock poisoned")
            .remove(model_id);
        Ok(())
    }

    // -- lifecycle -----------------------------------------------------------

    async fn load_model(&self, model_id: &str) -> OrchestratorResult<()> {
        if !self.registered.read().expect("lock poisoned").contains(model_id) {
            return Err(OrchestratorError::ModelNotFound(model_id.to_string()));
        }
        self.loaded
            .write()
            .expect("lock poisoned")
            .insert(model_id.to_string());
        Ok(())
    }

    async fn unload_model(&self, model_id: &str) -> OrchestratorResult<()> {
        self.loaded
            .write()
            .expect("lock poisoned")
            .remove(model_id);
        Ok(())
    }

    fn is_model_loaded(&self, model_id: &str) -> bool {
        self.loaded
            .read()
            .expect("lock poisoned")
            .contains(model_id)
    }

    // -- inference -----------------------------------------------------------

    async fn infer(&self, _model_id: &str, input: &str) -> OrchestratorResult<String> {
        Ok(self.resolve(input))
    }

    async fn route_by_type(&self, task: &ModelType) -> OrchestratorResult<String> {
        // Return the first registered model (deterministic since HashSet→Vec)
        let registered = self.registered.read().expect("lock poisoned");
        registered
            .iter()
            .next()
            .cloned()
            .ok_or_else(|| OrchestratorError::NoModelForType(task.to_string()))
    }

    // -- introspection -------------------------------------------------------

    fn get_statistics(&self) -> OrchestratorResult<PoolStatistics> {
        Ok(PoolStatistics {
            loaded_models_count: self.loaded.read().expect("lock poisoned").len(),
            total_memory_usage: 0,
            available_memory: u64::MAX,
            queued_models_count: 0,
            timestamp: chrono::Utc::now(),
        })
    }

    fn list_models(&self) -> Vec<String> {
        self.registered
            .read()
            .expect("lock poisoned")
            .iter()
            .cloned()
            .collect()
    }

    fn list_loaded_models(&self) -> Vec<String> {
        self.loaded
            .read()
            .expect("lock poisoned")
            .iter()
            .cloned()
            .collect()
    }

    // -- memory management ---------------------------------------------------

    async fn trigger_eviction(&self, _target_bytes: u64) -> OrchestratorResult<usize> {
        // Mock: evict everything
        let mut loaded = self.loaded.write().expect("lock poisoned");
        let count = loaded.len();
        loaded.clear();
        Ok(count)
    }

    async fn set_memory_threshold(&self, bytes: u64) -> OrchestratorResult<()> {
        *self.memory_threshold.write().expect("lock poisoned") = bytes;
        Ok(())
    }

    fn get_memory_threshold(&self) -> u64 {
        *self.memory_threshold.read().expect("lock poisoned")
    }

    async fn set_idle_timeout_secs(&self, secs: u64) -> OrchestratorResult<()> {
        *self.idle_timeout_secs.write().expect("lock poisoned") = secs;
        Ok(())
    }

    fn get_idle_timeout_secs(&self) -> u64 {
        *self.idle_timeout_secs.read().expect("lock poisoned")
    }
}
