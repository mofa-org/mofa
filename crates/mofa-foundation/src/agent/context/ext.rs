//! Context Extension Traits
//!
//! Provides a generic extension mechanism for CoreAgentContext
//!
//! # Design Philosophy
//!
//! The context system should be extensible for different use cases:
//! - RichAgentContext: metrics and output tracking
//! - PromptContext: prompt building capabilities
//! - Custom contexts: user-specific extensions
//!
//! # Example
//!
//! ```rust,ignore
//! use mofa_foundation::agent::context::{ContextExt, RichAgentContext};
//! use mofa_kernel::agent::context::CoreAgentContext;
//!
//! let core = CoreAgentContext::new("exec-123");
//! let rich = RichAgentContext::new(core);
//!
//! // Use extension methods
//! rich.record_output("llm", json!("response")).await;
//! ```

use mofa_kernel::agent::context::AgentContext;
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Generic context extension trait
///
/// Allows adding custom data to any context implementation
pub trait ContextExt {
    /// Set extension data
    fn set_extension<T: Send + Sync + serde::Serialize + 'static>(
        &self,
        value: T,
    ) -> impl std::future::Future<Output = ()> + Send;
    /// Get extension data
    fn get_extension<T: Send + Sync + serde::de::DeserializeOwned + 'static>(
        &self,
    ) -> impl std::future::Future<Output = Option<T>> + Send;
    /// Remove extension data
    fn remove_extension<T: Send + Sync + serde::de::DeserializeOwned + 'static>(
        &self,
    ) -> impl std::future::Future<Output = Option<T>> + Send;
    /// Check if extension exists
    fn has_extension<T: Send + Sync + 'static>(
        &self,
    ) -> impl std::future::Future<Output = bool> + Send;
}

/// Extension storage for context
///
/// Stores type-safe extension data
#[derive(Clone, Default)]
pub struct ExtensionStorage {
    inner: Arc<RwLock<HashMap<TypeId, Box<dyn Any + Send + Sync>>>>,
}

impl ExtensionStorage {
    /// Create new storage
    pub fn new() -> Self {
        Self::default()
    }

    /// Set extension value
    pub async fn set<T: Send + Sync + 'static>(&self, value: T) {
        let mut inner = self.inner.write().await;
        inner.insert(TypeId::of::<T>(), Box::new(value));
    }

    /// Get extension value
    pub async fn get<T: Send + Sync + Clone + 'static>(&self) -> Option<T> {
        let inner = self.inner.read().await;
        inner
            .get(&TypeId::of::<T>())
            .and_then(|v| v.downcast_ref::<T>())
            .cloned()
    }

    /// Remove extension value
    pub async fn remove<T: Send + Sync + 'static>(&self) -> Option<T> {
        let mut inner = self.inner.write().await;
        inner
            .remove(&TypeId::of::<T>())
            .and_then(|v| v.downcast::<T>().ok())
            .map(|v| *v)
    }

    /// Check if extension exists
    pub async fn has<T: Send + Sync + 'static>(&self) -> bool {
        let inner = self.inner.read().await;
        inner.contains_key(&TypeId::of::<T>())
    }
}

/// Implement ContextExt for CoreAgentContext using extension storage
impl ContextExt for AgentContext {
    async fn set_extension<T: Send + Sync + serde::Serialize + 'static>(&self, value: T) {
        // Store in the generic K/V store
        let type_name = std::any::type_name::<T>();
        let key = format!("__ext__:{}", type_name);
        if let Ok(v) = serde_json::to_value(&value) {
            self.set(&key, v).await;
        }
    }

    async fn get_extension<T: Send + Sync + serde::de::DeserializeOwned + 'static>(
        &self,
    ) -> Option<T> {
        let type_name = std::any::type_name::<T>();
        let key = format!("__ext__:{}", type_name);
        self.get(&key).await.and_then(|v| {
            match serde_json::from_value(v) {
                Ok(val) => Some(val),
                Err(e) => {
                    tracing::warn!(key = %key, error = %e, "ContextExt::get_extension deserialization failed");
                    None
                }
            }
        })
    }

    async fn remove_extension<T: Send + Sync + serde::de::DeserializeOwned + 'static>(
        &self,
    ) -> Option<T> {
        let type_name = std::any::type_name::<T>();
        let key = format!("__ext__:{}", type_name);
        self.remove(&key)
            .await
            .and_then(|v| {
                match serde_json::from_value(v) {
                    Ok(val) => Some(val),
                    Err(e) => {
                        tracing::warn!(key = %key, error = %e, "ContextExt::remove_extension deserialization failed");
                        None
                    }
                }
            })
    }

    async fn has_extension<T: Send + Sync + 'static>(&self) -> bool {
        let type_name = std::any::type_name::<T>();
        let key = format!("__ext__:{}", type_name);
        self.contains(&key).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
    struct TestExtension {
        value: String,
        count: u32,
    }

    #[tokio::test]
    async fn test_extension_storage() {
        let storage = ExtensionStorage::new();

        storage
            .set(TestExtension {
                value: "test".to_string(),
                count: 42,
            })
            .await;

        assert!(storage.has::<TestExtension>().await);

        let retrieved = storage.get::<TestExtension>().await;
        assert_eq!(
            retrieved,
            Some(TestExtension {
                value: "test".to_string(),
                count: 42,
            })
        );
    }

    #[tokio::test]
    async fn test_context_ext() {
        let ctx = AgentContext::new("test-exec");

        ctx.set_extension(TestExtension {
            value: "test".to_string(),
            count: 42,
        })
        .await;

        assert!(ctx.has_extension::<TestExtension>().await);

        let retrieved = ctx.get_extension::<TestExtension>().await;
        assert_eq!(
            retrieved,
            Some(TestExtension {
                value: "test".to_string(),
                count: 42,
            })
        );

        // Remove and verify
        let removed = ctx.remove_extension::<TestExtension>().await;
        assert_eq!(
            removed,
            Some(TestExtension {
                value: "test".to_string(),
                count: 42,
            })
        );

        assert!(!ctx.has_extension::<TestExtension>().await);
    }
}
