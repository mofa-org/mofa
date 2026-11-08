//! Preemptive Memory Telemetry & Proactive Eviction Engine for Inference ModelPool.
//!
//! Provides background system memory monitoring and proactive multi-tiered eviction
//! to prevent OOM kills on edge systems and shared-host runtime environments.

use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::watch;
use tokio::task::JoinHandle;

use super::model_pool::ModelPool;

/// Trait defining a memory telemetry sampler for host RAM / VRAM.
pub trait MemoryTelemetrySampler: Send + Sync {
    /// Returns a tuple of `(total_memory_bytes, available_memory_bytes)`.
    fn sample_memory(&self) -> (u64, u64);
}

/// Default system memory sampler that queries host RAM dynamically via `sysinfo`.
#[derive(Debug, Default, Clone)]
pub struct HostMemorySampler;

impl MemoryTelemetrySampler for HostMemorySampler {
    fn sample_memory(&self) -> (u64, u64) {
        let mut sys = sysinfo::System::new();
        sys.refresh_memory();
        (sys.total_memory(), sys.available_memory())
    }
}

/// Simulated memory telemetry sampler for deterministic unit and integration tests.
#[derive(Debug, Clone)]
pub struct MockMemorySampler {
    total: Arc<Mutex<u64>>,
    available: Arc<Mutex<u64>>,
}

impl MockMemorySampler {
    /// Create a mock sampler with explicit total and available memory in bytes.
    pub fn new(total_bytes: u64, available_bytes: u64) -> Self {
        Self {
            total: Arc::new(Mutex::new(total_bytes)),
            available: Arc::new(Mutex::new(available_bytes)),
        }
    }

    /// Update available memory dynamically to simulate memory spikes or pressure.
    pub fn set_available(&self, available_bytes: u64) {
        *self.available.lock().unwrap() = available_bytes;
    }
}

impl MemoryTelemetrySampler for MockMemorySampler {
    fn sample_memory(&self) -> (u64, u64) {
        (*self.total.lock().unwrap(), *self.available.lock().unwrap())
    }
}

/// Configuration governing preemptive memory telemetry and proactive eviction.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PreemptiveEvictionConfig {
    /// Whether background memory telemetry monitoring is active.
    pub enabled: bool,
    /// Polling interval for sampling host memory.
    #[serde(with = "crate::inference::orchestrator::duration_secs")]
    pub poll_interval: Duration,
    /// Memory pressure threshold (0.0 .. 1.0) above which proactive eviction is triggered.
    /// Default: 0.90 (triggers when 90% of host memory is consumed).
    pub critical_pressure_threshold: f64,
    /// Target memory pressure threshold (0.0 .. 1.0) to achieve after proactive eviction.
    /// Default: 0.75 (evicts until pressure drops to 75% or pool is empty).
    pub target_pressure_threshold: f64,
}

impl Default for PreemptiveEvictionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            poll_interval: Duration::from_secs(5),
            critical_pressure_threshold: 0.90,
            target_pressure_threshold: 0.75,
        }
    }
}

/// Result report from a proactive eviction evaluation cycle.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EvictionReport {
    /// Model IDs evicted due to exceeding idle timeout.
    pub idle_evicted: Vec<String>,
    /// Model IDs evicted due to LRU / priority-weighted pressure reduction.
    pub priority_evicted: Vec<String>,
    /// Host memory pressure ratio (0.0 - 1.0) observed prior to eviction.
    pub initial_pressure: f64,
    /// Host memory pressure ratio (0.0 - 1.0) estimated after eviction.
    pub final_pressure: f64,
    /// Whether proactive eviction was triggered.
    pub triggered: bool,
}

/// Background memory telemetry monitor that runs independently of inference requests.
pub struct PreemptiveMemoryMonitor {
    config: PreemptiveEvictionConfig,
    pool: Arc<Mutex<ModelPool>>,
    sampler: Arc<dyn MemoryTelemetrySampler>,
    shutdown_tx: Option<watch::Sender<bool>>,
    handle: Option<JoinHandle<()>>,
}

impl PreemptiveMemoryMonitor {
    /// Spawns a background telemetry monitor task on the current Tokio runtime.
    pub fn start(
        config: PreemptiveEvictionConfig,
        pool: Arc<Mutex<ModelPool>>,
        sampler: Arc<dyn MemoryTelemetrySampler>,
    ) -> Self {
        let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
        let pool_clone = Arc::clone(&pool);
        let sampler_clone = Arc::clone(&sampler);
        let config_clone = config.clone();

        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(config_clone.poll_interval);
            loop {
                tokio::select! {
                    _ = shutdown_rx.changed() => {
                        if *shutdown_rx.borrow() {
                            break;
                        }
                    }
                    _ = interval.tick() => {
                        let mut locked_pool = pool_clone.lock().unwrap();
                        let _report = Self::check_and_evict(&mut locked_pool, &config_clone, &*sampler_clone);
                    }
                }
            }
        });

        Self {
            config,
            pool,
            sampler,
            shutdown_tx: Some(shutdown_tx),
            handle: Some(handle),
        }
    }

    /// Synchronously performs one check-and-evict cycle.
    pub fn check_and_evict(
        pool: &mut ModelPool,
        config: &PreemptiveEvictionConfig,
        sampler: &dyn MemoryTelemetrySampler,
    ) -> EvictionReport {
        let (total_bytes, available_bytes) = sampler.sample_memory();
        if total_bytes == 0 {
            return EvictionReport::default();
        }

        let used_bytes = total_bytes.saturating_sub(available_bytes);
        let initial_pressure = used_bytes as f64 / total_bytes as f64;

        if initial_pressure < config.critical_pressure_threshold {
            return EvictionReport {
                initial_pressure,
                final_pressure: initial_pressure,
                triggered: false,
                ..Default::default()
            };
        }

        // Host is under critical memory pressure! Proactively reclaim memory.
        let target_used_bytes = (total_bytes as f64 * config.target_pressure_threshold) as u64;
        let bytes_to_reclaim = used_bytes.saturating_sub(target_used_bytes);
        let mb_to_reclaim = (bytes_to_reclaim / 1_000_000) as usize;

        let current_pool_mb = pool.total_memory_mb();
        let target_pool_mb = current_pool_mb.saturating_sub(mb_to_reclaim);

        let (idle_evicted, priority_evicted) = pool.evict_preemptive(target_pool_mb);

        // Recompute projected final pressure
        let current_remaining_pool_mb = pool.total_memory_mb();
        let actual_freed_mb = current_pool_mb.saturating_sub(current_remaining_pool_mb);
        let freed_bytes = (actual_freed_mb as u64) * 1_000_000;
        let final_used_bytes = used_bytes.saturating_sub(freed_bytes);
        let final_pressure = final_used_bytes as f64 / total_bytes as f64;

        EvictionReport {
            idle_evicted,
            priority_evicted,
            initial_pressure,
            final_pressure,
            triggered: true,
        }
    }

    /// Stop the background telemetry task gracefully.
    pub fn stop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(true);
        }
        if let Some(handle) = self.handle.take() {
            handle.abort();
        }
    }

    /// Trigger an immediate manual eviction check cycle.
    pub fn poll_now(&self) -> EvictionReport {
        let mut pool = self.pool.lock().unwrap();
        Self::check_and_evict(&mut pool, &self.config, &*self.sampler)
    }
}

impl Drop for PreemptiveMemoryMonitor {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inference::types::{Precision, RequestPriority};

    #[tokio::test]
    async fn test_preemptive_eviction_triggers_on_high_pressure() {
        let pool = Arc::new(Mutex::new(ModelPool::new(5, Duration::from_millis(100))));
        {
            let mut p = pool.lock().unwrap();
            p.load("idle-1", 2000, Precision::F16, RequestPriority::High);
        }
        tokio::time::sleep(Duration::from_millis(120)).await; // idle-1 expires

        {
            let mut p = pool.lock().unwrap();
            p.load("low-1", 2000, Precision::Q4, RequestPriority::Low);
            p.load("high-1", 2000, Precision::F16, RequestPriority::High);
        }

        // Mock host: 10,000 MB total, 200 MB available (98% pressure > 90% critical threshold)
        // Target is 70% (7,000 MB used). Used is 9,800 MB -> reclaim 2,800 MB.
        // Current pool is 6,000 MB -> target pool is 3,200 MB.
        // Idle eviction frees idle-1 (2,000 MB) -> remaining 4,000 MB.
        // 4,000 MB > 3,200 MB -> priority eviction frees low-1 (2,000 MB) -> remaining 2,000 MB <= 3,200 MB.
        let sampler = MockMemorySampler::new(10_000_000_000, 200_000_000);
        let config = PreemptiveEvictionConfig {
            enabled: true,
            poll_interval: Duration::from_millis(20),
            critical_pressure_threshold: 0.90,
            target_pressure_threshold: 0.70,
        };

        let mut p = pool.lock().unwrap();
        let report = PreemptiveMemoryMonitor::check_and_evict(&mut p, &config, &sampler);

        assert!(report.triggered);
        assert!(report.initial_pressure >= 0.95);
        assert_eq!(report.idle_evicted, vec!["idle-1".to_string()]);
        assert_eq!(report.priority_evicted, vec!["low-1".to_string()]);
        assert!(p.is_loaded("high-1"));
        assert_eq!(p.total_memory_mb(), 2000);
    }

    #[tokio::test]
    async fn test_preemptive_eviction_noop_when_pressure_safe() {
        let pool = Arc::new(Mutex::new(ModelPool::new(5, Duration::from_secs(300))));
        {
            let mut p = pool.lock().unwrap();
            p.load("model-a", 2000, Precision::F16, RequestPriority::Normal);
        }

        // Mock host: 10,000 MB total, 3,000 MB available (70% pressure < 90% critical threshold)
        let sampler = MockMemorySampler::new(10_000_000_000, 3_000_000_000);
        let config = PreemptiveEvictionConfig::default();

        let mut p = pool.lock().unwrap();
        let report = PreemptiveMemoryMonitor::check_and_evict(&mut p, &config, &sampler);

        assert!(!report.triggered);
        assert!(report.idle_evicted.is_empty());
        assert!(report.priority_evicted.is_empty());
        assert_eq!(p.len(), 1);
    }
}

