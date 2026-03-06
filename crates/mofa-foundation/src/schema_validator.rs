use jsonschema::JSONSchema;
use serde_json::Value;

/// Error types for schema validation.
#[derive(Debug, thiserror::Error)]
pub enum SchemaError {
    #[error("invalid schema JSON: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error("invalid JSON Schema: {0}")]
    InvalidSchema(String),
}

/// Validates JSON responses against a JSON Schema.
pub struct SchemaValidator {
    compiled: JSONSchema,
}

impl SchemaValidator {
    /// Creates a new `SchemaValidator` from a JSON Schema string.
    pub fn new(schema_str: &str) -> Result<Self, SchemaError> {
        let schema: Value = serde_json::from_str(schema_str)?;
        let compiled = JSONSchema::compile(&schema)
            .map_err(|e| SchemaError::InvalidSchema(e.to_string()))?;
        Ok(SchemaValidator { compiled })
    }

    /// Validates a raw JSON string against the schema.
    ///
    /// Returns the parsed `Value` on success, or a string describing all
    /// validation errors on failure.
    pub fn validate(&self, response: &str) -> Result<Value, String> {
        let value: Value = serde_json::from_str(response).map_err(|e| e.to_string())?;
        if let Err(errors) = self.compiled.validate(&value) {
            let messages: Vec<String> = errors.map(|e| e.to_string()).collect();
            return Err(messages.join("; "));
        }
        Ok(value)
    }
}
