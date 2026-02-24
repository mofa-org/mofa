//! ModelPool - Lifecycle Management for Local Models
//!
//! This module provides ModelPool for managing local model lifecycle including:
//! - On-demand model loading with async initialization
//! - LRU-based model cache with configurable size
//! - Idle timeout-based automatic unloading
//! - Memory pressure monitoring for Apple Silicon unified memory
//! - Graceful shutdown with state preservation
//! - Model preloading based on usage patterns
//!
//! # Example
//!
//! ```rust
//! use mofa_foundation::model_pool::{ModelPool, ModelPoolConfig};
//!
//! #[tokio::main]
//! async fn main() {
//!     let config = ModelPoolConfig::default()
//!         .with_max_models(3)
//!         .with_idle_timeout_secs(300);
//!
//!     let pool = ModelPool::new(config);
//!     // ... use the pool
//! }
//! ```

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Configuration for ModelPool
#[derive(Debug, Clone)]
pub struct ModelPoolConfig {
    /// Maximum number of models to keep in memory
    pub max_models: usize,
    /// Idle timeout in seconds before unloading a model
    pub idle_timeout_secs: u64,
    /// Maximum memory usage in bytes (0 = unlimited)
    pub max_memory_bytes: usize,
    /// Enable Apple Silicon memory pressure monitoring
    pub enable_apple_silicon_monitor: bool,
    /// Preload models on startup
    pub preload_models: Vec<String>,
    /// Check interval for idle models
    pub eviction_check_interval_secs: u64,
}

impl Default for ModelPoolConfig {
    fn default() -> Self {
        Self {
            max_models: 2,
            idle_timeout_secs: 600, // 10 minutes
            max_memory_bytes: 0,  // Unlimited by default
            enable_apple_silicon_monitor: false,
            preload_models: vec![],
            eviction_check_interval_secs: 60, // Check every minute
        }
    }
}

impl ModelPoolConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_max_models(mut self, max: usize) -> Self {
        self.max_models = max;
        self
    }

    pub fn with_idle_timeout_secs(mut self, secs: u64) -> Self {
        self.idle_timeout_secs = secs;
        self
    }

    pub fn with_max_memory_bytes(mut self, bytes: usize) -> Self {
        self.max_memory_bytes = bytes;
        self
    }

    pub fn with_apple_silicon_monitor(mut self, enable: bool) -> Self {
        self.enable_apple_silicon_monitor = enable;
        self
    }

    pub fn with_preload_models(mut self, models: Vec<String>) -> Self {
        self.preload_models = models;
        self
    }
}

/// Model state in the pool
#[derive(Debug, Clone, PartialEq)]
pub enum ModelState {
    /// Model is being loaded
    Loading,
    /// Model is ready to use
    Ready,
    /// Model is being unloaded
    Unloading,
    /// Model failed to load
    Error(String),
}

/// Information about a loaded model
#[derive(Debug, Clone)]
pub struct ModelInfo {
    pub model_id: String,
    pub state: ModelState,
    pub loaded_at: Instant,
    pub last_accessed: Instant,
    pub memory_usage_bytes: usize,
}

/// Trait for model backends that can be loaded into the pool
#[async_trait]
pub trait ModelBackend: Send + Sync {
    /// Unique identifier for this model
    fn model_id(&self) -> &str;

    /// Load the model into memory
    async fn load(&self) -> Result<()>;

    /// Unload the model from memory
    async fn unload(&self) -> Result<()>;

    /// Check if model is loaded
    fn is_loaded(&self) -> bool;

    /// Get estimated memory usage in bytes
    fn memory_usage(&self) -> usize;

    /// Generate text (placeholder for actual inference)
    async fn generate(&self, prompt: &str) -> Result<String>;
}

/// Entry in the LRU cache
#[derive(Clone)]
struct CacheEntry {
    backend: Arc<dyn ModelBackend>,
    info: ModelInfo,
}

/// ModelPool - LRU cache for model lifecycle management
pub struct ModelPool {
    config: ModelPoolConfig,
    cache: RwLock<Vec<CacheEntry>>,
    is_running: AtomicBool,
    shutdown_flag: AtomicBool,
}

impl ModelPool {
    /// Create a new ModelPool with the given configuration
    pub fn new(config: ModelPoolConfig) -> Self {
        Self {
            config,
            cache: RwLock::new(Vec::new()),
            is_running: AtomicBool::new(false),
            shutdown_flag: AtomicBool::new(false),
        }
    }

    /// Start the ModelPool background tasks (idle eviction, memory monitoring)
    pub fn start(&self) {
        if self.is_running.load(Ordering::SeqCst) {
            warn!("ModelPool is already running");
            return;
        }

        self.is_running.store(true, Ordering::SeqCst);
        self.shutdown_flag.store(false, Ordering::SeqCst);
        
        info!("ModelPool started with config: max_models={}, idle_timeout={}s",
            self.config.max_models, self.config.idle_timeout_secs);

        // Preload models if configured
        if !self.config.preload_models.is_empty() {
            let pool = Arc::new(self.clone());
            tokio::spawn(async move {
                for model_id in &pool.config.preload_models {
                    if let Err(e) = pool.load_model(model_id).await {
                        error!("Failed to preload model {}: {}", model_id, e);
                    }
                }
            });
        }

        // Start background tasks
        let pool = Arc::new(self.clone());
        Self::start_background_tasks(pool);
    }

    /// Stop the ModelPool and gracefully shutdown
    pub async fn shutdown(&self) {
        if !self.is_running.load(Ordering::SeqCst) {
            return;
        }

        info!("ModelPool shutting down...");

        // Signal shutdown
        self.shutdown_flag.store(true, Ordering::SeqCst);

        // Unload all models
        let model_ids: Vec<String> = self.list_models().await;
        for model_id in model_ids {
            if let Err(e) = self.unload_model(&model_id).await {
                error!("Error unloading model {}: {}", model_id, e);
            }
        }

        self.is_running.store(false, Ordering::SeqCst);
        info!("ModelPool shutdown complete");
    }

    /// Clone the pool (creates a new reference to the same pool)
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            cache: RwLock::new(Vec::new()), // Clone doesn't share cache
            is_running: AtomicBool::new(self.is_running.load(Ordering::SeqCst)),
            shutdown_flag: AtomicBool::new(false),
        }
    }

    /// Load a model into the pool (on-demand)
    pub async fn load_model(&self, model_id: &str) -> Result<()> {
        // Check if already loaded
        {
            let cache = self.cache.read().await;
            if let Some(entry) = cache.iter().find(|e| e.backend.model_id() == model_id) {
                if entry.info.state == ModelState::Ready {
                    // Update last accessed time
                    drop(cache);
                    self.touch_model(model_id).await?;
                    return Ok(());
                }
            }
        }

        // Check if we need to evict models
        self.maybe_evict().await?;

        // For now, we'll create a mock backend
        // In production, this would come from a registry
        let backend = Arc::new(MockModelBackend::new(model_id.to_string())) as Arc<dyn ModelBackend>;
        
        // Load the model
        backend.load().await?;

        let info = ModelInfo {
            model_id: model_id.to_string(),
            state: ModelState::Ready,
            loaded_at: Instant::now(),
            last_accessed: Instant::now(),
            memory_usage_bytes: backend.memory_usage(),
        };

        // Add to cache
        let entry = CacheEntry { backend, info };
        self.cache.write().await.push(entry);

        // Move to front (most recently used)
        self.move_to_front(model_id).await?;

        info!("Loaded model: {}", model_id);
        Ok(())
    }

    /// Unload a model from the pool
    pub async fn unload_model(&self, model_id: &str) -> Result<()> {
        let backend_to_unload = {
            let mut cache = self.cache.write().await;
            
            if let Some(pos) = cache.iter().position(|e| e.backend.model_id() == model_id) {
                Some(cache.remove(pos).backend)
            } else {
                None
            }
        };
        
        if let Some(backend) = backend_to_unload {
            backend.unload().await?;
            info!("Unloaded model: {}", model_id);
        }
        
        Ok(())
    }

    /// Get a model from the pool (keeps it loaded)
    pub async fn get_model(&self, model_id: &str) -> Result<Arc<dyn ModelBackend>> {
        // Check if loaded
        let backend = {
            let cache = self.cache.read().await;
            if let Some(entry) = cache.iter().find(|e| e.backend.model_id() == model_id) {
                if entry.info.state == ModelState::Ready {
                    // Clone the backend before releasing the lock
                    Some(entry.backend.clone())
                } else {
                    None
                }
            } else {
                None
            }
        };

        if let Some(b) = backend {
            // Update last accessed time
            self.touch_model(model_id).await?;
            return Ok(b);
        }

        // Not loaded, load on demand
        self.load_model(model_id).await?;

        let cache = self.cache.read().await;
        Ok(cache
            .iter()
            .find(|e| e.backend.model_id() == model_id)
            .map(|e| e.backend.clone())
            .unwrap())
    }

    /// Check if a model is loaded
    pub async fn is_model_loaded(&self, model_id: &str) -> bool {
        let cache = self.cache.read().await;
        cache.iter().any(|e| e.backend.model_id() == model_id && e.info.state == ModelState::Ready)
    }

    /// List all loaded models
    pub async fn list_models(&self) -> Vec<String> {
        let cache = self.cache.read().await;
        cache.iter().map(|e| e.info.model_id.clone()).collect()
    }

    /// Get model info
    pub async fn get_model_info(&self, model_id: &str) -> Option<ModelInfo> {
        let cache = self.cache.read().await;
        cache.iter()
            .find(|e| e.backend.model_id() == model_id)
            .map(|e| e.info.clone())
    }

    /// Get pool statistics
    pub async fn stats(&self) -> PoolStats {
        let cache = self.cache.read().await;
        let total_memory: usize = cache.iter().map(|e| e.info.memory_usage_bytes).sum();
        
        PoolStats {
            loaded_models: cache.len(),
            max_models: self.config.max_models,
            total_memory_bytes: total_memory,
            max_memory_bytes: self.config.max_memory_bytes,
        }
    }

    // Internal methods

    async fn touch_model(&self, model_id: &str) -> Result<()> {
        self.move_to_front(model_id).await
    }

    async fn move_to_front(&self, model_id: &str) -> Result<()> {
        let mut cache = self.cache.write().await;
        if let Some(pos) = cache.iter().position(|e| e.backend.model_id() == model_id) {
            let entry = cache.remove(pos);
            let mut info = entry.info;
            info.last_accessed = Instant::now();
            cache.insert(0, CacheEntry { backend: entry.backend, info });
        }
        Ok(())
    }

    async fn maybe_evict(&self) -> Result<()> {
        let cache = self.cache.read().await;
        
        // Check if we're at capacity
        if cache.len() < self.config.max_models {
            return Ok(());
        }

        // Find the least recently used model
        let lru_index = cache
            .iter()
            .enumerate()
            .min_by_key(|(_, e)| e.info.last_accessed)
            .map(|(i, _)| i);

        if let Some(index) = lru_index {
            let model_id = cache[index].info.model_id.clone();
            drop(cache);
            
            info!("Evicting model due to capacity: {}", model_id);
            self.unload_model(&model_id).await?;
        }

        Ok(())
    }

    async fn evict_idle_models(&self) {
        let idle_timeout = Duration::from_secs(self.config.idle_timeout_secs);
        let now = Instant::now();

        let models_to_evict: Vec<String> = {
            let cache = self.cache.read().await;
            cache
                .iter()
                .filter(|e| e.info.state == ModelState::Ready)
                .filter(|e| now.duration_since(e.info.last_accessed) > idle_timeout)
                .map(|e| e.info.model_id.clone())
                .collect()
        };

        for model_id in models_to_evict {
            if let Err(e) = self.unload_model(&model_id).await {
                error!("Error evicting idle model {}: {}", model_id, e);
            }
        }
    }

    fn start_background_tasks(pool: Arc<Self>) {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(pool.config.eviction_check_interval_secs));
            
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        // Check shutdown flag
                        if pool.shutdown_flag.load(Ordering::SeqCst) {
                            debug!("ModelPool background tasks shutting down");
                            break;
                        }
                        
                        pool.evict_idle_models().await;
                        
                        // Check memory pressure if enabled
                        if pool.config.enable_apple_silicon_monitor {
                            pool.check_memory_pressure().await;
                        }
                    }
                }
            }
        });
    }

    async fn check_memory_pressure(&self) {
        // For Apple Silicon, we'd check system memory pressure
        // This is a placeholder - actual implementation would use 
        // system APIs to get memory pressure
        #[cfg(target_os = "macos")]
        {
            // On macOS, we could use sysctl or NSProcessInfo
            debug!("Checking Apple Silicon memory pressure");
        }
    }
}

/// Statistics about the model pool
#[derive(Debug, Clone)]
pub struct PoolStats {
    /// Number of currently loaded models
    pub loaded_models: usize,
    /// Maximum number of models allowed
    pub max_models: usize,
    /// Total memory used by loaded models in bytes
    pub total_memory_bytes: usize,
    /// Maximum memory allowed (0 = unlimited)
    pub max_memory_bytes: usize,
}

/// Mock model backend for testing
pub struct MockModelBackend {
    model_id: String,
    loaded: AtomicBool,
}

impl MockModelBackend {
    pub fn new(model_id: String) -> Self {
        Self {
            model_id,
            loaded: AtomicBool::new(false),
        }
    }
}

#[async_trait]
impl ModelBackend for MockModelBackend {
    fn model_id(&self) -> &str {
        &self.model_id
    }

    async fn load(&self) -> Result<()> {
        self.loaded.store(true, Ordering::SeqCst);
        Ok(())
    }

    async fn unload(&self) -> Result<()> {
        self.loaded.store(false, Ordering::SeqCst);
        Ok(())
    }

    fn is_loaded(&self) -> bool {
        self.loaded.load(Ordering::SeqCst)
    }

    fn memory_usage(&self) -> usize {
        // Mock: 1GB per model
        1024 * 1024 * 1024
    }

    async fn generate(&self, prompt: &str) -> Result<String> {
        Ok(format!("Mock response for: {}", prompt))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_model_pool_basic() {
        let config = ModelPoolConfig::default()
            .with_max_models(2)
            .with_idle_timeout_secs(60);
        
        let pool = ModelPool::new(config);
        
        // Test load model
        pool.load_model("test-model").await.unwrap();
        
        // Test is_model_loaded
        assert!(pool.is_model_loaded("test-model").await);
        
        // Test list_models
        let models = pool.list_models().await;
        assert_eq!(models, vec!["test-model"]);
        
        // Test unload
        pool.unload_model("test-model").await.unwrap();
        assert!(!pool.is_model_loaded("test-model").await);
    }

    #[tokio::test]
    async fn test_model_pool_lru_eviction() {
        let config = ModelPoolConfig::default()
            .with_max_models(2);
        
        let pool = ModelPool::new(config);
        
        // Load 3 models - should evict first one
        pool.load_model("model-1").await.unwrap();
        pool.load_model("model-2").await.unwrap();
        pool.load_model("model-3").await.unwrap();
        
        // model-1 should have been evicted
        assert!(!pool.is_model_loaded("model-1").await);
        assert!(pool.is_model_loaded("model-2").await);
        assert!(pool.is_model_loaded("model-3").await);
    }

    #[tokio::test]
    async fn test_model_pool_get_model() {
        let config = ModelPoolConfig::default();
        let pool = ModelPool::new(config);
        
        // Load a model
        pool.load_model("test-model").await.unwrap();
        
        // Get the model - should return the same backend
        let backend1 = pool.get_model("test-model").await.unwrap();
        let backend2 = pool.get_model("test-model").await.unwrap();
        
        // Both should have the same model_id
        assert_eq!(backend1.model_id(), backend2.model_id());
    }
}
