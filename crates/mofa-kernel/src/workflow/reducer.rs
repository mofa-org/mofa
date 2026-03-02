//! Reducer Trait and Types
//!
//! Defines the Reducer pattern for state update strategies.
//! Reducers determine how state updates are merged with existing values.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::agent::error::AgentResult;

/// Reducer trait for state update strategies
///
/// A Reducer defines how to merge a state update with an existing value.
/// Different keys in the state can have different reducers.
///
/// # Example
///
/// ```rust,ignore
/// // Messages should be appended
/// graph.add_reducer("messages", Box::new(AppendReducer));
///
/// // Result should overwrite
/// graph.add_reducer("result", Box::new(OverwriteReducer));
/// ```
#[async_trait]
pub trait Reducer: Send + Sync {
    /// Reduce the current value with the update value
    ///
    /// # Arguments
    /// * `current` - The current value (None if key doesn't exist)
    /// * `update` - The new value to merge
    ///
    /// # Returns
    /// The merged result
    async fn reduce(&self, current: Option<&Value>, update: &Value) -> AgentResult<Value>;

    /// Returns the name of this reducer
    fn name(&self) -> &str;

    /// Returns the type of this reducer
    fn reducer_type(&self) -> ReducerType;
}

/// Built-in reducer types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[non_exhaustive]
pub enum ReducerType {
    /// Overwrite the current value with the update (default)
    #[default]
    Overwrite,

    /// Append the update to a list (creates list if doesn't exist)
    Append,

    /// Extend the current list with items from update list
    Extend,

    /// Merge the update into the current object
    Merge {
        /// Whether to deep merge nested objects
        deep: bool,
    },

    /// Keep only the last N items in a list
    LastN {
        /// Maximum number of items to keep
        n: usize,
    },

    /// Take the first non-null value
    First,

    /// Take the last non-null value
    Last,

    /// Custom reducer with a name identifier
    Custom(String),
}

impl std::fmt::Display for ReducerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReducerType::Overwrite => write!(f, "overwrite"),
            ReducerType::Append => write!(f, "append"),
            ReducerType::Extend => write!(f, "extend"),
            ReducerType::Merge { deep } => write!(f, "merge(deep={})", deep),
            ReducerType::LastN { n } => write!(f, "last_n({})", n),
            ReducerType::First => write!(f, "first"),
            ReducerType::Last => write!(f, "last"),
            ReducerType::Custom(name) => write!(f, "custom({})", name),
        }
    }
}

/// State update operation
///
/// Represents a single key-value update to be applied to the state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateUpdate<V = Value> {
    /// The key to update
    pub key: String,
    /// The new value
    pub value: V,
}

impl<V> StateUpdate<V> {
    /// Create a new state update
    pub fn new(key: impl Into<String>, value: V) -> Self {
        Self {
            key: key.into(),
            value,
        }
    }
}

impl StateUpdate<Value> {
    /// Create a state update from a serializable value
    pub fn from_serializable<T: Serialize>(key: impl Into<String>, value: &T) -> AgentResult<Self> {
        Ok(Self::new(key, serde_json::to_value(value)?))
    }
}

impl<V> From<(String, V)> for StateUpdate<V> {
    fn from((key, value): (String, V)) -> Self {
        Self::new(key, value)
    }
}

impl<V> From<(&str, V)> for StateUpdate<V> {
    fn from((key, value): (&str, V)) -> Self {
        Self::new(key, value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_state_update_creation() {
        let update = StateUpdate::new("key", json!("value"));
        assert_eq!(update.key, "key");
        assert_eq!(update.value, json!("value"));
    }

    #[test]
    fn test_state_update_from_tuple() {
        let update: StateUpdate = ("message", json!("hello")).into();
        assert_eq!(update.key, "message");
        assert_eq!(update.value, json!("hello"));
    }

    #[test]
    fn test_reducer_type_display() {
        assert_eq!(ReducerType::Overwrite.to_string(), "overwrite");
        assert_eq!(
            ReducerType::Merge { deep: true }.to_string(),
            "merge(deep=true)"
        );
        assert_eq!(ReducerType::LastN { n: 5 }.to_string(), "last_n(5)");
    }
}
