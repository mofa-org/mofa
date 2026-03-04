//! LRU Model Pool with idle-timeout eviction.
//!
//! Manages the lifecycle of locally loaded models. Models are loaded
//! on demand, tracked for memory usage, and evicted either when they
//! exceed an idle timeout or when memory pressure requires reclaiming
//! resources.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use super::types::{Precision, RequestPriority};

/// An entry in the model pool representing a loaded model.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ModelEntry {
    /// The model identifier
    pub model_id: String,
    /// Memory consumed by this model (in MB)
    pub memory_mb: usize,
    /// The precision/quantization level the model was loaded at
    pub precision: Precision,
    /// When the model was last used for inference
    #[serde(skip, default = "Instant::now")]
    pub last_used: Instant,
    /// Priority of the session that originally loaded this model.
    ///
    /// Used by priority-weighted eviction: models loaded for lower-priority
    /// sessions are evicted before those loaded for higher-priority sessions,
    /// all else being equal.
    pub priority: RequestPriority,
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
    /// If the pool is at capacity, the least-eviction-resistant model is removed
    /// first (see [`ModelPool::evict_lru_for_priority`]).
    ///
    /// Returns the model ID of any evicted model, or `None`.
    pub fn load(
        &mut self,
        model_id: &str,
        memory_mb: usize,
        precision: Precision,
        priority: RequestPriority,
    ) -> Option<String> {
        // If already loaded, just refresh the timestamp
        if let Some(entry) = self.loaded.get_mut(model_id) {
            entry.last_used = Instant::now();
            // Upgrade priority if the new request is more important
            if priority > entry.priority {
                entry.priority = priority;
            }
            return None;
        }

        // Evict least-priority LRU model if at capacity
        let evicted = if self.loaded.len() >= self.capacity {
            self.evict_lru_for_priority(priority)
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
                priority,
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

    /// Evict the least-eviction-resistant model, taking priority into account.
    ///
    /// Each loaded model is scored with a combined key `(priority_rank, last_used)`
    /// where `priority_rank` is the *inverse* of the model's load priority (lower
    /// enum discriminant = evict first). Within the same priority bucket, the
    /// least-recently-used model is chosen.
    ///
    /// This ensures that models loaded by `Low`-priority sessions are always
    /// evicted before models loaded by `High`/`Critical`-priority sessions,
    /// even if they were accessed more recently.
    ///
    /// `incoming_priority` is provided for informational purposes; the method
    /// always evicts the *least* resistant model regardless.
    ///
    /// Returns the model ID of the evicted model, or `None` if the pool is empty.
    pub fn evict_lru_for_priority(
        &mut self,
        _incoming_priority: RequestPriority,
    ) -> Option<String> {
        // Score: (priority_as_u8_ascending_from_lowest, last_used)
        // Lower priority_as_u8 → evict first (Low=0, Normal=1, High=2, Critical=3)
        let candidate = self
            .loaded
            .iter()
            .min_by_key(|(_, entry)| {
                let priority_rank = match entry.priority {
                    RequestPriority::Low => 0u8,
                    RequestPriority::Normal => 1,
                    RequestPriority::High => 2,
                    RequestPriority::Critical => 3,
                };
                (priority_rank, entry.last_used)
            })
            .map(|(id, _)| id.clone());

        if let Some(ref id) = candidate {
            self.loaded.remove(id);
        }

        candidate
    }

    /// Evict the least-recently-used model (pure recency, ignores priority).
    ///
    /// Prefer [`ModelPool::evict_lru_for_priority`] when an incoming request
    /// priority is known. This method is kept for backwards compatibility and
    /// internal use in [`ModelPool::evict_until_below`].
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

        pool.load(
            "llama-3-13b",
            13312,
            Precision::F16,
            RequestPriority::Normal,
        );
        assert!(pool.is_loaded("llama-3-13b"));
        assert!(!pool.is_loaded("gpt-4"));
        assert_eq!(pool.len(), 1);
        assert_eq!(pool.total_memory_mb(), 13312);
    }

    #[test]
    fn test_lru_eviction_at_capacity() {
        let mut pool = ModelPool::new(2, Duration::from_secs(300));

        pool.load("model-a", 1000, Precision::F16, RequestPriority::Normal);
        pool.load("model-b", 2000, Precision::F16, RequestPriority::Normal);

        // Pool is full (capacity=2). Loading a third model should evict model-a (oldest).
        let evicted = pool.load("model-c", 3000, Precision::Q8, RequestPriority::Normal);
        assert_eq!(evicted, Some("model-a".to_string()));
        assert!(!pool.is_loaded("model-a"));
        assert!(pool.is_loaded("model-b"));
        assert!(pool.is_loaded("model-c"));
    }

    #[test]
    fn test_touch_refreshes_lru_order() {
        let mut pool = ModelPool::new(2, Duration::from_secs(300));

        pool.load("model-a", 1000, Precision::F16, RequestPriority::Normal);
        std::thread::sleep(Duration::from_millis(10));
        pool.load("model-b", 2000, Precision::F16, RequestPriority::Normal);
        std::thread::sleep(Duration::from_millis(10));

        // Touch model-a to make it recently used
        pool.touch("model-a");

        // Now model-b is the LRU within the same priority bucket
        let evicted = pool.load("model-c", 500, Precision::Q4, RequestPriority::Normal);
        assert_eq!(evicted, Some("model-b".to_string()));
        assert!(pool.is_loaded("model-a"));
        assert!(pool.is_loaded("model-c"));
    }

    #[test]
    fn test_total_memory_tracking() {
        let mut pool = ModelPool::new(10, Duration::from_secs(300));

        pool.load("model-a", 1000, Precision::F16, RequestPriority::Normal);
        pool.load("model-b", 2000, Precision::F16, RequestPriority::Normal);
        pool.load("model-c", 3000, Precision::Q8, RequestPriority::Normal);

        assert_eq!(pool.total_memory_mb(), 6000);

        pool.unload("model-b");
        assert_eq!(pool.total_memory_mb(), 4000);
    }

    #[test]
    fn test_evict_until_below_target() {
        let mut pool = ModelPool::new(10, Duration::from_secs(300));

        pool.load("small", 1000, Precision::Q4, RequestPriority::Normal);
        std::thread::sleep(Duration::from_millis(10));
        pool.load("medium", 4000, Precision::Q8, RequestPriority::Normal);
        std::thread::sleep(Duration::from_millis(10));
        pool.load("large", 8000, Precision::F16, RequestPriority::Normal);

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

        pool.load("model-a", 1000, Precision::F16, RequestPriority::Normal);
        // Loading the same model again should just refresh, not add a duplicate
        let evicted = pool.load("model-a", 1000, Precision::F16, RequestPriority::Normal);
        assert_eq!(evicted, None);
        assert_eq!(pool.len(), 1);
        assert_eq!(pool.total_memory_mb(), 1000);
    }

    #[test]
    fn test_unload_returns_freed_memory() {
        let mut pool = ModelPool::new(3, Duration::from_secs(300));

        pool.load("model-a", 4096, Precision::F16, RequestPriority::Normal);
        let freed = pool.unload("model-a");
        assert_eq!(freed, 4096);

        // Unloading a model that doesn't exist returns 0
        let freed = pool.unload("nonexistent");
        assert_eq!(freed, 0);
    }

    #[test]
    fn test_model_entry_serde_roundtrip() {
        let entry = ModelEntry {
            model_id: "llama-3-13b".into(),
            memory_mb: 13312,
            precision: Precision::F16,
            last_used: Instant::now(),
            priority: RequestPriority::High,
        };
        let json = serde_json::to_string(&entry).unwrap();
        let back: ModelEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(back.model_id, entry.model_id);
        assert_eq!(back.memory_mb, entry.memory_mb);
        assert_eq!(back.precision, entry.precision);
        assert_eq!(back.priority, entry.priority);
    }

    #[test]
    fn test_model_entry_serde_skips_last_used() {
        let entry = ModelEntry {
            model_id: "llama-3".into(),
            memory_mb: 7168,
            precision: Precision::Q8,
            last_used: Instant::now(),
            priority: RequestPriority::Low,
        };
        let json = serde_json::to_string(&entry).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(value.get("last_used").is_none());
    }

    // ── Priority-weighted eviction tests ───────────────────────────────────────

    /// A Low-priority model should be evicted before a High-priority model,
    /// even if the High-priority model has an older last_used timestamp.
    #[test]
    fn test_priority_weighted_eviction_evicts_low_priority_first() {
        let mut pool = ModelPool::new(2, Duration::from_secs(300));

        // Load High-priority model first (it becomes the LRU in time)
        pool.load("high-model", 3000, Precision::F16, RequestPriority::High);
        std::thread::sleep(Duration::from_millis(10));
        // Load Low-priority model second (more recently used)
        pool.load("low-model", 2000, Precision::Q8, RequestPriority::Low);

        // At capacity=2: new load should evict "low-model" despite it being newer,
        // because its priority rank (0) is lower than "high-model" (2).
        let evicted = pool.load("new-model", 1000, Precision::Q4, RequestPriority::Normal);
        assert_eq!(
            evicted,
            Some("low-model".to_string()),
            "Low-priority model should be evicted even if it is more recently used"
        );
        assert!(
            pool.is_loaded("high-model"),
            "High-priority model should survive eviction"
        );
        assert!(pool.is_loaded("new-model"));
    }

    /// Within the same priority bucket, LRU ordering still applies.
    #[test]
    fn test_same_priority_falls_back_to_lru_order() {
        let mut pool = ModelPool::new(2, Duration::from_secs(300));

        pool.load("model-a", 1000, Precision::F16, RequestPriority::Normal);
        std::thread::sleep(Duration::from_millis(10));
        pool.load("model-b", 2000, Precision::F16, RequestPriority::Normal);

        // Both Normal: model-a is the LRU → evicted first
        let evicted = pool.load("model-c", 500, Precision::Q4, RequestPriority::Normal);
        assert_eq!(evicted, Some("model-a".to_string()));
        assert!(pool.is_loaded("model-b"));
        assert!(pool.is_loaded("model-c"));
    }

    /// Reloading an existing model with a higher priority should upgrade its stored priority.
    #[test]
    fn test_reload_upgrades_priority() {
        let mut pool = ModelPool::new(3, Duration::from_secs(300));

        pool.load("model-a", 1000, Precision::F16, RequestPriority::Low);
        assert_eq!(pool.get("model-a").unwrap().priority, RequestPriority::Low);

        // Reload with higher priority — should upgrade, not duplicate
        pool.load("model-a", 1000, Precision::F16, RequestPriority::High);
        let entry = pool.get("model-a").unwrap();
        assert_eq!(
            entry.priority,
            RequestPriority::High,
            "Priority should be upgraded on reload"
        );
        assert_eq!(pool.len(), 1, "No duplicate entry should be created");
    }
}
