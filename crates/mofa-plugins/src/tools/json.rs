use super::*;
use serde_json::json;

/// JSON 处理工具 - JSON 解析和操作
/// JSON processing utilities - JSON parsing and manipulation
pub struct JsonTool {
    definition: ToolDefinition,
}

impl Default for JsonTool {
    fn default() -> Self {
        Self::new()
    }
}

impl JsonTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                name: "json".to_string(),
                description: "JSON operations: parse, stringify, query with JSONPath-like syntax, merge objects.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "operation": {
                            "type": "string",
                            "enum": ["parse", "stringify", "get", "set", "merge", "keys", "values"],
                            "description": "JSON operation to perform"
                        },
                        "data": {
                            "description": "JSON data (string for parse, object/array for others)"
                        },
                        "path": {
                            "type": "string",
                            "description": "Dot-notation path for get/set operations (e.g., 'user.name')"
                        },
                        "value": {
                            "description": "Value to set (for set operation)"
                        },
                        "other": {
                            "type": "object",
                            "description": "Object to merge with (for merge operation)"
                        }
                    },
                    "required": ["operation", "data"]
                }),
                requires_confirmation: false,
            },
        }
    }

    fn get_by_path<'a>(value: &'a serde_json::Value, path: &str) -> Option<&'a serde_json::Value> {
        let parts: Vec<&str> = path.split('.').collect();
        let mut current = value;

        for part in parts {
            if part.is_empty() {
                continue;
            }
            // Check if it's an array index
            if let Ok(index) = part.parse::<usize>() {
                current = current.get(index)?;
            } else {
                current = current.get(part)?;
            }
        }

        Some(current)
    }

    fn set_by_path(
        value: &mut serde_json::Value,
        path: &str,
        new_value: serde_json::Value,
    ) -> PluginResult<()> {
        let parts: Vec<&str> = path.split('.').collect();
        let mut current = value;

        for (i, part) in parts.iter().enumerate() {
            if i == parts.len() - 1 {
                // Last part - set the value
                if let Ok(index) = part.parse::<usize>() {
                    if let Some(arr) = current.as_array_mut()
                        && index < arr.len()
                    {
                        arr[index] = new_value;
                        return Ok(());
                    }
                } else if let Some(obj) = current.as_object_mut() {
                    obj.insert(part.to_string(), new_value);
                    return Ok(());
                }
                return Err(mofa_kernel::plugin::PluginError::ExecutionFailed("Cannot set value at path".to_string()));
            } else {
                // Navigate to next level
                if let Ok(index) = part.parse::<usize>() {
                    current = current
                        .get_mut(index)
                        .ok_or_else(|| mofa_kernel::plugin::PluginError::ExecutionFailed("Invalid path".to_string()))?;
                } else {
                    current = current
                        .get_mut(*part)
                        .ok_or_else(|| mofa_kernel::plugin::PluginError::ExecutionFailed("Invalid path".to_string()))?;
                }
            }
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl ToolExecutor for JsonTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, arguments: serde_json::Value) -> PluginResult<serde_json::Value> {
        let operation = arguments["operation"]
            .as_str()
            .ok_or_else(|| mofa_kernel::plugin::PluginError::ExecutionFailed("Operation is required".to_string()))?;

        match operation {
            "parse" => {
                let data = arguments["data"]
                    .as_str()
                    .ok_or_else(|| mofa_kernel::plugin::PluginError::ExecutionFailed("String data is required for parse".to_string()))?;
                let parsed: serde_json::Value = serde_json::from_str(data)?;
                Ok(json!({
                    "success": true,
                    "result": parsed
                }))
            }
            "stringify" => {
                let data = &arguments["data"];
                let pretty = arguments
                    .get("pretty")
                    .and_then(|p| p.as_bool())
                    .unwrap_or(true);
                let result = if pretty {
                    serde_json::to_string_pretty(data)?
                } else {
                    serde_json::to_string(data)?
                };
                Ok(json!({
                    "success": true,
                    "result": result
                }))
            }
            "get" => {
                let path = arguments["path"]
                    .as_str()
                    .ok_or_else(|| mofa_kernel::plugin::PluginError::ExecutionFailed("Path is required for get operation".to_string()))?;
                let data = &arguments["data"];
                let result = Self::get_by_path(data, path);
                Ok(json!({
                    "success": result.is_some(),
                    "result": result
                }))
            }
            "set" => {
                let path = arguments["path"]
                    .as_str()
                    .ok_or_else(|| mofa_kernel::plugin::PluginError::ExecutionFailed("Path is required for set operation".to_string()))?;
                let value = arguments
                    .get("value")
                    .ok_or_else(|| mofa_kernel::plugin::PluginError::ExecutionFailed("Value is required for set operation".to_string()))?
                    .clone();
                let mut data = arguments["data"].clone();
                Self::set_by_path(&mut data, path, value)?;
                Ok(json!({
                    "success": true,
                    "result": data
                }))
            }
            "merge" => {
                let mut data = arguments["data"]
                    .as_object()
                    .ok_or_else(|| mofa_kernel::plugin::PluginError::ExecutionFailed("Data must be an object for merge".to_string()))?
                    .clone();
                let other = arguments["other"]
                    .as_object()
                    .ok_or_else(|| mofa_kernel::plugin::PluginError::ExecutionFailed("Other must be an object for merge".to_string()))?;
                for (k, v) in other {
                    data.insert(k.clone(), v.clone());
                }
                Ok(json!({
                    "success": true,
                    "result": data
                }))
            }
            "keys" => {
                let data = arguments["data"]
                    .as_object()
                    .ok_or_else(|| mofa_kernel::plugin::PluginError::ExecutionFailed("Data must be an object for keys".to_string()))?;
                let keys: Vec<&String> = data.keys().collect();
                Ok(json!({
                    "success": true,
                    "result": keys
                }))
            }
            "values" => {
                let data = arguments["data"]
                    .as_object()
                    .ok_or_else(|| mofa_kernel::plugin::PluginError::ExecutionFailed("Data must be an object for values".to_string()))?;
                let values: Vec<&serde_json::Value> = data.values().collect();
                Ok(json!({
                    "success": true,
                    "result": values
                }))
            }
            _ => Err(mofa_kernel::plugin::PluginError::ExecutionFailed(format!("Unknown operation: {}", operation))),
        }
    }
}
