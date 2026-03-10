use super::*;
use reqwest::Client;
use serde_json::json;
use std::net::{IpAddr, Ipv4Addr, ToSocketAddrs};
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
                // Avoid following redirects to internal/private networks.
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .expect("failed to build HTTP client"),
        }
    }

    fn truncate_utf8(input: &str, max_bytes: usize) -> String {
        if input.len() <= max_bytes {
            return input.to_string();
        }

        let mut end = max_bytes.min(input.len());
        while end > 0 && !input.is_char_boundary(end) {
            end -= 1;
        }
        input[..end].to_string()
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
        if host.eq_ignore_ascii_case("metadata.google.internal") {
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
            if Self::is_blocked_ip(&addr.ip()) {
                return false;
            }
        }

        true
    }

    /// Check if an IP address is blocked for security reasons.
    fn is_blocked_ip(ip: &IpAddr) -> bool {
        match ip {
            IpAddr::V4(ipv4) => {
                ipv4.is_private()
                    || ipv4.is_loopback()
                    || ipv4.is_unspecified()
                    || ipv4.is_multicast()
                    || ipv4.is_link_local()
                    || ipv4.is_broadcast()
                    || ipv4.is_documentation()
                    || Self::is_cgnat_ipv4(*ipv4)
            }
            IpAddr::V6(ipv6) => {
                // Handle IPv4-mapped IPv6 addresses, e.g. ::ffff:127.0.0.1.
                if let Some(mapped) = ipv6.to_ipv4_mapped() {
                    return Self::is_blocked_ip(&IpAddr::V4(mapped));
                }

                ipv6.is_loopback()
                    || ipv6.is_unspecified()
                    || ipv6.is_multicast()
                    || ipv6.is_unique_local()
                    || ipv6.is_unicast_link_local()
                    || Self::is_documentation_ipv6(*ipv6)
            }
        }
    }

    fn is_cgnat_ipv4(ipv4: Ipv4Addr) -> bool {
        let octets = ipv4.octets();
        octets[0] == 100 && (64..=127).contains(&octets[1])
    }

    fn is_documentation_ipv6(ipv6: std::net::Ipv6Addr) -> bool {
        let segments = ipv6.segments();
        // 2001:db8::/32
        segments[0] == 0x2001 && segments[1] == 0x0db8
    }
}

#[async_trait::async_trait]
impl ToolExecutor for HttpRequestTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, arguments: serde_json::Value) -> PluginResult<serde_json::Value> {
        let method = arguments["method"].as_str().unwrap_or("GET");
        let url = arguments["url"].as_str().ok_or_else(|| {
            mofa_kernel::plugin::PluginError::ExecutionFailed("URL is required".to_string())
        })?;

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
            _ => {
                return Err(mofa_kernel::plugin::PluginError::ExecutionFailed(format!(
                    "Unsupported HTTP method: {}",
                    method
                )));
            }
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

        let response = request
            .send()
            .await
            .map_err(|e| mofa_kernel::plugin::PluginError::ExecutionFailed(e.to_string()))?;
        let status = response.status().as_u16();
        let headers: std::collections::HashMap<String, String> = response
            .headers()
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();

        let body = response
            .text()
            .await
            .map_err(|e| mofa_kernel::plugin::PluginError::ExecutionFailed(e.to_string()))?;

        // Truncate body if too long
        let truncated_body = if body.len() > 5000 {
            format!(
                "{}... [truncated, total {} bytes]",
                Self::truncate_utf8(&body, 5000),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_ipv4_mapped_loopback() {
        let ip: IpAddr = "::ffff:127.0.0.1".parse().unwrap();
        assert!(HttpRequestTool::is_blocked_ip(&ip));
    }

    #[test]
    fn blocks_localhost_url() {
        assert!(!HttpRequestTool::is_url_allowed("http://127.0.0.1:8080/"));
    }

    #[test]
    fn blocks_ipv4_mapped_loopback_url() {
        assert!(!HttpRequestTool::is_url_allowed(
            "http://[::ffff:127.0.0.1]:8080/"
        ));
    }

    #[test]
    fn blocks_metadata_hostname_case_insensitively() {
        assert!(!HttpRequestTool::is_url_allowed(
            "http://METADATA.GOOGLE.INTERNAL/"
        ));
    }

    #[test]
    fn truncates_utf8_without_panicking() {
        let value = "A🙂B";
        assert_eq!(HttpRequestTool::truncate_utf8(value, 2), "A");
    }
}
