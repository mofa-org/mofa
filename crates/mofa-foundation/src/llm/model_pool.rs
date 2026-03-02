use crate::llm::inference::{BackendHealth, InferenceBackend, InferenceError, InferenceResult, ModelHandle, ModelMetadata};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Semaphore};
use tokio::time::interval;

#[derive(Debug, Clone)]
pub struct ModelPoolConfig {
    pub max_models: usize,
    pub max_memory_gb: u32,
    pub idle_timeout_secs: u64,
    pub preload_on_start: Vec<String>,
}

impl Default for ModelPoolConfig {
    fn default() -> Self {
        Self {
            max_models: 3,
            max_memory_gb: 16,
            idle_timeout_secs: 300,
            preload_on_start: vec![],
        }
    }
}

#[derive(Debug)]
struct LoadedModel {
    handle: ModelHandle,
    loaded_at: Instant,
    last_used: Instant,
    reference_count: usize,
}

pub struct ModelPool {
    config: ModelPoolConfig,
    backend: Arc<dyn InferenceBackend>,
    models: RwLock<HashMap<String, LoadedModel>>,
    memory_used_gb: RwLock<u32>,
    semaphore: Semaphore,
}

impl ModelPool {
    pub fn new(config: ModelPoolConfig, backend: Arc<dyn InferenceBackend>) -> Self {
        let max_concurrent = config.max_models;
        Self {
            config,
            backend,
            models: RwLock::new(HashMap::new()),
            memory_used_gb: RwLock::new(0),
            semaphore: Semaphore::new(max_concurrent),
        }
    }

    pub async fn acquire(&self, model_id: &str) -> InferenceResult<ModelHandle> {
        let permit = self.semaphore.acquire().await.map_err(|_| {
            InferenceError::ResourceExhausted(\"Too many models loaded concurrently\".to_string())
        })?;

        let mut models = self.models.write().await;

        if let Some(loaded) = models.get_mut(model_id) {
            loaded.last_used = Instant::now();
            loaded.reference_count += 1;
            drop(permit);
            return Ok(loaded.handle.clone());
        }

        let handle = self.backend.load_model(model_id).await?;

        if models.len() >= self.config.max_models {
            self.evict_lru(&mut models).await?;
        }

        let memory_estimate_gb = self.estimate_model_memory(model_id);
        let mut memory_used = self.memory_used_gb.write().await;

        if *memory_used + memory_estimate_gb > self.config.max_memory_gb {
            return Err(InferenceError::ResourceExhausted(
                \"Insufficient memory to load model\".to_string(),
            ));
        }

        *memory_used += memory_estimate_gb;

        let loaded = LoadedModel {
            handle: handle.clone(),
            loaded_at: Instant::now(),
            last_used: Instant::now(),
            reference_count: 1,
        };

        models.insert(model_id.to_string(), loaded);
        drop(permit);
        drop(models);
        drop(memory_used);

        Ok(handle)
    }

    pub async fn release(&self, model_id: &str) {
        let mut models = self.models.write().await;
        if let Some(loaded) = models.get_mut(model_id) {
            loaded.reference_count = loaded.reference_count.saturating_sub(1);
            if loaded.reference_count == 0 {
                loaded.last_used = Instant::now();
            }
        }
    }

    async fn evict_lru(&self, models: &mut HashMap<String, LoadedModel>) -> InferenceResult<()> {
        if models.is_empty() {
            return Ok(());
        }

        let lru_key = models
            .iter()
            .filter(|(_, loaded)| loaded.reference_count == 0)
            .min_by_key(|(_, loaded)| loaded.last_used)
            .map(|(k, _)| k.clone());

        if let Some(key) = lru_key {
            if let Some(loaded) = models.remove(&key) {
                let _ = self.backend.unload_model(&key).await;
                let memory_freed = self.estimate_model_memory(&key);
                let mut memory_used = self.memory_used_gb.write().await;
                *memory_used = memory_used.saturating_sub(memory_freed);
            }
        }

        Ok(())
    }

    fn estimate_model_memory(&self, model_id: &str) -> u32 {
        match model_id {
            _ if model_id.contains(\"qwen\") || model_id.contains(\"llama\") => 8,
            _ if model_id.contains(\"gpt\") => 4,
            _ if model_id.contains(\"asr\") || model_id.contains(\"funasr\") => 2,
            _ if model_id.contains(\"tts\") || model_id.contains(\"sovits\") => 4,
            _ => 4,
        }
    }

    pub async fn start_idle_cleanup(&self) {
        let config = self.config.clone();
        let backend = self.backend.clone();
        let models = Arc::new(self.models.clone());
        let memory_used = Arc::new(self.memory_used_gb.clone());

        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(60));
            loop {
                ticker.tick().await;

                let mut models_guard = models.write().await;
                let idle_threshold = Duration::from_secs(config.idle_timeout_secs);
                let now = Instant::now();

                let to_evict: Vec<String> = models_guard
                    .iter()
                    .filter(|(_, loaded)| {
                        loaded.reference_count == 0 && now.duration_since(loaded.last_used) > idle_threshold
                    })
                    .map(|(k, _)| k.clone())
                    .collect();

                for key in to_evict {
                    if let Some(loaded) = models_guard.remove(&key) {
                        let _ = backend.unload_model(&key).await;
                        drop(loaded);
                    }
                }

                let mut memory = memory_used.write().await;
                *memory = models_guard
                    .iter()
                    .map(|(k, _)| {
                        match k.as_str() {
                            _ if k.contains(\"qwen\") || k.contains(\"llama\") => 8,
                            _ => 4,
                        }
                    })
                    .sum();
            }
        });
    }

    pub async fn get_loaded_models(&self) -> Vec<String> {
        let models = self.models.read().await;
        models.keys().cloned().collect()
    }

    pub async fn get_memory_usage(&self) -> u32 {
        *self.memory_used_gb.read().await
    }

    pub async fn health_check(&self) -> InferenceResult<BackendHealth> {
        self.backend.health_check().await
    }
}
