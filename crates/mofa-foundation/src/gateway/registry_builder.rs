use std::sync::Arc;

use crate::gateway::{
    DuckDuckGoSearchCapability, GatewayCapabilityRegistry, HttpFetchCapability,
    ReadSensorCapability, WebhookNotificationCapability,
};

/// Environment/config-backed settings for registering built-in capabilities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatewayCapabilityRegistryConfig {
    /// Base URL for DuckDuckGo-compatible search.
    pub web_search_url: Option<String>,
    /// Whether generic HTTP fetch should be registered.
    pub enable_http_fetch: bool,
    /// Webhook target for notification delivery.
    pub notification_webhook_url: Option<String>,
    /// Sensor endpoint for simple HTTP-based device reads.
    pub sensor_url: Option<String>,
}

impl Default for GatewayCapabilityRegistryConfig {
    fn default() -> Self {
        Self {
            web_search_url: Some("https://api.duckduckgo.com/".to_string()),
            enable_http_fetch: true,
            notification_webhook_url: None,
            sensor_url: None,
        }
    }
}

impl GatewayCapabilityRegistryConfig {
    /// Load built-in capability settings from environment variables.
    pub fn from_env() -> Self {
        Self {
            web_search_url: std::env::var("GATEWAY_WEB_SEARCH_URL")
                .ok()
                .or_else(|| Some("https://api.duckduckgo.com/".to_string())),
            enable_http_fetch: std::env::var("GATEWAY_HTTP_FETCH_ENABLED")
                .ok()
                .map(|value| !matches!(value.trim().to_ascii_lowercase().as_str(), "0" | "false" | "no"))
                .unwrap_or(true),
            notification_webhook_url: std::env::var("GATEWAY_NOTIFICATION_WEBHOOK").ok(),
            sensor_url: std::env::var("GATEWAY_SENSOR_URL").ok(),
        }
    }

    /// Build a capability registry from this config.
    pub fn build_registry(&self) -> Arc<GatewayCapabilityRegistry> {
        let registry = Arc::new(GatewayCapabilityRegistry::new());

        if let Some(web_search_url) = &self.web_search_url {
            registry.register(Arc::new(DuckDuckGoSearchCapability::new(
                web_search_url.clone(),
            )));
        }

        if self.enable_http_fetch {
            registry.register(Arc::new(HttpFetchCapability::new("http_fetch")));
        }

        if let Some(webhook_url) = &self.notification_webhook_url {
            registry.register(Arc::new(WebhookNotificationCapability::new(
                webhook_url.clone(),
            )));
        }

        if let Some(sensor_url) = &self.sensor_url {
            registry.register(Arc::new(ReadSensorCapability::new(sensor_url.clone())));
        }

        registry
    }
}

/// Convenience helper for the common case of loading built-ins from env vars.
pub fn built_in_capability_registry_from_env() -> Arc<GatewayCapabilityRegistry> {
    GatewayCapabilityRegistryConfig::from_env().build_registry()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_registers_web_search_and_http_fetch() {
        let registry = GatewayCapabilityRegistryConfig::default().build_registry();
        assert!(registry.contains("web_search"));
        assert!(registry.contains("http_fetch"));
        assert!(!registry.contains("send_notification"));
        assert!(!registry.contains("read_sensor"));
    }

    #[test]
    fn custom_config_registers_optional_capabilities() {
        let registry = GatewayCapabilityRegistryConfig {
            web_search_url: None,
            enable_http_fetch: false,
            notification_webhook_url: Some("https://example.test/webhook".to_string()),
            sensor_url: Some("https://example.test/sensor".to_string()),
        }
        .build_registry();

        assert!(!registry.contains("web_search"));
        assert!(!registry.contains("http_fetch"));
        assert!(registry.contains("send_notification"));
        assert!(registry.contains("read_sensor"));
    }
}
