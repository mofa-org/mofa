use std::fs;
use std::path::Path;
use thiserror::Error;

use super::AdversarialCase;

#[derive(Debug, Error)]
pub enum AdversarialLoaderError {
    #[error("Failed to read file: {0}")]
    Io(#[from] std::io::Error),
    #[error("Failed to parse YAML: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("Failed to parse JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Validation failed: {0}")]
    Validation(String),
}

/// Validates loaded cases to ensure required fields aren't completely empty.
fn validate_suite(suite: &[AdversarialCase]) -> Result<(), AdversarialLoaderError> {
    for case in suite {
        if case.id.trim().is_empty() {
            return Err(AdversarialLoaderError::Validation(
                "Case id cannot be empty".to_string(),
            ));
        }
        if case.prompt.trim().is_empty() {
            return Err(AdversarialLoaderError::Validation(format!(
                "Case prompt cannot be empty for id '{}'",
                case.id
            )));
        }
    }
    Ok(())
}

/// Load an adversarial test suite from a YAML file.
pub fn load_suite_from_yaml(path: impl AsRef<Path>) -> Result<Vec<AdversarialCase>, AdversarialLoaderError> {
    let content = fs::read_to_string(path)?;
    let suite: Vec<AdversarialCase> = serde_yaml::from_str(&content)?;
    validate_suite(&suite)?;
    Ok(suite)
}

/// Load an adversarial test suite from a JSON file.
pub fn load_suite_from_json(path: impl AsRef<Path>) -> Result<Vec<AdversarialCase>, AdversarialLoaderError> {
    let content = fs::read_to_string(path)?;
    let suite: Vec<AdversarialCase> = serde_json::from_str(&content)?;
    validate_suite(&suite)?;
    Ok(suite)
}
