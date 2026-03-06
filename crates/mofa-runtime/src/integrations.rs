//! Runtime-level integration helpers for optional external adapters.

#[cfg(feature = "integrations-s3")]
use mofa_integrations::s3::{S3Config, S3ObjectStore};
#[cfg(feature = "integrations-socketio")]
use mofa_integrations::socketio::{SocketIoBridge, SocketIoConfig};
use mofa_kernel::agent::error::{AgentError, AgentResult};
#[cfg(feature = "integrations-socketio")]
use mofa_kernel::bus::AgentBus;
#[cfg(feature = "integrations-socketio")]
use std::sync::Arc;

/// Build an S3-backed [`mofa_kernel::ObjectStore`] adapter.
#[cfg(feature = "integrations-s3")]
pub async fn build_s3_object_store(config: S3Config) -> AgentResult<S3ObjectStore> {
    S3ObjectStore::new(config).await
}

/// Build a Socket.IO bridge for real-time `AgentBus` fanout.
#[cfg(feature = "integrations-socketio")]
pub fn build_socketio_bridge(
    config: SocketIoConfig,
    bus: Arc<AgentBus>,
) -> (socketioxide::layer::SocketIoLayer, axum::Router) {
    let bridge = SocketIoBridge::new(config, bus);
    bridge.build()
}

/// Runtime helper to validate integration feature availability at startup.
pub fn validate_integration_feature(feature_name: &str, enabled: bool) -> AgentResult<()> {
    if enabled {
        Ok(())
    } else {
        Err(AgentError::ConfigError(format!(
            "feature '{feature_name}' is not enabled in this build"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::validate_integration_feature;

    #[test]
    fn validate_integration_feature_reports_disabled() {
        let err = validate_integration_feature("integrations-s3", false)
            .expect_err("disabled feature should return error");
        assert!(err.to_string().contains("integrations-s3"));
    }

    #[test]
    fn validate_integration_feature_accepts_enabled() {
        assert!(validate_integration_feature("integrations-s3", true).is_ok());
    }
}
