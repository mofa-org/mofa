//! Storage traits for key-value and session storage
//!
//! Defines abstract storage interfaces that can be implemented
//! by different storage backends (in-memory, file, database, etc.)

use async_trait::async_trait;
use crate::agent::error::AgentResult;

// ============================================================================
// Generic Storage Trait
// ============================================================================

/// Generic storage trait for key-value operations
///
/// This trait provides a simple interface for storing and retrieving
/// arbitrary data by key. Implementations can use various backends
/// such as in-memory maps, files, databases, etc.
///
/// # Type Parameters
///
/// * `K` - Key type (must be able to be converted to/from String for serialization)
/// * `V` - Value type (must be serializable)
///
/// # Example
///
/// ```rust,ignore
/// use mofa_kernel::storage::Storage;
///
/// struct MyStorage {
///     data: HashMap<String, Vec<u8>>,
/// }
///
/// #[async_trait]
/// impl Storage<String, Vec<u8>> for MyStorage {
///     async fn load(&self, key: &String) -> AgentResult<Option<Vec<u8>>> {
///         Ok(self.data.get(key).cloned())
///     }
///
///     async fn save(&self, key: &String, value: &Vec<u8>) -> AgentResult<()> {
///         self.data.insert(key.clone(), value.clone());
///         Ok(())
///     }
///
///     async fn delete(&self, key: &String) -> AgentResult<bool> {
///         Ok(self.data.remove(key).is_some())
///     }
///
///     async fn list(&self) -> AgentResult<Vec<String>> {
///         Ok(self.data.keys().cloned().collect())
///     }
/// }
/// ```
#[async_trait]
pub trait Storage<K, V>: Send + Sync {
    /// Load a value by key
    ///
    /// Returns `Ok(None)` if the key doesn't exist.
    async fn load(&self, key: &K) -> AgentResult<Option<V>>;

    /// Save a value by key
    ///
    /// Creates a new entry or updates an existing one.
    async fn save(&self, key: &K, value: &V) -> AgentResult<()>;

    /// Delete a value by key
    ///
    /// Returns `Ok(true)` if the key existed and was deleted,
    /// `Ok(false)` if the key didn't exist.
    async fn delete(&self, key: &K) -> AgentResult<bool>;

    /// List all keys
    ///
    /// Returns a vector of all keys in the storage.
    async fn list(&self) -> AgentResult<Vec<K>>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    // Simple in-memory storage for testing
    struct InMemoryStorage {
        data: Arc<RwLock<HashMap<String, Vec<u8>>>>,
    }

    impl InMemoryStorage {
        fn new() -> Self {
            Self {
                data: Arc::new(RwLock::new(HashMap::new())),
            }
        }
    }

    #[async_trait]
    impl Storage<String, Vec<u8>> for InMemoryStorage {
        async fn load(&self, key: &String) -> AgentResult<Option<Vec<u8>>> {
            let data = self.data.read().await;
            Ok(data.get(key).cloned())
        }

        async fn save(&self, key: &String, value: &Vec<u8>) -> AgentResult<()> {
            let mut data = self.data.write().await;
            data.insert(key.clone(), value.clone());
            Ok(())
        }

        async fn delete(&self, key: &String) -> AgentResult<bool> {
            let mut data = self.data.write().await;
            Ok(data.remove(key).is_some())
        }

        async fn list(&self) -> AgentResult<Vec<String>> {
            let data = self.data.read().await;
            Ok(data.keys().cloned().collect())
        }
    }

    #[tokio::test]
    async fn test_storage_basic_operations() {
        let storage = InMemoryStorage::new();

        // Save
        storage.save(&"key1".to_string(), &vec![1, 2, 3]).await.unwrap();

        // Load
        let value = storage.load(&"key1".to_string()).await.unwrap();
        assert_eq!(value, Some(vec![1, 2, 3]));

        // List
        let keys = storage.list().await.unwrap();
        assert_eq!(keys, vec!["key1".to_string()]);

        // Delete
        let deleted = storage.delete(&"key1".to_string()).await.unwrap();
        assert!(deleted);

        // Verify deletion
        let value = storage.load(&"key1".to_string()).await.unwrap();
        assert_eq!(value, None);
    }
}
