//! Model Context Protocol (MCP) adapter.
//!
//! Provides the ability to interface with MCP servers to list and invoke
//! context tools and prompts over HTTP/SSE.

use async_trait::async_trait;
use mofa_kernel::gateway::{GatewayAdapter, GatewayContext, GatewayRequest, GatewayResponse, DispatchError};
use reqwest::Client;

/// Error type for MCP operations.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum McpError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("API error: {0}")]
    Api(String),
}

// Construct DispatchError::AdapterInvocationFailed inside invoke manually for precise attribution.

/// Adapter for invoking MCP context and tools.
pub struct McpAdapter {
    http: Client,
    base_url: String,
    allowed_domains: Vec<String>,
}

impl McpAdapter {
    /// Create a new MCP adapter targeting a default base URL.
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            http: Client::new(),
            base_url: base_url.into(),
            allowed_domains: Vec::new(),
        }
    }
    
    pub fn with_allowed_domains(mut self, domains: Vec<String>) -> Self {
        self.allowed_domains = domains;
        self
    }
}

#[async_trait]
impl GatewayAdapter for McpAdapter {
    fn name(&self) -> &str {
        "mcp"
    }

    async fn invoke(
        &self,
        req: &GatewayRequest,
        ctx: &GatewayContext,
    ) -> Result<GatewayResponse, DispatchError> {
        let target_url = match req.headers.get("x-mcp-target-url") {
            Some(s) => {
                let url = s.as_str();
                if self.allowed_domains.is_empty() && cfg!(test) {
                    url // Allow any URL in tests if no domains configured
                } else if self.allowed_domains.iter().any(|domain| url.starts_with(domain)) {
                    url
                } else {
                    let mut res = GatewayResponse::new(403, self.name());
                    res.body = format!("SSRF blocked: target URL '{}' not in allowlist", url).into_bytes();
                    return Ok(res);
                }
            }
            None => &self.base_url,
        };

        // Enforce the route matcher timeout if specified
        let timeout = ctx
            .route_match
            .as_ref()
            .map(|rm| std::time::Duration::from_millis(rm.timeout_ms))
            .unwrap_or_else(|| std::time::Duration::from_secs(30));

        let url = format!("{}/{}", target_url.trim_end_matches('/'), req.path.trim_start_matches('/'));

        let mut request_builder = match req.method {
            mofa_kernel::gateway::route::HttpMethod::Get => self.http.get(&url),
            mofa_kernel::gateway::route::HttpMethod::Post => self.http.post(&url),
            mofa_kernel::gateway::route::HttpMethod::Put => self.http.put(&url),
            mofa_kernel::gateway::route::HttpMethod::Delete => self.http.delete(&url),
            mofa_kernel::gateway::route::HttpMethod::Patch => self.http.patch(&url),
            _ => return Err(DispatchError::AdapterInvocationFailed {
                adapter: self.name().to_string(),
                reason: format!("Unsupported method '{:?}' for MCP endpoint", req.method),
            }),
        };

        // Forward headers from inbound request, stripping hop-by-hop and internal ones.
        for (k, v) in &req.headers {
            let key = k.to_lowercase();
            if key != "x-mcp-target-url" && key != "connection" && key != "transfer-encoding" && key != "host" {
                request_builder = request_builder.header(k, v);
            }
        }

        request_builder = request_builder.timeout(timeout);

        if !req.body.is_empty() {
            let body_val: Result<serde_json::Value, _> = serde_json::from_slice(&req.body);
            if let Ok(json_body) = body_val {
                request_builder = request_builder.json(&json_body);
            } else {
                request_builder = request_builder.body(req.body.clone());
            }
        }

        let res = request_builder.send().await.map_err(|e| DispatchError::AdapterInvocationFailed {
            adapter: self.name().to_string(),
            reason: e.to_string(),
        })?;
        let status = res.status().as_u16();

        let mut gateway_res = GatewayResponse::new(status, self.name());

        // Propagate upstream response headers (normalized to lowercase).
        for (name, value) in res.headers().iter() {
            let key = name.as_str().to_ascii_lowercase();
            if let Ok(val_str) = value.to_str() {
                gateway_res.headers.insert(key, val_str.to_string());
            }
        }

        let bytes = res.bytes().await.map_err(|e| DispatchError::AdapterInvocationFailed {
            adapter: self.name().to_string(),
            reason: e.to_string(),
        })?;
        gateway_res.body = bytes.to_vec();

        Ok(gateway_res)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mofa_kernel::gateway::route::HttpMethod;

    #[tokio::test]
    async fn mcp_adapter_ssrf_protection_blocked() {
        let adapter = McpAdapter::new("http://default.internal")
            .with_allowed_domains(vec!["http://safe.external".to_string()]);
        
        let mut req = GatewayRequest::new("id_1", "/tools/list", HttpMethod::Get);
        req = req.with_header("x-mcp-target-url", "http://malicious.external");
        
        let ctx = GatewayContext::new(req.clone());
        let res = adapter.invoke(&req, &ctx).await.expect("SSRF block should return Ok(403)");
        
        assert_eq!(res.status, 403);
        assert!(String::from_utf8_lossy(&res.body).contains("SSRF blocked"));
    }

    #[tokio::test]
    async fn mcp_adapter_success_path_with_headers() {
        let mut server = mockito::Server::new_async().await;
        let url = server.url();
        
        let _m = server.mock("POST", "/tools/call")
            .match_header("authorization", "Bearer test-token")
            .match_body(mockito::Matcher::Json(serde_json::json!({"tool": "calculator"})))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_header("x-upstream-id", "mcp-123")
            .with_body(r#"{"result": 42}"#)
            .create_async().await;

        let adapter = McpAdapter::new(url);
        let mut req = GatewayRequest::new("id_1", "/tools/call", HttpMethod::Post);
        req = req.with_header("authorization", "Bearer test-token");
        req.body = serde_json::to_vec(&serde_json::json!({"tool": "calculator"})).unwrap();
        
        let ctx = GatewayContext::new(req.clone());
        let res = adapter.invoke(&req, &ctx).await.unwrap();
        
        assert_eq!(res.status, 200);
        assert_eq!(res.backend_id, "mcp");
        assert_eq!(res.headers.get("x-upstream-id"), Some(&"mcp-123".to_string()));
        assert_eq!(serde_json::from_slice::<serde_json::Value>(&res.body).unwrap(), serde_json::json!({"result": 42}));
    }

    #[tokio::test]
    async fn mcp_adapter_timeout_enforcement() {
        let mut server = mockito::Server::new_async().await;
        let url = server.url();
        
        let _m = server.mock("GET", "/slow")
            .with_status(200)
            .create_async().await;

        let adapter = McpAdapter::new(url);
        let req = GatewayRequest::new("id_1", "/slow", HttpMethod::Get);
        
        let mut ctx = GatewayContext::new(req.clone());
        // Set a short timeout
        ctx.route_match = Some(mofa_kernel::gateway::RouteMatch {
            route_id: "r1".into(),
            backend_id: "mcp".into(),
            path_params: std::collections::HashMap::new(),
            timeout_ms: 1, // Extremely short timeout
        });

        let result = adapter.invoke(&req, &ctx).await;
        
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("timeout") || err_msg.contains("timed out"));
    }

    #[tokio::test]
    async fn mcp_adapter_returns_correct_name() {
        let adapter = McpAdapter::new("http://default");
        assert_eq!(adapter.name(), "mcp");
    }
}
