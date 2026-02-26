//! Cross-crate error conversions for mofa-runtime
//!
//! Implements `From<DomainError> for GlobalError` so that runtime-specific
//! errors from this crate can be converted to the unified `GlobalError`
//! type defined in `mofa-kernel` using the `?` operator.

use mofa_kernel::agent::types::error::GlobalError;

// ============================================================================
// DoraError → GlobalError (only when dora feature is enabled)
// ============================================================================

#[cfg(feature = "dora")]
impl From<crate::dora_adapter::error::DoraError> for GlobalError {
    fn from(err: crate::dora_adapter::error::DoraError) -> Self {
        GlobalError::Dora(err.to_string())
    }
}

// ============================================================================
// Runtime ConfigError → GlobalError
// ============================================================================

impl From<crate::config::ConfigError> for GlobalError {
    fn from(err: crate::config::ConfigError) -> Self {
        GlobalError::Config(err.to_string())
    }
}

// ============================================================================
// AgentConfigError → GlobalError
// ============================================================================

impl From<crate::agent::config::loader::AgentConfigError> for GlobalError {
    fn from(err: crate::agent::config::loader::AgentConfigError) -> Self {
        GlobalError::Config(err.to_string())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ConfigError;
    use crate::agent::config::loader::AgentConfigError;
    use mofa_kernel::agent::types::error::ErrorCategory;

    #[test]
    fn test_runtime_config_error_to_global() {
        let cfg_err = ConfigError::Parse("invalid YAML".to_string());
        let global: GlobalError = cfg_err.into();

        assert_eq!(global.category(), ErrorCategory::Config);
        assert!(global.to_string().contains("invalid YAML"));
    }

    #[test]
    fn test_agent_config_error_to_global() {
        let cfg_err = AgentConfigError::Validation("missing id field".to_string());
        let global: GlobalError = cfg_err.into();

        assert_eq!(global.category(), ErrorCategory::Config);
        assert!(global.to_string().contains("missing id field"));
    }

    #[test]
    fn test_agent_config_parse_error() {
        let err = AgentConfigError::Parse("unexpected token".to_string());
        let global: GlobalError = err.into();
        assert!(global.to_string().contains("unexpected token"));
    }

    #[test]
    fn test_runtime_config_field_missing() {
        let err = ConfigError::FieldMissing("database_url");
        let global: GlobalError = err.into();
        assert!(global.to_string().contains("database_url"));
    }
}
