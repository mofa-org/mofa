use super::*;
use chrono::{DateTime, Local, TimeZone, Utc};
use serde_json::json;

/// 日期时间工具 - 获取当前时间、格式化日期
/// Date and time utilities - Get current time, format dates
pub struct DateTimeTool {
    definition: ToolDefinition,
}

impl Default for DateTimeTool {
    fn default() -> Self {
        Self::new()
    }
}

impl DateTimeTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                name: "datetime".to_string(),
                description: "Date and time operations: get current time, format dates, calculate time differences.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "operation": {
                            "type": "string",
                            "enum": ["now", "format", "parse", "add", "diff"],
                            "description": "Operation to perform"
                        },
                        "format": {
                            "type": "string",
                            "description": "Date format string (e.g., '%Y-%m-%d %H:%M:%S')"
                        },
                        "timestamp": {
                            "type": "integer",
                            "description": "Unix timestamp in seconds"
                        },
                        "date_string": {
                            "type": "string",
                            "description": "Date string to parse"
                        },
                        "timezone": {
                            "type": "string",
                            "description": "Timezone (e.g., 'UTC', 'Local')"
                        },
                        "duration_seconds": {
                            "type": "integer",
                            "description": "Duration in seconds for add operation"
                        }
                    },
                    "required": ["operation"]
                }),
                requires_confirmation: false,
            },
        }
    }
}

#[async_trait::async_trait]
impl ToolExecutor for DateTimeTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, arguments: serde_json::Value) -> PluginResult<serde_json::Value> {
        let operation = arguments["operation"]
            .as_str()
            .ok_or_else(|| mofa_kernel::plugin::PluginError::ExecutionFailed("Operation is required".to_string()))?;

        match operation {
            "now" => {
                let now_utc = Utc::now();
                let now_local = Local::now();
                Ok(json!({
                    "utc": now_utc.to_rfc3339(),
                    "local": now_local.to_rfc3339(),
                    "timestamp": now_utc.timestamp(),
                    "timestamp_millis": now_utc.timestamp_millis(),
                    "formatted": now_local.format("%Y-%m-%d %H:%M:%S").to_string()
                }))
            }
            "format" => {
                let timestamp = arguments["timestamp"]
                    .as_i64()
                    .ok_or_else(|| mofa_kernel::plugin::PluginError::ExecutionFailed("Timestamp is required for format operation".to_string()))?;
                let format = arguments["format"].as_str().unwrap_or("%Y-%m-%d %H:%M:%S");
                let timezone = arguments["timezone"].as_str().unwrap_or("UTC");

                let formatted = if timezone == "Local" {
                    Local
                        .timestamp_opt(timestamp, 0)
                        .single()
                        .map(|dt| dt.format(format).to_string())
                } else {
                    Utc.timestamp_opt(timestamp, 0)
                        .single()
                        .map(|dt| dt.format(format).to_string())
                };

                match formatted {
                    Some(f) => Ok(json!({
                        "formatted": f,
                        "timestamp": timestamp
                    })),
                    None => Err(mofa_kernel::plugin::PluginError::ExecutionFailed("Invalid timestamp".to_string())),
                }
            }
            "parse" => {
                let date_string = arguments["date_string"].as_str().ok_or_else(|| {
                    mofa_kernel::plugin::PluginError::ExecutionFailed("date_string is required for parse operation".to_string())
                })?;

                // Try RFC3339 first
                if let Ok(dt) = DateTime::parse_from_rfc3339(date_string) {
                    return Ok(json!({
                        "timestamp": dt.timestamp(),
                        "utc": dt.to_utc().to_rfc3339()
                    }));
                }

                // Try common format
                if let Ok(dt) =
                    chrono::NaiveDateTime::parse_from_str(date_string, "%Y-%m-%d %H:%M:%S")
                {
                    return Ok(json!({
                        "timestamp": dt.and_utc().timestamp(),
                        "utc": dt.and_utc().to_rfc3339()
                    }));
                }

                Err(mofa_kernel::plugin::PluginError::ExecutionFailed(format!(
                    "Could not parse date string: {}",
                    date_string
                )))
            }
            "add" => {
                let timestamp = arguments["timestamp"]
                    .as_i64()
                    .unwrap_or_else(|| Utc::now().timestamp());
                let duration = arguments["duration_seconds"].as_i64().ok_or_else(|| {
                    mofa_kernel::plugin::PluginError::ExecutionFailed("duration_seconds is required for add operation".to_string())
                })?;

                let new_timestamp = timestamp + duration;
                let dt = Utc.timestamp_opt(new_timestamp, 0).single();

                match dt {
                    Some(dt) => Ok(json!({
                        "original_timestamp": timestamp,
                        "new_timestamp": new_timestamp,
                        "utc": dt.to_rfc3339()
                    })),
                    None => Err(mofa_kernel::plugin::PluginError::ExecutionFailed("Invalid resulting timestamp".to_string())),
                }
            }
            "diff" => {
                let ts1 = arguments["timestamp1"]
                    .as_i64()
                    .ok_or_else(|| mofa_kernel::plugin::PluginError::ExecutionFailed("timestamp1 is required for diff operation".to_string()))?;
                let ts2 = arguments["timestamp2"]
                    .as_i64()
                    .ok_or_else(|| mofa_kernel::plugin::PluginError::ExecutionFailed("timestamp2 is required for diff operation".to_string()))?;

                let diff = ts2 - ts1;
                let days = diff / 86400;
                let hours = (diff % 86400) / 3600;
                let minutes = (diff % 3600) / 60;
                let seconds = diff % 60;

                Ok(json!({
                    "diff_seconds": diff,
                    "diff_human": format!("{}d {}h {}m {}s", days, hours, minutes, seconds)
                }))
            }
            _ => Err(mofa_kernel::plugin::PluginError::ExecutionFailed(format!("Unknown operation: {}", operation))),
        }
    }
}
