//! Tool schema helpers for consistent JSON schema handling.

use super::types::{LLMError, LLMResult};
use serde_json::{Value, json};

/// Parse a JSON schema from a string.
pub fn parse_schema(schema_json: &str) -> LLMResult<Value> {
    serde_json::from_str(schema_json)
        .map_err(|e| LLMError::SerializationError(format!("Invalid tool schema JSON: {}", e)))
}

/// Validate a tool schema with a minimal, opinionated check.
///
/// This does not implement full JSON Schema validation. It ensures:
/// - The schema is a JSON object.
/// - If `type` is present, it must be `"object"`.
/// - If `properties` is present, it must be an object.
/// - If `required` is present, it must be an array of strings.
pub fn validate_schema(schema: &Value) -> LLMResult<()> {
    let obj = schema
        .as_object()
        .ok_or_else(|| LLMError::ConfigError("Tool schema must be a JSON object".to_string()))?;

    if let Some(schema_type) = obj.get("type") {
        let schema_type = schema_type.as_str().ok_or_else(|| {
            LLMError::ConfigError("Tool schema 'type' must be a string".to_string())
        })?;
        if schema_type != "object" {
            return Err(LLMError::ConfigError(format!(
                "Tool schema 'type' must be \"object\", got \"{}\"",
                schema_type
            )));
        }
    }

    if let Some(properties) = obj.get("properties")
        && !properties.is_object()
    {
        return Err(LLMError::ConfigError(
            "Tool schema 'properties' must be an object".to_string(),
        ));
    }

    if let Some(required) = obj.get("required") {
        let required = required.as_array().ok_or_else(|| {
            LLMError::ConfigError("Tool schema 'required' must be an array".to_string())
        })?;
        for item in required {
            if !item.is_string() {
                return Err(LLMError::ConfigError(
                    "Tool schema 'required' must be an array of strings".to_string(),
                ));
            }
        }
    }

    Ok(())
}

/// Normalize a tool schema into a canonical JSON object.
///
/// - Non-object or null schemas become an empty object schema.
/// - Ensures `type` defaults to `"object"` when missing.
pub fn normalize_schema(schema: Value) -> Value {
    let mut obj = match schema {
        Value::Object(map) => map,
        Value::Null => serde_json::Map::new(),
        _ => serde_json::Map::new(),
    };

    obj.entry("type".to_string())
        .or_insert_with(|| Value::String("object".to_string()));

    obj.entry("properties".to_string())
        .or_insert_with(|| json!({}));

    Value::Object(obj)
}
