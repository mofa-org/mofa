
use super::*;
use mofa_extra::rhai::{RhaiScriptEngine, ScriptContext, ScriptEngineConfig};
use serde_json::json;

/// Rhai 脚本执行工具 - 执行 Rhai 脚本
pub struct RhaiScriptTool {
    definition: ToolDefinition,
    engine: RhaiScriptEngine,
}

impl RhaiScriptTool {
    pub fn new() -> PluginResult<Self> {
        let config = ScriptEngineConfig::default();
        let engine = RhaiScriptEngine::new(config)?;
        Ok(Self {
            definition: ToolDefinition {
                name: "rhai_script".to_string(),
                description: "Execute Rhai scripts for complex calculations or data processing. Rhai is a safe embedded scripting language.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "script": {
                            "type": "string",
                            "description": "Rhai script code to execute"
                        },
                        "variables": {
                            "type": "object",
                            "description": "Variables to inject into script context",
                            "additionalProperties": true
                        }
                    },
                    "required": ["script"]
                }),
                requires_confirmation: true,
            },
            engine,
        })
    }
}

#[async_trait::async_trait]
impl ToolExecutor for RhaiScriptTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, arguments: serde_json::Value) -> PluginResult<serde_json::Value> {
        let script = arguments["script"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Script is required"))?;

        let mut context = ScriptContext::new();

        // Inject variables if provided
        if let Some(vars) = arguments.get("variables").and_then(|v| v.as_object()) {
            for (key, value) in vars {
                // Convert JSON values to Rhai-compatible types
                match value {
                    serde_json::Value::Number(n) => {
                        if let Some(i) = n.as_i64() {
                            context = context.with_variable(key, i)?;
                        } else if let Some(f) = n.as_f64() {
                            context = context.with_variable(key, f)?;
                        }
                    }
                    serde_json::Value::String(s) => {
                        context = context.with_variable(key, s.clone())?;
                    }
                    serde_json::Value::Bool(b) => {
                        context = context.with_variable(key, *b)?;
                    }
                    _ => {
                        // For complex types, pass as JSON string
                        context = context.with_variable(key, value.to_string())?;
                    }
                }
            }
        }

        let result = self.engine.execute(script, &context).await?;

        Ok(json!({
            "success": true,
            "result": result.value,
            "execution_time_ms": result.execution_time_ms
        }))
    }
}
