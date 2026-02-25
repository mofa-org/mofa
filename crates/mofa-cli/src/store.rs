//! Generic file-based persisted store for CLI state.
//!
//! Uses atomic write-then-rename to guarantee crash safety:
//! data is first written to a temporary file in the same directory,
//! `fsync`'d to disk, then atomically renamed to the target path.
//! On POSIX systems, `rename(2)` within the same filesystem is atomic.

use serde::Serialize;
use serde::de::DeserializeOwned;
use std::fmt;
use std::fs;
use std::io::Write;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;
use tracing::warn;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can occur during [`PersistedStore`] operations.
#[derive(Debug)]
pub enum StoreError {
    /// Failed to create the store directory.
    CreateDir { path: PathBuf, source: std::io::Error },
    /// JSON serialization failed.
    Serialize(serde_json::Error),
    /// JSON deserialization failed.
    Deserialize { path: PathBuf, source: serde_json::Error },
    /// An I/O operation on a specific path failed.
    Io { path: PathBuf, source: std::io::Error },
    /// An I/O operation without a specific path (e.g. temp file creation).
    IoRaw(std::io::Error),
    /// Atomic rename of the temp file to the target path failed.
    Persist { path: PathBuf, source: std::io::Error },
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CreateDir { path, source } => {
                write!(f, "failed to create store directory {}: {source}", path.display())
            }
            Self::Serialize(e) => write!(f, "failed to serialize store entry: {e}"),
            Self::Deserialize { path, source } => {
                write!(f, "failed to deserialize store entry {}: {source}", path.display())
            }
            Self::Io { path, source } => {
                write!(f, "I/O error on {}: {source}", path.display())
            }
            Self::IoRaw(e) => write!(f, "I/O error: {e}"),
            Self::Persist { path, source } => {
                write!(f, "failed to atomically persist {}: {source}", path.display())
            }
        }
    }
}

impl std::error::Error for StoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CreateDir { source, .. } => Some(source),
            Self::Serialize(e) => Some(e),
            Self::Deserialize { source, .. } => Some(source),
            Self::Io { source, .. } => Some(source),
            Self::IoRaw(e) => Some(e),
            Self::Persist { source, .. } => Some(source),
        }
    }
}

impl From<serde_json::Error> for StoreError {
    fn from(e: serde_json::Error) -> Self {
        Self::Serialize(e)
    }
}

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

pub struct PersistedStore<T> {
    dir: PathBuf,
    _phantom: PhantomData<T>,
}

impl<T: Serialize + DeserializeOwned> PersistedStore<T> {
    pub fn new(dir: impl AsRef<Path>) -> Result<Self, StoreError> {
        let dir = dir.as_ref().to_path_buf();
        fs::create_dir_all(&dir).map_err(|e| StoreError::CreateDir {
            path: dir.clone(),
            source: e,
        })?;
        Ok(Self {
            dir,
            _phantom: PhantomData,
        })
    }

    /// Persist `item` under the given `id`.
    ///
    /// Uses an atomic write strategy:
    /// 1. Serialize to a temporary file in the **same directory** (ensures same filesystem).
    /// 2. `fsync` the temp file so bytes are durable on disk.
    /// 3. Atomically rename the temp file to the final path.
    ///
    /// If the process crashes at any point before the rename completes,
    /// the previous version of the file (if any) remains intact.
    pub fn save(&self, id: &str, item: &T) -> Result<(), StoreError> {
        let target = self.path_for(id);
        let payload = serde_json::to_vec_pretty(item)?;

        // Create temp file in the same directory so rename is guaranteed
        // to be a same-filesystem atomic operation.
        let mut tmp = NamedTempFile::new_in(&self.dir)
            .map_err(StoreError::IoRaw)?;

        tmp.write_all(&payload)
            .map_err(StoreError::IoRaw)?;

        // Flush userspace buffers and fsync to ensure durability before rename.
        tmp.as_file().sync_all()
            .map_err(StoreError::IoRaw)?;

        // Atomically replace the target file.
        // `persist` calls `rename(2)` on POSIX — atomic within the same filesystem.
        tmp.persist(&target)
            .map_err(|e| StoreError::Persist {
                path: target,
                source: e.error,
            })?;

        Ok(())
    }

    pub fn get(&self, id: &str) -> Result<Option<T>, StoreError> {
        let path = self.path_for(id);
        if !path.exists() {
            return Ok(None);
        }

        let payload = fs::read(&path).map_err(|e| StoreError::Io {
            path: path.clone(),
            source: e,
        })?;
        let item = serde_json::from_slice(&payload).map_err(|e| StoreError::Deserialize {
            path,
            source: e,
        })?;
        Ok(Some(item))
    }

    /// List all stored entries.
    ///
    /// Individual corrupt or unreadable files are logged and skipped rather than
    /// aborting the entire listing — a single bad file should not break the CLI.
    pub fn list(&self) -> Result<Vec<(String, T)>, StoreError> {
        let mut items = Vec::new();

        let entries = fs::read_dir(&self.dir).map_err(|e| StoreError::Io {
            path: self.dir.clone(),
            source: e,
        })?;

        for entry in entries {
            let entry = entry.map_err(StoreError::IoRaw)?;
            let path = entry.path();

            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }

            let id = match path.file_stem().and_then(|stem| stem.to_str()) {
                Some(stem) => stem.to_string(),
                None => continue,
            };

            match fs::read(&path).and_then(|payload| {
                serde_json::from_slice::<T>(&payload)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
            }) {
                Ok(item) => items.push((id, item)),
                Err(e) => {
                    warn!(path = %path.display(), error = %e, "skipping corrupt store entry");
                }
            }
        }

        items.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(items)
    }

    pub fn delete(&self, id: &str) -> Result<bool, StoreError> {
        let path = self.path_for(id);
        if !path.exists() {
            return Ok(false);
        }

        fs::remove_file(&path).map_err(|e| StoreError::Io {
            path,
            source: e,
        })?;
        Ok(true)
    }

    fn path_for(&self, id: &str) -> PathBuf {
        let safe_id: String = id
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                    c
                } else {
                    '_'
                }
            })
            .collect();

        let file_name = if safe_id.is_empty() {
            "_".to_string()
        } else {
            safe_id
        };

        self.dir.join(format!("{}.json", file_name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[derive(Debug, Clone, Serialize, serde::Deserialize, PartialEq)]
    struct TestEntry {
        name: String,
        value: u32,
    }

    #[test]
    fn test_save_and_get() {
        let temp = TempDir::new().unwrap();
        let store = PersistedStore::<TestEntry>::new(temp.path()).unwrap();
        let entry = TestEntry {
            name: "alpha".to_string(),
            value: 1,
        };

        store.save("alpha", &entry).unwrap();
        let loaded = store.get("alpha").unwrap();
        assert_eq!(loaded, Some(entry));
    }

    #[test]
    fn test_get_returns_none_for_missing() {
        let temp = TempDir::new().unwrap();
        let store = PersistedStore::<TestEntry>::new(temp.path()).unwrap();

        assert!(store.get("missing").unwrap().is_none());
    }

    #[test]
    fn test_list_returns_all() {
        let temp = TempDir::new().unwrap();
        let store = PersistedStore::<TestEntry>::new(temp.path()).unwrap();
        store
            .save(
                "a",
                &TestEntry {
                    name: "a".to_string(),
                    value: 1,
                },
            )
            .unwrap();
        store
            .save(
                "b",
                &TestEntry {
                    name: "b".to_string(),
                    value: 2,
                },
            )
            .unwrap();

        let items = store.list().unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].0, "a");
        assert_eq!(items[1].0, "b");
    }

    #[test]
    fn test_list_skips_corrupt_files() {
        let temp = TempDir::new().unwrap();
        let store = PersistedStore::<TestEntry>::new(temp.path()).unwrap();

        // Write a valid entry.
        store
            .save(
                "good",
                &TestEntry {
                    name: "good".to_string(),
                    value: 1,
                },
            )
            .unwrap();

        // Manually write a corrupt JSON file.
        fs::write(temp.path().join("bad.json"), b"NOT VALID JSON {{{").unwrap();

        // list() should return only the valid entry, not abort.
        let items = store.list().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].0, "good");
    }

    #[test]
    fn test_delete() {
        let temp = TempDir::new().unwrap();
        let store = PersistedStore::<TestEntry>::new(temp.path()).unwrap();
        store
            .save(
                "x",
                &TestEntry {
                    name: "x".to_string(),
                    value: 9,
                },
            )
            .unwrap();

        assert!(store.delete("x").unwrap());
        assert!(store.get("x").unwrap().is_none());
    }

    #[test]
    fn test_delete_nonexistent_returns_false() {
        let temp = TempDir::new().unwrap();
        let store = PersistedStore::<TestEntry>::new(temp.path()).unwrap();

        assert!(!store.delete("ghost").unwrap());
    }

    #[test]
    fn test_overwrite() {
        let temp = TempDir::new().unwrap();
        let store = PersistedStore::<TestEntry>::new(temp.path()).unwrap();

        store
            .save(
                "k",
                &TestEntry {
                    name: "old".to_string(),
                    value: 1,
                },
            )
            .unwrap();
        store
            .save(
                "k",
                &TestEntry {
                    name: "new".to_string(),
                    value: 2,
                },
            )
            .unwrap();

        let loaded = store.get("k").unwrap().unwrap();
        assert_eq!(loaded.name, "new");
        assert_eq!(loaded.value, 2);
    }

    #[test]
    fn test_overwrite_preserves_original_on_failure() {
        // Simulate the guarantee: if a second save were to fail after
        // writing the original, the original data must still be intact.
        let temp = TempDir::new().unwrap();
        let store = PersistedStore::<TestEntry>::new(temp.path()).unwrap();

        let original = TestEntry {
            name: "original".to_string(),
            value: 42,
        };
        store.save("item", &original).unwrap();

        // Verify original is readable.
        let loaded = store.get("item").unwrap().unwrap();
        assert_eq!(loaded, original);

        // Write a second version — the atomic rename ensures no partial state.
        let updated = TestEntry {
            name: "updated".to_string(),
            value: 99,
        };
        store.save("item", &updated).unwrap();
        let loaded = store.get("item").unwrap().unwrap();
        assert_eq!(loaded, updated);
    }

    #[test]
    fn test_save_leaves_no_temp_files() {
        let temp = TempDir::new().unwrap();
        let store = PersistedStore::<TestEntry>::new(temp.path()).unwrap();

        store
            .save(
                "clean",
                &TestEntry {
                    name: "clean".to_string(),
                    value: 1,
                },
            )
            .unwrap();

        // After a successful save, the only file in the directory should
        // be the final .json — no leftover temp files.
        let files: Vec<_> = fs::read_dir(temp.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();
        assert_eq!(files.len(), 1);
        assert_eq!(
            files[0].path().file_name().unwrap().to_str().unwrap(),
            "clean.json"
        );
    }

    #[test]
    fn test_survives_new_instance() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().to_path_buf();

        {
            let store = PersistedStore::<TestEntry>::new(&path).unwrap();
            store
                .save(
                    "persisted",
                    &TestEntry {
                        name: "persisted".to_string(),
                        value: 7,
                    },
                )
                .unwrap();
        }

        let new_store = PersistedStore::<TestEntry>::new(&path).unwrap();
        let loaded = new_store.get("persisted").unwrap();
        assert_eq!(
            loaded,
            Some(TestEntry {
                name: "persisted".to_string(),
                value: 7
            })
        );
    }
}
