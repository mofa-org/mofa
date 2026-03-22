use async_trait::async_trait;
use reqwest::Client;
use serde_json::Value;
use std::collections::HashMap;
use std::time::Instant;

use mofa_kernel::agent::types::error::{GlobalError, GlobalResult};

use crate::gateway::{CapabilityRequest, CapabilityResponse, GatewayCapability};

/// Generic HTTP GET capability that fetches a configured endpoint.
pub struct HttpFetchCapability {
    client: Client,
    name: String,
    url_param: String,
}

/// POST-backed notification capability for webhooks such as Slack.
pub struct WebhookNotificationCapability {
    client: Client,
    name: String,
    webhook_url: String,
}

impl WebhookNotificationCapability {
    /// Create a new notification capability using the provided webhook URL.
    pub fn new(webhook_url: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            name: "send_notification".to_string(),
            webhook_url: webhook_url.into(),
        }
    }

    /// Override the registry-visible capability name.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }
}

/// HTTP GET capability for sensor or IoT endpoints.
pub struct ReadSensorCapability {
    client: Client,
    name: String,
    endpoint_url: String,
}

impl ReadSensorCapability {
    /// Create a new sensor capability using the provided endpoint URL.
    pub fn new(endpoint_url: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            name: "read_sensor".to_string(),
            endpoint_url: endpoint_url.into(),
        }
    }

    /// Override the registry-visible capability name.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }
}

impl HttpFetchCapability {
    /// Create a new HTTP fetch capability that reads the target URL from params.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            name: name.into(),
            url_param: "url".to_string(),
        }
    }

    /// Override the request parameter used to carry the target URL.
    pub fn with_url_param(mut self, url_param: impl Into<String>) -> Self {
        self.url_param = url_param.into();
        self
    }
}

#[async_trait]
impl GatewayCapability for HttpFetchCapability {
    fn name(&self) -> &str {
        &self.name
    }

    async fn invoke(&self, input: CapabilityRequest) -> GlobalResult<CapabilityResponse> {
        let target_url = input
            .params
            .get(&self.url_param)
            .and_then(Value::as_str)
            .ok_or_else(|| {
                GlobalError::Other(format!("missing '{}' param for {}", self.url_param, self.name))
            })?;

        let start = Instant::now();
        let response = self
            .client
            .get(target_url)
            .send()
            .await
            .map_err(|e| GlobalError::Other(format!("http fetch request failed: {e}")))?;

        let status = response.status().as_u16();
        let body = response
            .text()
            .await
            .map_err(|e| GlobalError::Other(format!("http fetch body read failed: {e}")))?;

        Ok(CapabilityResponse {
            output: body,
            metadata: HashMap::from([
                ("status".to_string(), Value::from(status)),
                ("url".to_string(), Value::String(target_url.to_string())),
                ("trace_id".to_string(), Value::String(input.trace_id)),
            ]),
            latency_ms: start.elapsed().as_millis() as u64,
        })
    }
}

/// DuckDuckGo-backed web search capability.
pub struct DuckDuckGoSearchCapability {
    client: Client,
    name: String,
    base_url: String,
}

impl DuckDuckGoSearchCapability {
    /// Create a new web search capability using the provided base URL.
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            name: "web_search".to_string(),
            base_url: base_url.into(),
        }
    }

    /// Override the registry-visible capability name.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }
}

#[async_trait]
impl GatewayCapability for DuckDuckGoSearchCapability {
    fn name(&self) -> &str {
        &self.name
    }

    async fn invoke(&self, input: CapabilityRequest) -> GlobalResult<CapabilityResponse> {
        let query = input.input.trim();
        if query.is_empty() {
            return Err(GlobalError::Other("web search query must not be empty".to_string()));
        }

        let start = Instant::now();
        let response = self
            .client
            .get(&self.base_url)
            .query(&[("q", query), ("format", "json"), ("no_html", "1")])
            .send()
            .await
            .map_err(|e| GlobalError::Other(format!("web search request failed: {e}")))?;

        let status = response.status().as_u16();
        let payload: Value = response
            .json()
            .await
            .map_err(|e| GlobalError::Other(format!("web search JSON parse failed: {e}")))?;

        let output = payload
            .get("AbstractText")
            .and_then(Value::as_str)
            .filter(|text| !text.trim().is_empty())
            .map(ToOwned::to_owned)
            .or_else(|| {
                payload
                    .get("Heading")
                    .and_then(Value::as_str)
                    .filter(|text| !text.trim().is_empty())
                    .map(ToOwned::to_owned)
            })
            .unwrap_or_else(|| payload.to_string());

        Ok(CapabilityResponse {
            output,
            metadata: HashMap::from([
                ("status".to_string(), Value::from(status)),
                ("query".to_string(), Value::String(query.to_string())),
                ("trace_id".to_string(), Value::String(input.trace_id)),
            ]),
            latency_ms: start.elapsed().as_millis() as u64,
        })
    }
}

#[async_trait]
impl GatewayCapability for WebhookNotificationCapability {
    fn name(&self) -> &str {
        &self.name
    }

    async fn invoke(&self, input: CapabilityRequest) -> GlobalResult<CapabilityResponse> {
        let start = Instant::now();
        let response = self
            .client
            .post(&self.webhook_url)
            .json(&serde_json::json!({
                "text": input.input,
                "params": input.params,
                "trace_id": input.trace_id,
            }))
            .send()
            .await
            .map_err(|e| GlobalError::Other(format!("notification request failed: {e}")))?;

        let status = response.status().as_u16();
        let body = response
            .text()
            .await
            .map_err(|e| GlobalError::Other(format!("notification body read failed: {e}")))?;

        Ok(CapabilityResponse {
            output: body,
            metadata: HashMap::from([
                ("status".to_string(), Value::from(status)),
                ("webhook_url".to_string(), Value::String(self.webhook_url.clone())),
            ]),
            latency_ms: start.elapsed().as_millis() as u64,
        })
    }
}

#[async_trait]
impl GatewayCapability for ReadSensorCapability {
    fn name(&self) -> &str {
        &self.name
    }

    async fn invoke(&self, input: CapabilityRequest) -> GlobalResult<CapabilityResponse> {
        let start = Instant::now();
        let response = self
            .client
            .get(&self.endpoint_url)
            .query(&[("trace_id", input.trace_id.clone())])
            .send()
            .await
            .map_err(|e| GlobalError::Other(format!("sensor read request failed: {e}")))?;

        let status = response.status().as_u16();
        let body = response
            .text()
            .await
            .map_err(|e| GlobalError::Other(format!("sensor body read failed: {e}")))?;

        Ok(CapabilityResponse {
            output: body,
            metadata: HashMap::from([
                ("status".to_string(), Value::from(status)),
                ("endpoint_url".to_string(), Value::String(self.endpoint_url.clone())),
            ]),
            latency_ms: start.elapsed().as_millis() as u64,
        })
    }
}

#[cfg(all(test, feature = "http-api"))]
mod tests {
    use super::*;
    use axum::{Json, Router, routing::get};
    use tokio::net::TcpListener;

    async fn spawn_test_server(app: Router) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        format!("http://{}", addr)
    }

    #[tokio::test]
    async fn http_fetch_capability_returns_body_and_metadata() {
        let base_url = spawn_test_server(Router::new().route(
            "/payload",
            get(|| async { "sensor=24.5C" }),
        ))
        .await;

        let capability = HttpFetchCapability::new("http_fetch");
        let response = capability
            .invoke(CapabilityRequest {
                input: "ignored".to_string(),
                params: HashMap::from([(
                    "url".to_string(),
                    Value::String(format!("{base_url}/payload")),
                )]),
                trace_id: "trace-http".to_string(),
            })
            .await
            .unwrap();

        assert_eq!(response.output, "sensor=24.5C");
        assert_eq!(
            response.metadata.get("status"),
            Some(&Value::from(200_u16))
        );
    }

    #[tokio::test]
    async fn duckduckgo_capability_prefers_abstract_text() {
        let base_url = spawn_test_server(Router::new().route(
            "/",
            get(|| async {
                Json(serde_json::json!({
                    "AbstractText": "MoFA summary result",
                    "Heading": "Ignored heading"
                }))
            }),
        ))
        .await;

        let capability = DuckDuckGoSearchCapability::new(base_url);
        let response = capability
            .invoke(CapabilityRequest {
                input: "mofa".to_string(),
                params: HashMap::new(),
                trace_id: "trace-search".to_string(),
            })
            .await
            .unwrap();

        assert_eq!(response.output, "MoFA summary result");
        assert_eq!(
            response.metadata.get("query"),
            Some(&Value::String("mofa".to_string()))
        );
    }

    #[tokio::test]
    async fn webhook_notification_posts_json_payload() {
        let base_url = spawn_test_server(Router::new().route(
            "/notify",
            axum::routing::post(|Json(payload): Json<Value>| async move { Json(payload) }),
        ))
        .await;

        let capability = WebhookNotificationCapability::new(format!("{base_url}/notify"));
        let response = capability
            .invoke(CapabilityRequest {
                input: "Ship it".to_string(),
                params: HashMap::from([("channel".to_string(), Value::String("alerts".to_string()))]),
                trace_id: "trace-notify".to_string(),
            })
            .await
            .unwrap();

        let echoed: Value = serde_json::from_str(&response.output).unwrap();
        assert_eq!(echoed.get("text"), Some(&Value::String("Ship it".to_string())));
    }

    #[tokio::test]
    async fn read_sensor_fetches_endpoint_text() {
        let base_url = spawn_test_server(Router::new().route(
            "/sensor",
            get(|| async { "temperature=21.2" }),
        ))
        .await;

        let capability = ReadSensorCapability::new(format!("{base_url}/sensor"));
        let response = capability
            .invoke(CapabilityRequest {
                input: "ignored".to_string(),
                params: HashMap::new(),
                trace_id: "trace-sensor".to_string(),
            })
            .await
            .unwrap();

        assert_eq!(response.output, "temperature=21.2");
    }
}
