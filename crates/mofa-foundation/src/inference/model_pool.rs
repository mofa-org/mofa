//! LRU Model Pool with idle-timeout eviction.
//!
//! Manages the lifecycle of locally loaded models. Models are loaded
//! on demand, tracked for memory usage, and evicted either when they
//! exceed an idle timeout or when memory pressure requires reclaiming
//! resources.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use super::types::Precision;

/// An entry in the model pool representing a loaded model.
#[derive(Debug, Clone)]
pub struct ModelEntry {
    /// The model identifier
    pub model_id: String,
    /// Memory consumed by this model (in MB)
    pub memory_mb: usize,
    /// The precision/quantization level the model was loaded at
    pub precision: Precision,
    /// When the model was last used for inference
    pub last_used: Instant,
}

/// An LRU model pool that tracks loaded models, their memory footprint,
/// and supports both idle-timeout and capacity-based eviction.
#[derive(Debug)]
pub struct ModelPool {
    /// Currently loaded models, keyed by model_id
    loaded: HashMap<String, ModelEntry>,
    /// Maximum number of models that can be concurrently loaded
    capacity: usize,
    /// Models idle longer than this duration are candidates for eviction
    idle_timeout: Duration,
}

impl ModelPool {
    /// Create a new model pool with the given capacity and idle timeout.
    pub fn new(capacity: usize, idle_timeout: Duration) -> Self {
        Self {
            loaded: HashMap::new(),
            capacity,
            idle_timeout,
        }
    }

    /// Load a model into the pool.
    ///
    /// If the model is already loaded, its `last_used` timestamp is refreshed.
    /// If the pool is at capacity, the least-recently-used model is evicted first.
    ///
    /// Returns the model ID of any evicted model, or `None`.
    pub fn load(
        &mut self,
        model_id: &str,
        memory_mb: usize,
        precision: Precision,
    ) -> Option<String> {
        // If already loaded, just refresh the timestamp
        if let Some(entry) = self.loaded.get_mut(model_id) {
            entry.last_used = Instant::now();
            return None;
        }

        // Evict LRU if at capacity
        let evicted = if self.loaded.len() >= self.capacity {
            self.evict_lru()
        } else {
            None
        };

        self.loaded.insert(
            model_id.to_string(),
            ModelEntry {
                model_id: model_id.to_string(),
                memory_mb,
                precision,
                last_used: Instant::now(),
            },
        );

        evicted
    }

    /// Mark a model as recently used (refreshes its LRU timestamp).
    pub fn touch(&mut self, model_id: &str) {
        if let Some(entry) = self.loaded.get_mut(model_id) {
            entry.last_used = Instant::now();
        }
    }

    /// Check if a model is currently loaded.
    pub fn is_loaded(&self, model_id: &str) -> bool {
        self.loaded.contains_key(model_id)
    }

    /// Get information about a loaded model.
    pub fn get(&self, model_id: &str) -> Option<&ModelEntry> {
        self.loaded.get(model_id)
    }

    /// Explicitly unload a model, freeing its memory allocation.
    ///
    /// Returns the memory that was freed (in MB), or 0 if the model was not loaded.
    pub fn unload(&mut self, model_id: &str) -> usize {
        self.loaded
            .remove(model_id)
            .map(|entry| entry.memory_mb)
            .unwrap_or(0)
    }

    /// Total memory consumed by all currently loaded models (in MB).
    pub fn total_memory_mb(&self) -> usize {
        self.loaded.values().map(|e| e.memory_mb).sum()
    }

    /// Number of currently loaded models.
    pub fn len(&self) -> usize {
        self.loaded.len()
    }

    /// Returns true if no models are loaded.
    pub fn is_empty(&self) -> bool {
        self.loaded.is_empty()
    }

    /// Evict all models that have been idle longer than the configured timeout.
    ///
    /// Returns a list of evicted model IDs.
    pub fn evict_idle(&mut self) -> Vec<String> {
        let now = Instant::now();
        let idle_ids: Vec<String> = self
            .loaded
            .iter()
            .filter(|(_, entry)| now.duration_since(entry.last_used) > self.idle_timeout)
            .map(|(id, _)| id.clone())
            .collect();

        for id in &idle_ids {
            self.loaded.remove(id);
        }

        idle_ids
    }

    /// Evict the least-recently-used model.
    ///
    /// Returns the model ID of the evicted model, or `None` if the pool is empty.
    pub fn evict_lru(&mut self) -> Option<String> {
        let lru_id = self
            .loaded
            .iter()
            .min_by_key(|(_, entry)| entry.last_used)
            .map(|(id, _)| id.clone());

        if let Some(ref id) = lru_id {
            self.loaded.remove(id);
        }

        lru_id
    }

    /// Evict models until total memory drops below the given threshold (in MB).
    ///
    /// Evicts in LRU order. Returns the list of evicted model IDs.
    pub fn evict_until_below(&mut self, target_mb: usize) -> Vec<String> {
        let mut evicted = Vec::new();
        while self.total_memory_mb() > target_mb && !self.loaded.is_empty() {
            if let Some(id) = self.evict_lru() {
                evicted.push(id);
            }
        }
        evicted
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_and_query() {
        let mut pool = ModelPool::new(3, Duration::from_secs(300));

        pool.load("llama-3-13b", 13312, Precision::F16);
        assert!(pool.is_loaded("llama-3-13b"));
        assert!(!pool.is_loaded("gpt-4"));
        assert_eq!(pool.len(), 1);
        assert_eq!(pool.total_memory_mb(), 13312);
    }

    #[test]
    fn test_lru_eviction_at_capacity() {
        let mut pool = ModelPool::new(2, Duration::from_secs(300));

        pool.load("model-a", 1000, Precision::F16);
        pool.load("model-b", 2000, Precision::F16);

        // Pool is full (capacity=2). Loading a third model should evict model-a (oldest).
        let evicted = pool.load("model-c", 3000, Precision::Q8);
        assert_eq!(evicted, Some("model-a".to_string()));
        assert!(!pool.is_loaded("model-a"));
        assert!(pool.is_loaded("model-b"));
        assert!(pool.is_loaded("model-c"));
    }

    #[test]
    fn test_touch_refreshes_lru_order() {
        let mut pool = ModelPool::new(2, Duration::from_secs(300));

        pool.load("model-a", 1000, Precision::F16);
        std::thread::sleep(Duration::from_millis(10));
        pool.load("model-b", 2000, Precision::F16);
        std::thread::sleep(Duration::from_millis(10));

        // Touch model-a to make it recently used
        pool.touch("model-a");

        // Now model-b is the LRU, so loading a new model should evict model-b
        let evicted = pool.load("model-c", 500, Precision::Q4);
        assert_eq!(evicted, Some("model-b".to_string()));
        assert!(pool.is_loaded("model-a"));
        assert!(pool.is_loaded("model-c"));
    }

    #[test]
    fn test_total_memory_tracking() {
        let mut pool = ModelPool::new(10, Duration::from_secs(300));

        pool.load("model-a", 1000, Precision::F16);
        pool.load("model-b", 2000, Precision::F16);
        pool.load("model-c", 3000, Precision::Q8);

        assert_eq!(pool.total_memory_mb(), 6000);

        pool.unload("model-b");
        assert_eq!(pool.total_memory_mb(), 4000);
    }

    #[test]
    fn test_evict_until_below_target() {
        let mut pool = ModelPool::new(10, Duration::from_secs(300));

        pool.load("small", 1000, Precision::Q4);
        std::thread::sleep(Duration::from_millis(10));
        pool.load("medium", 4000, Precision::Q8);
        std::thread::sleep(Duration::from_millis(10));
        pool.load("large", 8000, Precision::F16);

        assert_eq!(pool.total_memory_mb(), 13000);

        // Evict until below 5000 MB
        let evicted = pool.evict_until_below(5000);
        // Should evict "small" (oldest) then "medium" to get to 8000... still above.
        // Then evict "large" to get to 0. Hmm, let's just check it works.
        assert!(pool.total_memory_mb() <= 5000);
        assert!(!evicted.is_empty());
    }

    #[test]
    fn test_reload_refreshes_timestamp() {
        let mut pool = ModelPool::new(3, Duration::from_secs(300));

        pool.load("model-a", 1000, Precision::F16);
        // Loading the same model again should just refresh, not add a duplicate
        let evicted = pool.load("model-a", 1000, Precision::F16);
        assert_eq!(evicted, None);
        assert_eq!(pool.len(), 1);
        assert_eq!(pool.total_memory_mb(), 1000);
    }

    #[test]
    fn test_unload_returns_freed_memory() {
        let mut pool = ModelPool::new(3, Duration::from_secs(300));

        pool.load("model-a", 4096, Precision::F16);
        let freed = pool.unload("model-a");
        assert_eq!(freed, 4096);

        // Unloading a model that doesn't exist returns 0
        let freed = pool.unload("nonexistent");
        assert_eq!(freed, 0);
    }
}
