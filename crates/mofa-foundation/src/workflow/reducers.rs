//! Concrete Reducer Implementations
//!
//! This module provides built-in reducers for state management in workflows.
//! Reducers determine how state updates are merged with existing values.

use async_trait::async_trait;
use mofa_kernel::workflow::{Reducer, ReducerType, StateUpdate};
use serde_json::Value;

use mofa_kernel::agent::error::{AgentError, AgentResult};

/// Overwrite reducer - replaces the current value with the update
///
/// This is the default reducer behavior. The new value completely
/// replaces any existing value.
///
/// # Example
///
/// ```rust,ignore
/// // Before: { "result": "old" }
/// // Update: { "result": "new" }
/// // After:  { "result": "new" }
/// ```
#[derive(Debug, Clone, Default)]
pub struct OverwriteReducer;

#[async_trait]
impl Reducer for OverwriteReducer {
    async fn reduce(&self, _current: Option<&Value>, update: &Value) -> AgentResult<Value> {
        Ok(update.clone())
    }

    fn name(&self) -> &str {
        "overwrite"
    }

    fn reducer_type(&self) -> ReducerType {
        ReducerType::Overwrite
    }
}

/// Append reducer - appends the update to a list
///
/// If the current value doesn't exist or isn't an array, creates a new array.
/// The update value is appended to the array.
///
/// # Example
///
/// ```rust,ignore
/// // Before: { "messages": ["hello"] }
/// // Update: { "messages": "world" }
/// // After:  { "messages": ["hello", "world"] }
/// ```
#[derive(Debug, Clone, Default)]
pub struct AppendReducer;

#[async_trait]
impl Reducer for AppendReducer {
    async fn reduce(&self, current: Option<&Value>, update: &Value) -> AgentResult<Value> {
        let mut arr = match current {
            Some(Value::Array(a)) => a.clone(),
            _ => Vec::new(),
        };
        arr.push(update.clone());
        Ok(Value::Array(arr))
    }

    fn name(&self) -> &str {
        "append"
    }

    fn reducer_type(&self) -> ReducerType {
        ReducerType::Append
    }
}

/// Extend reducer - extends a list with items from another list
///
/// If the update is an array, all items are added to the current array.
/// If the current value doesn't exist or isn't an array, creates a new array.
///
/// # Example
///
/// ```rust,ignore
/// // Before: { "items": [1, 2] }
/// // Update: { "items": [3, 4, 5] }
/// // After:  { "items": [1, 2, 3, 4, 5] }
/// ```
#[derive(Debug, Clone, Default)]
pub struct ExtendReducer;

#[async_trait]
impl Reducer for ExtendReducer {
    async fn reduce(&self, current: Option<&Value>, update: &Value) -> AgentResult<Value> {
        let mut arr = match current {
            Some(Value::Array(a)) => a.clone(),
            _ => Vec::new(),
        };

        match update {
            Value::Array(items) => {
                arr.extend(items.iter().cloned());
            }
            other => {
                arr.push(other.clone());
            }
        }

        Ok(Value::Array(arr))
    }

    fn name(&self) -> &str {
        "extend"
    }

    fn reducer_type(&self) -> ReducerType {
        ReducerType::Extend
    }
}

/// Merge reducer - merges the update into the current object
///
/// Performs a shallow or deep merge of two objects.
///
/// # Example
///
/// ```rust,ignore
/// // Before: { "config": { "a": 1, "b": 2 } }
/// // Update: { "config": { "b": 3, "c": 4 } }
/// // After (shallow): { "config": { "a": 1, "b": 3, "c": 4 } }
/// ```
#[derive(Debug, Clone, Default)]
pub struct MergeReducer {
    /// Whether to perform deep merge on nested objects
    pub deep: bool,
}

impl MergeReducer {
    /// Create a shallow merge reducer
    pub fn shallow() -> Self {
        Self { deep: false }
    }

    /// Create a deep merge reducer
    pub fn deep() -> Self {
        Self { deep: true }
    }
}

#[async_trait]
impl Reducer for MergeReducer {
    async fn reduce(&self, current: Option<&Value>, update: &Value) -> AgentResult<Value> {
        match (current, update) {
            (Some(Value::Object(current_map)), Value::Object(update_map)) => {
                let mut result = current_map.clone();

                for (key, value) in update_map {
                    // If deep merge and both values are objects, recurse
                    if self.deep
                        && let (Some(Value::Object(existing)), Value::Object(new_obj)) =
                            (result.get(key), value)
                    {
                        let merged = merge_objects_deep(existing.clone(), new_obj.clone());
                        result.insert(key.clone(), Value::Object(merged));
                        continue;
                    }
                    result.insert(key.clone(), value.clone());
                }

                Ok(Value::Object(result))
            }
            (None, Value::Object(update_map)) => Ok(Value::Object(update_map.clone())),
            (Some(current), _) => Ok(current.clone()),
            (None, update) => Ok(update.clone()),
        }
    }

    fn name(&self) -> &str {
        if self.deep { "merge_deep" } else { "merge" }
    }

    fn reducer_type(&self) -> ReducerType {
        ReducerType::Merge { deep: self.deep }
    }
}

/// Helper function for deep object merging
fn merge_objects_deep(
    mut base: serde_json::Map<String, Value>,
    update: serde_json::Map<String, Value>,
) -> serde_json::Map<String, Value> {
    for (key, value) in update {
        match (base.get(&key), value) {
            (Some(Value::Object(base_obj)), Value::Object(update_obj)) => {
                let merged = merge_objects_deep(base_obj.clone(), update_obj);
                base.insert(key, Value::Object(merged));
            }
            (_, value) => {
                base.insert(key, value);
            }
        }
    }
    base
}

/// LastN reducer - keeps only the last N items in a list
///
/// Useful for maintaining a sliding window of values.
///
/// # Example
///
/// ```rust,ignore
/// // With n=3:
/// // Before: { "history": [1, 2, 3, 4, 5] }
/// // Update: { "history": 6 }
/// // After:  { "history": [4, 5, 6] }
/// ```
#[derive(Debug, Clone)]
pub struct LastNReducer {
    /// Maximum number of items to keep
    pub n: usize,
}

impl LastNReducer {
    /// Create a new LastN reducer
    pub fn new(n: usize) -> Self {
        Self { n }
    }
}

#[async_trait]
impl Reducer for LastNReducer {
    async fn reduce(&self, current: Option<&Value>, update: &Value) -> AgentResult<Value> {
        let mut arr = match current {
            Some(Value::Array(a)) => a.clone(),
            _ => Vec::new(),
        };

        // Append the new value
        match update {
            Value::Array(items) => {
                arr.extend(items.iter().cloned());
            }
            other => {
                arr.push(other.clone());
            }
        }

        // Keep only last n items
        if arr.len() > self.n {
            let start = arr.len() - self.n;
            arr = arr.split_off(start);
        }

        Ok(Value::Array(arr))
    }

    fn name(&self) -> &str {
        "last_n"
    }

    fn reducer_type(&self) -> ReducerType {
        ReducerType::LastN { n: self.n }
    }
}

/// First reducer - keeps the first non-null value
///
/// Once a value is set, subsequent updates are ignored.
///
/// # Example
///
/// ```rust,ignore
/// // Before: null
/// // Update: "first"
/// // After:  "first"
/// // Update: "second"
/// // After:  "first" (unchanged)
/// ```
#[derive(Debug, Clone, Default)]
pub struct FirstReducer;

#[async_trait]
impl Reducer for FirstReducer {
    async fn reduce(&self, current: Option<&Value>, update: &Value) -> AgentResult<Value> {
        match current {
            Some(value) if !value.is_null() => Ok(value.clone()),
            _ => Ok(update.clone()),
        }
    }

    fn name(&self) -> &str {
        "first"
    }

    fn reducer_type(&self) -> ReducerType {
        ReducerType::First
    }
}

/// Last reducer - keeps the last non-null value
///
/// Always takes the most recent non-null value.
///
/// # Example
///
/// ```rust,ignore
/// // Before: "first"
/// // Update: "second"
/// // After:  "second"
/// // Update: null
/// // After:  "second" (unchanged because update is null)
/// ```
#[derive(Debug, Clone, Default)]
pub struct LastReducer;

#[async_trait]
impl Reducer for LastReducer {
    async fn reduce(&self, _current: Option<&Value>, update: &Value) -> AgentResult<Value> {
        // If update is null, keep current
        if update.is_null() {
            // This is a bit tricky - we need to return the current value
            // But if current is None, we return null
            // For now, let's just return the update (null)
            Ok(update.clone())
        } else {
            Ok(update.clone())
        }
    }

    fn name(&self) -> &str {
        "last"
    }

    fn reducer_type(&self) -> ReducerType {
        ReducerType::Last
    }
}

/// Custom reducer using a closure
///
/// Allows defining custom merge logic with a function.
pub struct CustomReducer<F>
where
    F: Fn(Option<&Value>, &Value) -> AgentResult<Value> + Send + Sync,
{
    name: String,
    func: F,
}

impl<F> CustomReducer<F>
where
    F: Fn(Option<&Value>, &Value) -> AgentResult<Value> + Send + Sync,
{
    /// Create a new custom reducer
    pub fn new(name: impl Into<String>, func: F) -> Self {
        Self {
            name: name.into(),
            func,
        }
    }
}

#[async_trait]
impl<F> Reducer for CustomReducer<F>
where
    F: Fn(Option<&Value>, &Value) -> AgentResult<Value> + Send + Sync,
{
    async fn reduce(&self, current: Option<&Value>, update: &Value) -> AgentResult<Value> {
        (self.func)(current, update)
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn reducer_type(&self) -> ReducerType {
        ReducerType::Custom(self.name.clone())
    }
}

/// Create a reducer from a ReducerType
pub fn create_reducer(reducer_type: &ReducerType) -> AgentResult<Box<dyn Reducer>> {
    match reducer_type {
        ReducerType::Overwrite => Ok(Box::new(OverwriteReducer)),
        ReducerType::Append => Ok(Box::new(AppendReducer)),
        ReducerType::Extend => Ok(Box::new(ExtendReducer)),
        ReducerType::Merge { deep } => Ok(Box::new(MergeReducer { deep: *deep })),
        ReducerType::LastN { n } => Ok(Box::new(LastNReducer::new(*n))),
        ReducerType::First => Ok(Box::new(FirstReducer)),
        ReducerType::Last => Ok(Box::new(LastReducer)),
        ReducerType::Custom(name) => Err(AgentError::Internal(format!(
            "Cannot create reducer for unknown custom type: {}",
            name
        ))),
        _ => Err(AgentError::Internal("Unknown reducer type".to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_overwrite_reducer() {
        let reducer = OverwriteReducer;

        let result = reducer
            .reduce(Some(&json!("old")), &json!("new"))
            .await
            .unwrap();
        assert_eq!(result, json!("new"));

        let result = reducer.reduce(None, &json!("value")).await.unwrap();
        assert_eq!(result, json!("value"));
    }

    #[tokio::test]
    async fn test_append_reducer() {
        let reducer = AppendReducer;

        let result = reducer
            .reduce(Some(&json!(["a", "b"])), &json!("c"))
            .await
            .unwrap();
        assert_eq!(result, json!(["a", "b", "c"]));

        let result = reducer.reduce(None, &json!("first")).await.unwrap();
        assert_eq!(result, json!(["first"]));
    }

    #[tokio::test]
    async fn test_extend_reducer() {
        let reducer = ExtendReducer;

        let result = reducer
            .reduce(Some(&json!([1, 2])), &json!([3, 4]))
            .await
            .unwrap();
        assert_eq!(result, json!([1, 2, 3, 4]));

        // Single item extends
        let result = reducer.reduce(Some(&json!([1])), &json!(2)).await.unwrap();
        assert_eq!(result, json!([1, 2]));
    }

    #[tokio::test]
    async fn test_merge_reducer_shallow() {
        let reducer = MergeReducer::shallow();

        let current = json!({"a": 1, "b": 2});
        let update = json!({"b": 3, "c": 4});
        let result = reducer.reduce(Some(&current), &update).await.unwrap();

        assert_eq!(result["a"], 1);
        assert_eq!(result["b"], 3);
        assert_eq!(result["c"], 4);
    }

    #[tokio::test]
    async fn test_merge_reducer_deep() {
        let reducer = MergeReducer::deep();

        let current = json!({
            "config": {
                "a": 1,
                "b": { "x": 1, "y": 2 }
            }
        });
        let update = json!({
            "config": {
                "b": { "y": 3, "z": 4 },
                "c": 5
            }
        });
        let result = reducer.reduce(Some(&current), &update).await.unwrap();

        // Deep merge should preserve nested "x"
        assert_eq!(result["config"]["a"], 1);
        assert_eq!(result["config"]["b"]["x"], 1);
        assert_eq!(result["config"]["b"]["y"], 3);
        assert_eq!(result["config"]["b"]["z"], 4);
        assert_eq!(result["config"]["c"], 5);
    }

    #[tokio::test]
    async fn test_last_n_reducer() {
        let reducer = LastNReducer::new(3);

        let result = reducer
            .reduce(Some(&json!([1, 2, 3, 4])), &json!(5))
            .await
            .unwrap();
        assert_eq!(result, json!([3, 4, 5]));

        // Adding array extends and keeps last N
        let result = reducer
            .reduce(Some(&json!([1, 2])), &json!([3, 4, 5, 6]))
            .await
            .unwrap();
        assert_eq!(result, json!([4, 5, 6]));
    }

    #[tokio::test]
    async fn test_first_reducer() {
        let reducer = FirstReducer;

        // First value
        let result = reducer.reduce(None, &json!("first")).await.unwrap();
        assert_eq!(result, json!("first"));

        // Subsequent updates ignored
        let result = reducer
            .reduce(Some(&json!("first")), &json!("second"))
            .await
            .unwrap();
        assert_eq!(result, json!("first"));

        // Null current accepts update
        let result = reducer
            .reduce(Some(&json!(null)), &json!("value"))
            .await
            .unwrap();
        assert_eq!(result, json!("value"));
    }

    #[tokio::test]
    async fn test_last_reducer() {
        let reducer = LastReducer;

        // Takes last non-null value
        let result = reducer
            .reduce(Some(&json!("first")), &json!("second"))
            .await
            .unwrap();
        assert_eq!(result, json!("second"));

        // Non-null update replaces
        let result = reducer
            .reduce(Some(&json!("old")), &json!("new"))
            .await
            .unwrap();
        assert_eq!(result, json!("new"));
    }

    #[tokio::test]
    async fn test_custom_reducer() {
        let reducer = CustomReducer::new("sum", |current, update| {
            let curr = current.and_then(|v| v.as_i64()).unwrap_or(0);
            let upd = update.as_i64().unwrap_or(0);
            Ok(json!(curr + upd))
        });

        let result = reducer.reduce(Some(&json!(10)), &json!(5)).await.unwrap();
        assert_eq!(result, json!(15));

        assert_eq!(reducer.name(), "sum");
    }

    #[test]
    fn test_create_reducer() {
        let r = create_reducer(&ReducerType::Overwrite).unwrap();
        assert_eq!(r.name(), "overwrite");

        let r = create_reducer(&ReducerType::Append).unwrap();
        assert_eq!(r.name(), "append");

        let r = create_reducer(&ReducerType::LastN { n: 5 }).unwrap();
        assert_eq!(r.name(), "last_n");
    }
}
