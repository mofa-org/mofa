use std::fs;
use std::time::Duration;

use mofa_monitoring::{DashboardConfig, DashboardServer, MetricsConfig};
use serde::Deserialize;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
struct SmithConfig {
    dashboard_port: u16,
    collection_interval_ms: u64,
}

impl Default for SmithConfig {
    fn default() -> Self {
        Self {
            dashboard_port: 8080,
            collection_interval_ms: 1000,
        }
    }
}

impl SmithConfig {
    fn load() -> Self {
        Self::load_from_path("smith.yaml")
    }

    fn load_from_path(path: &str) -> Self {

        match fs::read_to_string(path) {
            Ok(content) => match serde_yaml::from_str::<SmithConfig>(&content) {
                Ok(config) => {
                    info!(
                        path = path,
                        dashboard_port = config.dashboard_port,
                        collection_interval_ms = config.collection_interval_ms,
                        "Loaded smith daemon configuration"
                    );
                    config
                }
                Err(error) => {
                    warn!(
                        path = path,
                        error = %error,
                        "Failed to parse config file, using defaults"
                    );
                    SmithConfig::default()
                }
            },
            Err(error) => {
                warn!(
                    path = path,
                    error = %error,
                    "Config file not found/readable, using defaults"
                );
                SmithConfig::default()
            }
        }
    }
}

struct SmithDaemon {
    config: SmithConfig,
}

impl SmithDaemon {
    fn new(config: SmithConfig) -> Self {
        Self { config }
    }

    async fn start(self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!(
            dashboard_port = self.config.dashboard_port,
            collection_interval_ms = self.config.collection_interval_ms,
            "Starting mofa-smith daemon"
        );

        let metrics_config = MetricsConfig {
            collection_interval: Duration::from_millis(self.config.collection_interval_ms),
            ..MetricsConfig::default()
        };

        let dashboard_config = DashboardConfig::new()
            .with_port(self.config.dashboard_port)
            .with_metrics_config(metrics_config)
            .with_ws_interval(Duration::from_millis(self.config.collection_interval_ms));

        DashboardServer::new(dashboard_config).start().await
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let config = SmithConfig::load();
    let daemon = SmithDaemon::new(config);
    daemon.start().await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn test_smith_config_defaults() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be after unix epoch")
            .as_nanos();
        let missing_path = std::env::temp_dir().join(format!("smith-missing-{unique}.yaml"));

        let config = SmithConfig::load_from_path(&missing_path.to_string_lossy());
        assert_eq!(config.dashboard_port, 8080);
        assert_eq!(config.collection_interval_ms, 1000);
    }

    #[test]
    fn test_smith_config_custom() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be after unix epoch")
            .as_nanos();
        let temp_path: PathBuf = std::env::temp_dir().join(format!("smith-custom-{unique}.yaml"));

        let write_result = fs::write(&temp_path, "dashboard_port: 9090\n");
        assert!(write_result.is_ok(), "failed to create temp smith.yaml");

        let config = SmithConfig::load_from_path(&temp_path.to_string_lossy());
        assert_eq!(config.dashboard_port, 9090);

        let remove_result = fs::remove_file(&temp_path);
        assert!(remove_result.is_ok(), "failed to delete temp smith.yaml");
    }
}
