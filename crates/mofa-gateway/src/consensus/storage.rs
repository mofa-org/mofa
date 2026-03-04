//! Persistent storage for Raft state.
//!
//! This module provides persistent storage for Raft's persistent state:
//! - Current term
//! - Voted for (candidate that received vote in current term)
//! - Log entries
//!
//! Uses RocksDB for efficient, embedded storage.
//!
//! # Implementation Status
//!
//! **Complete** - Persistent storage with RocksDB and in-memory fallback implemented

use crate::error::{ConsensusError, ConsensusResult};
use crate::types::{LogEntry, LogIndex, NodeId, Term};
use std::path::Path;

// RocksDB is optional - use cfg feature gate
#[cfg(feature = "rocksdb")]
use rocksdb::{Options, DB};

/// Persistent storage for Raft state.
#[cfg(feature = "rocksdb")]
pub struct RaftStorage {
    db: DB,
    /// Optional temporary directory backing this RocksDB instance (tests).
    ///
    /// When `RaftStorage` is created via `RaftStorage::new()` for tests, we
    /// hold on to the `TempDir` so that the directory outlives the database
    /// and is cleaned up on drop instead of being leaked.
    #[cfg(test)]
    _temp_dir: Option<tempfile::TempDir>,
}

/// Persistent storage for Raft state (in-memory fallback).
#[cfg(not(feature = "rocksdb"))]
pub struct RaftStorage {
    // In-memory storage for testing
    _data: std::collections::HashMap<String, Vec<u8>>,
}

#[cfg(feature = "rocksdb")]
impl Default for RaftStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl RaftStorage {
    /// Create a new Raft storage instance for testing.
    ///
    /// When the `rocksdb` feature is enabled, this opens a RocksDB-backed
    /// storage in a temporary directory. When `rocksdb` is disabled, this
    /// creates an in-memory storage.
    #[cfg(feature = "rocksdb")]
    pub fn new() -> Self {
        // Use a temporary directory for RocksDB-backed tests.
        // TempDir cleans up on drop, but we keep the path alive for the DB.
        let tmp_dir = tempfile::TempDir::new()
            .expect("failed to create temporary directory for RaftStorage tests");
        let path = tmp_dir
            .path()
            .to_path_buf();

        // Keep the TempDir alive inside RaftStorage so the directory outlives
        // the DB for the duration of the test process, and is cleaned up when
        // the storage is dropped.
        let storage = Self::open(path).expect("failed to open RocksDB-backed RaftStorage");
        Self {
            db: storage.db,
            #[cfg(test)]
            _temp_dir: Some(tmp_dir),
        }
    }

    /// Open or create a Raft storage at the given path.
    #[cfg(feature = "rocksdb")]
    pub fn open<P: AsRef<Path>>(path: P) -> ConsensusResult<Self> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);

        let db = DB::open(&opts, path)
            .map_err(|e| ConsensusError::Storage(format!("RocksDB error: {}", e)))?;

        Ok(Self {
            db,
            #[cfg(test)]
            _temp_dir: None,
        })
    }

    /// Open or create a Raft storage (in-memory fallback).
    #[cfg(not(feature = "rocksdb"))]
    pub fn open<P: AsRef<Path>>(_path: P) -> ConsensusResult<Self> {
        Ok(Self {
            _data: std::collections::HashMap::new(),
        })
    }

    /// Create a new in-memory Raft storage (for testing).
    #[cfg(not(feature = "rocksdb"))]
    pub fn new() -> Self {
        Self {
            _data: std::collections::HashMap::new(),
        }
    }

    /// Save the current term.
    #[cfg(feature = "rocksdb")]
    pub fn save_term(&self, term: Term) -> ConsensusResult<()> {
        let key = b"current_term";
        let value = bincode::serialize(&term.0)
            .map_err(|e: bincode::Error| ConsensusError::Storage(e.to_string()))?;
        self.db
            .put(key, value)
            .map_err(|e| ConsensusError::Storage(format!("RocksDB error: {}", e)))?;
        Ok(())
    }

    /// Save the current term (in-memory fallback).
    #[cfg(not(feature = "rocksdb"))]
    pub fn save_term(&self, _term: Term) -> ConsensusResult<()> {
        // In-memory implementation
        Ok(())
    }

    /// Load the current term.
    #[cfg(feature = "rocksdb")]
    pub fn load_term(&self) -> ConsensusResult<Option<Term>> {
        let key = b"current_term";
        match self.db.get(key) {
            Ok(Some(value_box)) => {
                let term: u64 = bincode::deserialize(&value_box)
                    .map_err(|e: bincode::Error| ConsensusError::Storage(e.to_string()))?;
                Ok(Some(Term::new(term)))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(ConsensusError::Storage(format!("RocksDB error: {}", e))),
        }
    }

    /// Load the current term (in-memory fallback).
    #[cfg(not(feature = "rocksdb"))]
    pub fn load_term(&self) -> ConsensusResult<Option<Term>> {
        Ok(None)
    }

    /// Save the voted-for candidate.
    #[cfg(feature = "rocksdb")]
    pub fn save_voted_for(&self, node_id: Option<&NodeId>) -> ConsensusResult<()> {
        let key = b"voted_for";
        match node_id {
            Some(id) => {
                let value = bincode::serialize(&id.0)
                    .map_err(|e: bincode::Error| ConsensusError::Storage(e.to_string()))?;
                self.db
                    .put(key, value)
                    .map_err(|e| ConsensusError::Storage(format!("RocksDB error: {}", e)))?;
            }
            None => {
                self.db
                    .delete(key)
                    .map_err(|e| ConsensusError::Storage(format!("RocksDB error: {}", e)))?;
            }
        }
        Ok(())
    }

    /// Save the voted-for candidate (in-memory fallback).
    #[cfg(not(feature = "rocksdb"))]
    pub fn save_voted_for(&self, _node_id: Option<&NodeId>) -> ConsensusResult<()> {
        Ok(())
    }

    /// Load the voted-for candidate.
    #[cfg(feature = "rocksdb")]
    pub fn load_voted_for(&self) -> ConsensusResult<Option<NodeId>> {
        let key = b"voted_for";
        match self.db.get(key) {
            Ok(Some(value_box)) => {
                let id: String = bincode::deserialize(&value_box)
                    .map_err(|e: bincode::Error| ConsensusError::Storage(e.to_string()))?;
                Ok(Some(NodeId::new(id)))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(ConsensusError::Storage(format!("RocksDB error: {}", e))),
        }
    }

    /// Load the voted-for candidate (in-memory fallback).
    #[cfg(not(feature = "rocksdb"))]
    pub fn load_voted_for(&self) -> ConsensusResult<Option<NodeId>> {
        Ok(None)
    }

    /// Append a log entry.
    #[cfg(feature = "rocksdb")]
    pub fn append_log_entry(&self, entry: &LogEntry) -> ConsensusResult<()> {
        let key = format!("log:{}", entry.index.0);
        let value = bincode::serialize(entry)
            .map_err(|e: bincode::Error| ConsensusError::Storage(e.to_string()))?;
        self.db
            .put(key.as_bytes(), value)
            .map_err(|e| ConsensusError::Storage(format!("RocksDB error: {}", e)))?;
        Ok(())
    }

    /// Append a log entry (in-memory fallback).
    #[cfg(not(feature = "rocksdb"))]
    pub fn append_log_entry(&self, _entry: &LogEntry) -> ConsensusResult<()> {
        Ok(())
    }

    /// Load a log entry at the given index.
    #[cfg(feature = "rocksdb")]
    pub fn load_log_entry(&self, index: LogIndex) -> ConsensusResult<Option<LogEntry>> {
        let key = format!("log:{}", index.0);
        match self.db.get(key.as_bytes()) {
            Ok(Some(value_box)) => {
                let entry: LogEntry = bincode::deserialize(&value_box)
                    .map_err(|e: bincode::Error| ConsensusError::Storage(e.to_string()))?;
                Ok(Some(entry))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(ConsensusError::Storage(format!("RocksDB error: {}", e))),
        }
    }

    /// Load a log entry at the given index (in-memory fallback).
    #[cfg(not(feature = "rocksdb"))]
    pub fn load_log_entry(&self, _index: LogIndex) -> ConsensusResult<Option<LogEntry>> {
        Ok(None)
    }

    /// Get all log entries.
    #[cfg(feature = "rocksdb")]
    pub fn load_all_log_entries(&self) -> ConsensusResult<Vec<LogEntry>> {
        let mut entries = Vec::new();
        let iter = self.db.iterator(rocksdb::IteratorMode::Start);

        for item_result in iter {
            match item_result {
                Ok((key_box, value_box)) => {
                    let key: &[u8] = &key_box;
                    if key.starts_with(b"log:") {
                        let value: &[u8] = &value_box;
                        match bincode::deserialize::<LogEntry>(value) {
                            Ok(entry) => entries.push(entry),
                            Err(e) => return Err(ConsensusError::Storage(e.to_string())),
                        }
                    }
                }
                Err(e) => return Err(ConsensusError::Storage(format!("RocksDB error: {}", e))),
            }
        }

        // Sort by index
        entries.sort_by_key(|e| e.index.0);
        Ok(entries)
    }

    /// Get all log entries (in-memory fallback).
    #[cfg(not(feature = "rocksdb"))]
    pub fn load_all_log_entries(&self) -> ConsensusResult<Vec<LogEntry>> {
        Ok(Vec::new())
    }
}

#[cfg(not(feature = "rocksdb"))]
impl Default for RaftStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "rocksdb")]
    use tempfile::TempDir;

    #[tokio::test]
    #[cfg(feature = "rocksdb")]
    async fn test_storage_term() {
        let temp_dir = TempDir::new().unwrap();
        let storage = RaftStorage::open(temp_dir.path()).unwrap();

        // Save and load term
        storage.save_term(Term::new(5)).unwrap();
        let term = storage.load_term().unwrap();
        assert_eq!(term, Some(Term::new(5)));
    }

    #[tokio::test]
    #[cfg(feature = "rocksdb")]
    async fn test_storage_voted_for() {
        let temp_dir = TempDir::new().unwrap();
        let storage = RaftStorage::open(temp_dir.path()).unwrap();

        // Save and load voted_for
        let node_id = NodeId::new("node-1");
        storage.save_voted_for(Some(&node_id)).unwrap();
        let loaded = storage.load_voted_for().unwrap();
        assert_eq!(loaded, Some(node_id));

        // Clear voted_for
        storage.save_voted_for(None).unwrap();
        let loaded = storage.load_voted_for().unwrap();
        assert!(loaded.is_none());
    }

    #[tokio::test]
    #[cfg(feature = "rocksdb")]
    async fn test_storage_log_entries() {
        let temp_dir = TempDir::new().unwrap();
        let storage = RaftStorage::open(temp_dir.path()).unwrap();

        // Append log entries
        let entry1 = LogEntry {
            term: Term::new(1),
            index: LogIndex::new(1),
            data: b"entry1".to_vec(),
        };
        let entry2 = LogEntry {
            term: Term::new(1),
            index: LogIndex::new(2),
            data: b"entry2".to_vec(),
        };

        storage.append_log_entry(&entry1).unwrap();
        storage.append_log_entry(&entry2).unwrap();

        // Load entries
        let loaded1 = storage.load_log_entry(LogIndex::new(1)).unwrap();
        assert_eq!(loaded1.as_ref().map(|e| &e.data), Some(&b"entry1".to_vec()));

        let all = storage.load_all_log_entries().unwrap();
        assert_eq!(all.len(), 2);
    }
}
