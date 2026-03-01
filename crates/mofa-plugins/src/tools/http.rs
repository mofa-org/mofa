use super::*;
use reqwest::Client;
use serde_json::json;
use std::net::ToSocketAddrs;
use url::Url;

/// HTTP 请求工具 - 发送网络请求
/// HTTP request utilities - Send network requests
pub struct HttpRequestTool {
    definition: ToolDefinition,
    client: Client,
}

impl Default for HttpRequestTool {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpRequestTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                name: "http_request".to_string(),
                description: "Send HTTP requests (GET, POST, PUT, DELETE) to URLs. Useful for fetching web content or calling APIs.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "method": {
                            "type": "string",
                            "enum": ["GET", "POST", "PUT", "DELETE"],
                            "description": "HTTP method"
                        },
                        "url": {
                            "type": "string",
                            "description": "The URL to send the request to"
                        },
                        "headers": {
                            "type": "object",
                            "description": "Optional HTTP headers as key-value pairs",
                            "additionalProperties": { "type": "string" }
                        },
                        "body": {
                            "type": "string",
                            "description": "Optional request body for POST/PUT requests"
                        }
                    },
                    "required": ["method", "url"]
                }),
                requires_confirmation: true,
            },
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap(),
        }
    }

    /// Validate that a URL is safe to request (not targeting internal/private networks).
    fn is_url_allowed(url_str: &str) -> bool {
        let parsed = match Url::parse(url_str) {
            Ok(u) => u,
            Err(_) => return false,
        };

        // Only allow http and https schemes
        match parsed.scheme() {
            "http" | "https" => {}
            _ => return false,
        }

        let host = match parsed.host_str() {
            Some(h) => h,
            None => return false,
        };

        // Block well-known dangerous hostnames
        if host == "metadata.google.internal" {
            return false;
        }

        // Resolve hostname and check all resulting IPs
        let port = parsed
            .port()
            .unwrap_or(if parsed.scheme() == "https" { 443 } else { 80 });
        let socket_addrs = format!("{}:{}", host, port);
        let addrs: Vec<_> = match socket_addrs.to_socket_addrs() {
            Ok(a) => a.collect(),
            Err(_) => return false, // Deny if hostname cannot be resolved
        };

        if addrs.is_empty() {
            return false; // Deny if no addresses resolved
        }

        for addr in addrs {
            let ip = addr.ip();
            if ip.is_loopback()
                || ip.is_unspecified()
                || ip.is_multicast()
                || Self::is_private_ip(&ip)
            {
                return false;
            }
        }

        true
    }

    /// Check if an IP address is in a private/reserved range.
    fn is_private_ip(ip: &std::net::IpAddr) -> bool {
        match ip {
            std::net::IpAddr::V4(ipv4) => {
                let octets = ipv4.octets();
                // 10.0.0.0/8
                octets[0] == 10
                // 172.16.0.0/12
                || (octets[0] == 172 && (16..=31).contains(&octets[1]))
                // 192.168.0.0/16
                || (octets[0] == 192 && octets[1] == 168)
                // 169.254.0.0/16 (link-local, includes cloud metadata 169.254.169.254)
                || (octets[0] == 169 && octets[1] == 254)
                // 100.64.0.0/10 (carrier-grade NAT)
                || (octets[0] == 100 && (64..=127).contains(&octets[1]))
            }
            std::net::IpAddr::V6(ipv6) => {
                // Block unique local (fc00::/7) and link-local (fe80::/10)
                let segments = ipv6.segments();
                (segments[0] & 0xfe00) == 0xfc00 || (segments[0] & 0xffc0) == 0xfe80
            }
        }
    }
}

#[async_trait::async_trait]
impl ToolExecutor for HttpRequestTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, arguments: serde_json::Value) -> PluginResult<serde_json::Value> {
        let method = arguments["method"].as_str().unwrap_or("GET");
        let url = arguments["url"]
            .as_str()
            .ok_or_else(|| mofa_kernel::plugin::PluginError::ExecutionFailed("URL is required".to_string()))?;

        // Validate URL to prevent SSRF attacks
        if !Self::is_url_allowed(url) {
            return Err(mofa_kernel::plugin::PluginError::ExecutionFailed(format!(
                "Access denied: URL '{}' targets a blocked address (private/internal network or disallowed scheme)",
                url
            )));
        }

        let mut request = match method {
            "GET" => self.client.get(url),
            "POST" => self.client.post(url),
            "PUT" => self.client.put(url),
            "DELETE" => self.client.delete(url),
            _ => return Err(mofa_kernel::plugin::PluginError::ExecutionFailed(format!("Unsupported HTTP method: {}", method))),
        };

        // Add headers if provided
        if let Some(headers) = arguments.get("headers").and_then(|h| h.as_object()) {
            for (key, value) in headers {
                if let Some(v) = value.as_str() {
                    request = request.header(key.as_str(), v);
                }
            }
        }

        // Add body if provided
        if let Some(body) = arguments.get("body").and_then(|b| b.as_str()) {
            request = request.body(body.to_string());
        }

        let response = request.send().await
            .map_err(|e| mofa_kernel::plugin::PluginError::ExecutionFailed(e.to_string()))?;
        let status = response.status().as_u16();
        let headers: std::collections::HashMap<String, String> = response
            .headers()
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();

        let body = response.text().await
            .map_err(|e| mofa_kernel::plugin::PluginError::ExecutionFailed(e.to_string()))?;

        // Truncate body if too long
        let truncated_body = if body.len() > 5000 {
            format!(
                "{}... [truncated, total {} bytes]",
                &body[..5000],
                body.len()
            )
        } else {
            body
        };

        Ok(json!({
            "status": status,
            "headers": headers,
            "body": truncated_body
        }))
    }
}
