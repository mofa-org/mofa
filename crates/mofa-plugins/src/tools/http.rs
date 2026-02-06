use super::*;
use reqwest::Client;
use serde_json::json;

/// HTTP 请求工具 - 发送网络请求
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
                requires_confirmation: false,
            },
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap(),
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
            .ok_or_else(|| anyhow::anyhow!("URL is required"))?;

        let mut request = match method {
            "GET" => self.client.get(url),
            "POST" => self.client.post(url),
            "PUT" => self.client.put(url),
            "DELETE" => self.client.delete(url),
            _ => return Err(anyhow::anyhow!("Unsupported HTTP method: {}", method)),
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

        let response = request.send().await?;
        let status = response.status().as_u16();
        let headers: std::collections::HashMap<String, String> = response
            .headers()
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();

        let body = response.text().await?;

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
