//! Cross-crate error conversions for mofa-plugins
//!
//! Implements `From<DomainError> for GlobalError` so that plugin-specific
//! errors from this crate can be converted to the unified `GlobalError`
//! type defined in `mofa-kernel` using the `?` operator.

use mofa_kernel::agent::types::error::GlobalError;

// ============================================================================
// WasmError → GlobalError
// ============================================================================

impl From<crate::wasm_runtime::WasmError> for GlobalError {
    fn from(err: crate::wasm_runtime::WasmError) -> Self {
        GlobalError::Wasm(err.to_string())
    }
}

// ============================================================================
// RhaiPluginError → GlobalError
// ============================================================================

impl From<crate::rhai_runtime::RhaiPluginError> for GlobalError {
    fn from(err: crate::rhai_runtime::RhaiPluginError) -> Self {
        GlobalError::Rhai(err.to_string())
    }
}

// ============================================================================
// PluginLoadError → GlobalError
// ============================================================================

impl From<crate::hot_reload::PluginLoadError> for GlobalError {
    fn from(err: crate::hot_reload::PluginLoadError) -> Self {
        GlobalError::Plugin(err.to_string())
    }
}

// ============================================================================
// ReloadError → GlobalError
// ============================================================================

impl From<crate::hot_reload::ReloadError> for GlobalError {
    fn from(err: crate::hot_reload::ReloadError) -> Self {
        GlobalError::Plugin(err.to_string())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wasm_runtime::WasmError;
    use crate::rhai_runtime::RhaiPluginError;
    use mofa_kernel::agent::types::error::ErrorCategory;

    #[test]
    fn test_wasm_error_to_global() {
        let wasm_err = WasmError::CompilationError("invalid module".to_string());
        let global: GlobalError = wasm_err.into();

        assert_eq!(global.category(), ErrorCategory::Plugin);
        assert!(global.to_string().contains("invalid module"));
    }

    #[test]
    fn test_wasm_error_variants() {
        let timeout = WasmError::Timeout(5000);
        let global: GlobalError = timeout.into();
        assert!(global.is_retryable());
        assert!(global.to_string().contains("5000"));

        let not_found = WasmError::PluginNotFound("my-plugin".to_string());
        let global: GlobalError = not_found.into();
        assert!(global.to_string().contains("my-plugin"));

        let exec_err = WasmError::ExecutionError("stack overflow".to_string());
        let global: GlobalError = exec_err.into();
        assert!(global.to_string().contains("stack overflow"));
    }

    #[test]
    fn test_rhai_error_to_global() {
        let rhai_err = RhaiPluginError::CompilationError("syntax error".to_string());
        let global: GlobalError = rhai_err.into();

        assert_eq!(global.category(), ErrorCategory::Plugin);
        assert!(global.to_string().contains("syntax error"));
    }

    #[test]
    fn test_rhai_error_variants() {
        let exec_err = RhaiPluginError::ExecutionError("division by zero".to_string());
        let global: GlobalError = exec_err.into();
        assert!(global.to_string().contains("division by zero"));

        let missing = RhaiPluginError::MissingFunction("on_init".to_string());
        let global: GlobalError = missing.into();
        assert!(global.to_string().contains("on_init"));
    }
}
