//! OpenAI-compatible backend proxy.
//!
//! [`OpenAiBackend`] forwards requests to any OpenAI-compatible completion
//! endpoint (OpenAI, Azure OpenAI, local LLMs running Ollama / llama.cpp,
//! etc.) and relays the response body verbatim.
//!
//! The proxy is intentionally transparent: it does not parse or modify the
//! request/response JSON, which means it is forward-compatible with new
//! model parameters without code changes.

use crate::error::{GatewayImplError, GatewayResult};
use mofa_kernel::gateway::{GatewayRequest, GatewayResponse};
use reqwest::Client;
use std::time::Duration;
use tracing::{debug, instrument};

/// Proxies requests to an OpenAI-compatible REST API endpoint.
pub struct OpenAiBackend {
    backend_id: String,
    base_url: String,
    api_key: Option<String>,
    client: Client,
}

impl OpenAiBackend {
    /// Create a new backend proxy.
    ///
    /// - `backend_id`: registry id (used in metrics and error messages).
    /// - `base_url`:  base URL, e.g. `https://api.openai.com`.
    /// - `api_key`:   optional API key.  When `None` the gateway relies on the
    ///   caller to supply its own credentials via `Authorization` header.
    pub fn new(
        backend_id: impl Into<String>,
        base_url: impl Into<String>,
        api_key: Option<String>,
    ) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .expect("failed to build reqwest client");

        Self {
            backend_id: backend_id.into(),
            base_url: base_url.into().trim_end_matches('/').to_string(),
            api_key,
            client,
        }
    }

    /// Forward `req` to `{base_url}{req.path}` and return a [`GatewayResponse`].
    #[instrument(skip(self, req), fields(backend = %self.backend_id, path = %req.path))]
    pub async fn forward(&self, req: &GatewayRequest) -> GatewayResult<GatewayResponse> {
        let url = format!("{}{}", self.base_url, req.path);
        debug!(url = %url, "forwarding to OpenAI-compatible backend");

        let start = std::time::Instant::now();

        // Match on a reference to avoid moving out of the borrowed GatewayRequest.
        let mut builder = match &req.method {
            mofa_kernel::gateway::HttpMethod::Post => self.client.post(&url),
            mofa_kernel::gateway::HttpMethod::Get => self.client.get(&url),
            mofa_kernel::gateway::HttpMethod::Put => self.client.put(&url),
            mofa_kernel::gateway::HttpMethod::Delete => self.client.delete(&url),
            mofa_kernel::gateway::HttpMethod::Patch => self.client.patch(&url),
            _ => self.client.post(&url), // HEAD/OPTIONS: fall back to POST for proxy
        };

        // Forward headers from the original request, except `host`.
        for (key, value) in &req.headers {
            if key == "host" || key == "content-length" {
                continue;
            }
            builder = builder.header(key, value);
        }

        // Inject backend API key if configured, overriding any caller-supplied key.
        if let Some(key) = &self.api_key {
            builder = builder.header("authorization", format!("Bearer {}", key));
        }

        // Send body if present.
        if !req.body.is_empty() {
            builder = builder
                .header("content-type", "application/json")
                .body(req.body.clone());
        }

        let upstream_resp = builder.send().await.map_err(|e| GatewayImplError::NetworkError {
            backend_id: self.backend_id.clone(),
            source: e,
        })?;

        let latency_ms = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);
        let status = upstream_resp.status().as_u16();

        // Collect response headers.
        let mut headers = std::collections::HashMap::new();
        for (name, value) in upstream_resp.headers() {
            if let Ok(v) = value.to_str() {
                headers.insert(name.to_string(), v.to_string());
            }
        }

        let body = upstream_resp.bytes().await.map_err(|e| GatewayImplError::NetworkError {
            backend_id: self.backend_id.clone(),
            source: e,
        })?;

        // Surface 5xx responses as errors; 4xx are proxied transparently so
        // callers can inspect the structured error body (e.g. OpenAI 429 JSON).
        if status >= 500 {
            return Err(GatewayImplError::UpstreamError {
                backend_id: self.backend_id.clone(),
                status,
                message: String::from_utf8_lossy(&body).to_string(),
            });
        }
        // TODO: plumb RouteMatch.timeout_ms into per-request reqwest timeout
        // once the proxy_handler passes GatewayContext instead of GatewayRequest.

        let mut resp = GatewayResponse::new(status, &self.backend_id);
        resp.headers = headers;
        resp.body = body.to_vec();
        resp.latency_ms = latency_ms;
        Ok(resp)
    }
}
