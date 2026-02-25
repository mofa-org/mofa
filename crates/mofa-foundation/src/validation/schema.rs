//! Schema-based validation for requests and responses
//!
//! This module provides schema validation using JSON Schema draft-07.

use crate::validation::error::{
    ValidationError, ValidationErrorCollection, ValidationRule, ValidationRuleType,
};
use regex::Regex;
use serde_json::{Map, Value};

/// Schema validator for validating JSON data against rules
pub struct SchemaValidator {
    /// Compiled regex patterns for pattern validation
    patterns: std::collections::HashMap<String, Regex>,
}

impl SchemaValidator {
    /// Create a new schema validator
    pub fn new() -> Self {
        Self {
            patterns: std::collections::HashMap::new(),
        }
    }

    /// Validate a JSON value against a list of rules
    pub fn validate(&self, data: &Value, rules: &[ValidationRule]) -> ValidationErrorCollection {
        let mut errors = Vec::new();

        for rule in rules {
            if let Some(error) = self.validate_rule(data, rule) {
                errors.push(error);
            }
        }

        ValidationErrorCollection::with_errors(errors)
    }

    /// Validate a specific field path in the JSON data
    pub fn validate_field(&self, data: &Value, field_path: &str, rules: &[ValidationRule]) -> ValidationErrorCollection {
        let mut errors = Vec::new();

        // Get the value at the field path
        let field_value = self.get_field_value(data, field_path);

        for rule in rules {
            if rule.field != field_path {
                continue;
            }

            if let Some(error) = self.validate_value(field_value, field_path, rule) {
                errors.push(error);
            }
        }

        ValidationErrorCollection::with_errors(errors)
    }

    /// Validate a value against a single rule
    fn validate_rule(&self, data: &Value, rule: &ValidationRule) -> Option<ValidationError> {
        let field_value = self.get_field_value(data, &rule.field);
        self.validate_value(field_value, &rule.field, rule)
    }

    /// Validate a specific value against a rule
    fn validate_value(
        &self,
        value: Option<&Value>,
        field_path: &str,
        rule: &ValidationRule,
    ) -> Option<ValidationError> {
        // Handle optional fields
        if rule.optional {
            if value.is_none() || value == Some(&Value::Null) {
                return None;
            }
        }

        let value = match value {
            Some(v) => v,
            None => {
                // Field is missing - check if it's required
                if matches!(rule.rule_type, ValidationRuleType::Required) {
                    return Some(ValidationError::new(
                        field_path,
                        format!("Field '{}' is required", field_path),
                        "REQUIRED_FIELD_MISSING",
                    ));
                }
                return None;
            }
        };

        match &rule.rule_type {
            ValidationRuleType::Required => {
                if value.is_null() {
                    Some(ValidationError::new(
                        field_path,
                        format!("Field '{}' cannot be null", field_path),
                        "NULL_VALUE",
                    ))
                } else {
                    None
                }
            }
            ValidationRuleType::MinLength(min) => {
                if let Some(s) = value.as_str() {
                    if s.len() < *min {
                        return Some(ValidationError::with_value(
                            field_path,
                            format!("Field '{}' must be at least {} characters", field_path, min),
                            "MIN_LENGTH",
                            Value::Number((*min).into()),
                        ));
                    }
                } else if let Some(arr) = value.as_array() {
                    if arr.len() < *min {
                        return Some(ValidationError::with_value(
                            field_path,
                            format!("Field '{}' must have at least {} items", field_path, min),
                            "MIN_LENGTH",
                            Value::Number((*min).into()),
                        ));
                    }
                }
                None
            }
            ValidationRuleType::MaxLength(max) => {
                if let Some(s) = value.as_str() {
                    if s.len() > *max {
                        return Some(ValidationError::with_value(
                            field_path,
                            format!("Field '{}' must be at most {} characters", field_path, max),
                            "MAX_LENGTH",
                            Value::Number((*max).into()),
                        ));
                    }
                } else if let Some(arr) = value.as_array() {
                    if arr.len() > *max {
                        return Some(ValidationError::with_value(
                            field_path,
                            format!("Field '{}' must have at most {} items", field_path, max),
                            "MAX_LENGTH",
                            Value::Number((*max).into()),
                        ));
                    }
                }
                None
            }
            ValidationRuleType::MinValue(min) => {
                if let Some(n) = value.as_f64() {
                    if n < *min {
                        return Some(ValidationError::with_value(
                            field_path,
                            format!("Field '{}' must be at least {}", field_path, min),
                            "MIN_VALUE",
                            Value::Number(serde_json::Number::from_f64(*min).unwrap_or(serde_json::Number::from(0))),
                        ));
                    }
                }
                None
            }
            ValidationRuleType::MaxValue(max) => {
                if let Some(n) = value.as_f64() {
                    if n > *max {
                        return Some(ValidationError::with_value(
                            field_path,
                            format!("Field '{}' must be at most {}", field_path, max),
                            "MAX_VALUE",
                            Value::Number(serde_json::Number::from_f64(*max).unwrap_or(serde_json::Number::from(0))),
                        ));
                    }
                }
                None
            }
            ValidationRuleType::Pattern(pattern) => {
                if let Some(s) = value.as_str() {
                    let regex = self.patterns.get(pattern).cloned().or_else(|| {
                        Regex::new(pattern).ok()
                    });

                    if let Some(regex) = regex {
                        if !regex.is_match(s) {
                            return Some(ValidationError::new(
                                field_path,
                                format!("Field '{}' does not match pattern '{}'", field_path, pattern),
                                "PATTERN_MISMATCH",
                            ));
                        }
                    }
                }
                None
            }
            ValidationRuleType::AllowedValues(allowed) => {
                if !allowed.contains(value) {
                    return Some(ValidationError::new(
                        field_path,
                        format!("Field '{}' must be one of: {:?}", field_path, allowed),
                        "INVALID_VALUE",
                    ));
                }
                None
            }
            ValidationRuleType::Email => {
                if let Some(s) = value.as_str() {
                    let email_regex = Regex::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$").ok();
                    if let Some(regex) = email_regex {
                        if !regex.is_match(s) {
                            return Some(ValidationError::new(
                                field_path,
                                format!("Field '{}' must be a valid email address", field_path),
                                "INVALID_EMAIL",
                            ));
                        }
                    }
                }
                None
            }
            ValidationRuleType::Url => {
                if let Some(s) = value.as_str() {
                    if !s.starts_with("http://") && !s.starts_with("https://") {
                        return Some(ValidationError::new(
                            field_path,
                            format!("Field '{}' must be a valid URL", field_path),
                            "INVALID_URL",
                        ));
                    }
                    if url::Url::parse(s).is_err() {
                        return Some(ValidationError::new(
                            field_path,
                            format!("Field '{}' must be a valid URL", field_path),
                            "INVALID_URL",
                        ));
                    }
                }
                None
            }
            ValidationRuleType::Uuid => {
                if let Some(s) = value.as_str() {
                    let uuid_regex = Regex::new(r"^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$").ok();
                    if let Some(regex) = uuid_regex {
                        if !regex.is_match(s) {
                            return Some(ValidationError::new(
                                field_path,
                                format!("Field '{}' must be a valid UUID", field_path),
                                "INVALID_UUID",
                            ));
                        }
                    }
                }
                None
            }
            ValidationRuleType::Schema(schema) => {
                self.validate_json_schema(value, schema, field_path)
            }
            ValidationRuleType::Custom(_) => {
                // Custom validation would be handled at runtime
                // For now, we skip it
                None
            }
        }
    }

    /// Validate a value against a JSON schema
    fn validate_json_schema(
        &self,
        value: &Value,
        schema: &Value,
        field_path: &str,
    ) -> Option<ValidationError> {
        // Basic JSON Schema validation (draft-07 subset)
        if let Some(obj) = schema.as_object() {
            // Check type
            if let Some(type_val) = obj.get("type") {
                if let Some(type_str) = type_val.as_str() {
                    let valid = match type_str {
                        "string" => value.is_string(),
                        "number" | "integer" => value.is_number(),
                        "boolean" => value.is_boolean(),
                        "array" => value.is_array(),
                        "object" => value.is_object(),
                        "null" => value.is_null(),
                        _ => true, // Unknown type, skip
                    };
                    if !valid {
                        return Some(ValidationError::new(
                            field_path,
                            format!("Field '{}' must be of type '{}'", field_path, type_str),
                            "INVALID_TYPE",
                        ));
                    }
                }
            }

            // Check enum
            if let Some(enum_values) = obj.get("enum") {
                if let Some(arr) = enum_values.as_array() {
                    if !arr.contains(value) {
                        return Some(ValidationError::new(
                            field_path,
                            format!("Field '{}' must be one of the allowed values", field_path),
                            "ENUM_VIOLATION",
                        ));
                    }
                }
            }

            // Check minimum for numbers
            if let Some(min) = obj.get("minimum") {
                if let (Some(num), Some(min_val)) = (value.as_f64(), min.as_f64()) {
                    if num < min_val {
                        return Some(ValidationError::new(
                            field_path,
                            format!("Field '{}' must be at least {}", field_path, min_val),
                            "MINIMUM_VIOLATION",
                        ));
                    }
                }
            }

            // Check maximum for numbers
            if let Some(max) = obj.get("maximum") {
                if let (Some(num), Some(max_val)) = (value.as_f64(), max.as_f64()) {
                    if num > max_val {
                        return Some(ValidationError::new(
                            field_path,
                            format!("Field '{}' must be at most {}", field_path, max_val),
                            "MAXIMUM_VIOLATION",
                        ));
                    }
                }
            }

            // Check minLength for strings
            if let Some(min_len) = obj.get("minLength") {
                if let (Some(s), Some(len)) = (value.as_str(), min_len.as_u64()) {
                    if s.len() < len as usize {
                        return Some(ValidationError::new(
                            field_path,
                            format!("Field '{}' must be at least {} characters", field_path, len),
                            "MIN_LENGTH_VIOLATION",
                        ));
                    }
                }
            }

            // Check maxLength for strings
            if let Some(max_len) = obj.get("maxLength") {
                if let (Some(s), Some(len)) = (value.as_str(), max_len.as_u64()) {
                    if s.len() > len as usize {
                        return Some(ValidationError::new(
                            field_path,
                            format!("Field '{}' must be at most {} characters", field_path, len),
                            "MAX_LENGTH_VIOLATION",
                        ));
                    }
                }
            }

            // Check pattern for strings
            if let Some(pattern) = obj.get("pattern") {
                if let (Some(s), Some(pattern_str)) = (value.as_str(), pattern.as_str()) {
                    if let Ok(regex) = Regex::new(pattern_str) {
                        if !regex.is_match(s) {
                            return Some(ValidationError::new(
                                field_path,
                                format!("Field '{}' does not match pattern '{}'", field_path, pattern_str),
                                "PATTERN_VIOLATION",
                            ));
                        }
                    }
                }
            }
        }

        None
    }

    /// Get a value from a JSON object by field path (supports nested paths and array indexing)
    fn get_field_value<'a>(&self, data: &'a Value, field_path: &str) -> Option<&'a Value> {
        let parts: Vec<&str> = field_path.split('.').collect();
        let mut current: Option<&Value> = Some(data);

        for part in parts {
            if let Some(val) = current {
                // Handle array indexing (e.g., "messages[0].content")
                if let Some(bracket_pos) = part.find('[') {
                    let field = &part[..bracket_pos];
                    let index_part = &part[bracket_pos..];

                    current = if field.is_empty() {
                        val.as_array()?.get(0)
                    } else {
                        val.get(field)
                    };

                    // Parse array index
                    if let Some(start) = index_part.find('[') {
                        let inner = &index_part[start + 1..index_part.len() - 1];
                        if let Ok(idx) = inner.parse::<usize>() {
                            current = current.and_then(|v| v.as_array()?.get(idx));
                        }
                    }
                } else {
                    current = val.get(part);
                }
            } else {
                return None;
            }
        }

        current
    }

    /// Validate request body JSON
    pub fn validate_request(&self, body: &Value, rules: &[ValidationRule]) -> ValidationErrorCollection {
        self.validate(body, rules)
    }

    /// Validate response JSON
    pub fn validate_response(&self, response: &Value, rules: &[ValidationRule]) -> ValidationErrorCollection {
        self.validate(response, rules)
    }
}

impl Default for SchemaValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_required_field() {
        let validator = SchemaValidator::new();
        let data = json!({});
        let rules = vec![ValidationRule {
            name: "required_name".to_string(),
            field: "name".to_string(),
            rule_type: ValidationRuleType::Required,
            custom_message: None,
            optional: false,
        }];

        let result = validator.validate(&data, &rules);
        assert!(!result.is_valid());
        assert_eq!(result.errors.len(), 1);
    }

    #[test]
    fn test_min_max_length() {
        let validator = SchemaValidator::new();
        let data = json!({ "name": "ab" });
        let rules = vec![ValidationRule {
            name: "name_length".to_string(),
            field: "name".to_string(),
            rule_type: ValidationRuleType::MinLength(3),
            custom_message: None,
            optional: false,
        }];

        let result = validator.validate(&data, &rules);
        assert!(!result.is_valid());
    }

    #[test]
    fn test_valid_data() {
        let validator = SchemaValidator::new();
        let data = json!({ "name": "John", "age": 30 });
        let rules = vec![
            ValidationRule {
                name: "name_required".to_string(),
                field: "name".to_string(),
                rule_type: ValidationRuleType::Required,
                custom_message: None,
                optional: false,
            },
            ValidationRule {
                name: "age_range".to_string(),
                field: "age".to_string(),
                rule_type: ValidationRuleType::MinValue(0.0),
                custom_message: None,
                optional: false,
            },
        ];

        let result = validator.validate(&data, &rules);
        assert!(result.is_valid());
    }

    #[test]
    fn test_nested_field() {
        let validator = SchemaValidator::new();
        let data = json!({ "user": { "name": "John" } });
        let rules = vec![ValidationRule {
            name: "user_name".to_string(),
            field: "user.name".to_string(),
            rule_type: ValidationRuleType::Required,
            custom_message: None,
            optional: false,
        }];

        let result = validator.validate(&data, &rules);
        assert!(result.is_valid());
    }
}
