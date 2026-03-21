//! Schedule persistence for CronScheduler.
//!
//! This module provides atomic file-based persistence for schedule definitions,
//! ensuring that registered schedules survive process restarts. It uses atomic
//! writes (write-to-tmp + rename) to prevent data corruption during crashes.
//!
//! # Architecture
//!
//! Persistence is opt-in via `CronScheduler::with_persistence(path)`. Once enabled:
//! - `start()` loads and re-registers previously saved schedules
//! - `register()` and `unregister()` automatically persist changes
//! - Atomic writes prevent corruption from partial writes during crashes

use std::path::{Path, PathBuf};

use mofa_kernel::scheduler::{ScheduleDefinition, SchedulerError};
use tokio::fs;

/// Schedule persistence backend with atomic writes.
#[derive(Debug, Clone)]
pub struct SchedulePersistence {
    /// Path to the persistence file (e.g., "schedules.json")
    file_path: PathBuf,
    /// Path to the temporary file used for atomic writes
    tmp_path: PathBuf,
}

impl SchedulePersistence {
    /// Create a new persistence backend for the given file path.
    ///
    /// # Parameters
    ///
    /// - `file_path`: Path where schedule definitions will be saved/loaded
    pub fn new(file_path: impl AsRef<Path>) -> Self {
        let file_path = file_path.as_ref().to_path_buf();
        let tmp_path = file_path.with_extension("tmp");

        Self {
            file_path,
            tmp_path,
        }
    }

    /// Return the canonical path to the persistence file.
    pub fn file_path(&self) -> &Path {
        &self.file_path
    }

    /// Save schedule definitions to disk using atomic writes.
    ///
    /// The write process is:
    /// 1. Serialize definitions to JSON
    /// 2. Write to `{file}.tmp`
    /// 3. Rename `{file}.tmp` → `{file}` (atomic on POSIX)
    ///
    /// This prevents corruption if the process crashes during the write.
    ///
    /// # Errors
    ///
    /// Returns `SchedulerError::PersistenceError` for any IO failures.
    pub async fn save(&self, definitions: &[ScheduleDefinition]) -> Result<(), SchedulerError> {
        let json = serde_json::to_string_pretty(definitions).map_err(|e| {
            SchedulerError::PersistenceError(format!("JSON serialization failed: {}", e))
        })?;

        // Write to temporary file first
        fs::write(&self.tmp_path, &json).await.map_err(|e| {
            SchedulerError::PersistenceError(format!("Failed to write temp file: {}", e))
        })?;

        // Atomic rename (best-effort atomic on Windows, guaranteed on POSIX)
        fs::rename(&self.tmp_path, &self.file_path)
            .await
            .map_err(|e| {
                SchedulerError::PersistenceError(format!("Failed to rename temp file: {}", e))
            })?;

        Ok(())
    }

    /// Load schedule definitions from disk.
    ///
    /// # Returns
    ///
    /// Returns an empty vector if the file doesn't exist (first run).
    /// Returns `SchedulerError::PersistenceError` for IO or JSON parsing failures.
    pub async fn load(&self) -> Result<Vec<ScheduleDefinition>, SchedulerError> {
        match fs::read_to_string(&self.file_path).await {
            Ok(content) => {
                let definitions: Vec<ScheduleDefinition> =
                    serde_json::from_str(&content).map_err(|e| {
                        SchedulerError::PersistenceError(format!("JSON parsing failed: {}", e))
                    })?;
                Ok(definitions)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // File doesn't exist yet (first run) - return empty vec
                Ok(Vec::new())
            }
            Err(e) => Err(SchedulerError::PersistenceError(format!(
                "Failed to read file: {}",
                e
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mofa_kernel::agent::types::AgentInput;
    use mofa_kernel::scheduler::MissedTickPolicy;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_persistence_round_trip() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("schedules.json");
        let persistence = SchedulePersistence::new(&path);

        let defs = vec![
            ScheduleDefinition::new_interval(
                "s1",
                "agent-1",
                5000,
                1,
                AgentInput::text("test"),
                MissedTickPolicy::Skip,
            )
            .unwrap(),
            ScheduleDefinition::new_cron(
                "s2",
                "agent-2",
                "0 * * * * *",
                1,
                AgentInput::text("test"),
                MissedTickPolicy::Burst,
            )
            .unwrap(),
        ];

        // Save and load
        persistence.save(&defs).await.unwrap();
        let loaded = persistence.load().await.unwrap();

        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].schedule_id, "s1");
        assert_eq!(loaded[1].schedule_id, "s2");
        assert_eq!(loaded[1].cron_expression.as_deref(), Some("0 * * * * *"));
    }

    #[tokio::test]
    async fn test_load_nonexistent_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nonexistent.json");
        let persistence = SchedulePersistence::new(&path);

        let loaded = persistence.load().await.unwrap();
        assert!(loaded.is_empty());
    }

    #[tokio::test]
    async fn test_atomic_write_survives_partial() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("schedules.json");
        let tmp_path = path.with_extension("tmp");
        let persistence = SchedulePersistence::new(&path);

        // Save a good file first
        let defs = vec![
            ScheduleDefinition::new_interval(
                "s1",
                "agent-1",
                1000,
                1,
                AgentInput::text("test"),
                MissedTickPolicy::Skip,
            )
            .unwrap(),
        ];
        persistence.save(&defs).await.unwrap();

        // Simulate crash: write corrupt data to .tmp but do NOT rename
        tokio::fs::write(&tmp_path, b"{ this is not valid json ]]]")
            .await
            .unwrap();

        // Original file must still load successfully
        let loaded = persistence.load().await.unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].schedule_id, "s1");
    }
}
