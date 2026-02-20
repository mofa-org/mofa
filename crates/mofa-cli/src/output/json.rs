//! JSON output formatting

use serde::Serialize;
use serde_json::{Value, json};

/// Trait for types that can be serialized to JSON output
pub trait JsonOutput: Send + Sync {
    /// Convert to JSON value
    fn to_json(&self) -> Value;
}

impl<T: Serialize + Send + Sync> JsonOutput for T {
    fn to_json(&self) -> Value {
        json!(self)
    }
}

/// JSON output wrapper for structured CLI output
#[derive(Debug, Clone)]
pub struct JsonOutputWrapper {
    pub success: bool,
    pub message: Option<String>,
    pub data: Option<Value>,
    pub error: Option<String>,
}

impl JsonOutputWrapper {
    /// Create a success response
    pub fn success(message: impl Into<String>) -> Self {
        Self {
            success: true,
            message: Some(message.into()),
            data: None,
            error: None,
        }
    }

    /// Create a success response with data
    pub fn success_with_data(data: Value) -> Self {
        Self {
            success: true,
            message: None,
            data: Some(data),
            error: None,
        }
    }

    /// Create a success response with message and data
    pub fn success_with_message_and_data(message: impl Into<String>, data: Value) -> Self {
        Self {
            success: true,
            message: Some(message.into()),
            data: Some(data),
            error: None,
        }
    }

    /// Create an error response
    pub fn error(error: impl Into<String>) -> Self {
        Self {
            success: false,
            message: None,
            data: None,
            error: Some(error.into()),
        }
    }
}

impl JsonOutput for JsonOutputWrapper {
    fn to_json(&self) -> Value {
        let mut obj = serde_json::Map::new();
        obj.insert("success".to_string(), json!(self.success));
        if let Some(msg) = &self.message {
            obj.insert("message".to_string(), json!(msg));
        }
        if let Some(data) = &self.data {
            obj.insert("data".to_string(), data.clone());
        }
        if let Some(err) = &self.error {
            obj.insert("error".to_string(), json!(err));
        }
        Value::Object(obj)
    }
}

/// Convenience type alias for JSON value
pub type JsonValue = Value;
