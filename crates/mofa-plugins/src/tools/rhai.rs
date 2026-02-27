use super::*;
use mofa_extra::rhai::{RhaiScriptEngine, ScriptContext, ScriptEngineConfig};
use serde_json::json;

/// Rhai 脚本执行工具 - 执行 Rhai 脚本
/// Rhai script execution tool - Execute Rhai scripts
pub struct RhaiScriptTool {
    definition: ToolDefinition,
    engine: RhaiScriptEngine,
}

impl RhaiScriptTool {
    pub fn new() -> PluginResult<Self> {
        let config = ScriptEngineConfig::default();
        let engine = RhaiScriptEngine::new(config)
            .map_err(|e| mofa_kernel::plugin::PluginError::InitFailed(e.to_string()))?;
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
            .ok_or_else(|| mofa_kernel::plugin::PluginError::ExecutionFailed("Script is required".into()))?;

        let mut context = ScriptContext::new();

        // Inject variables if provided
        if let Some(vars) = arguments.get("variables").and_then(|v| v.as_object()) {
            for (key, value) in vars {
                // Convert JSON values to Rhai-compatible types
                let me = |e: mofa_extra::rhai::RhaiError| mofa_kernel::plugin::PluginError::ExecutionFailed(e.to_string());
                match value {
                    serde_json::Value::Number(n) => {
                        if let Some(i) = n.as_i64() {
                            context = context.with_variable(key, i).map_err(me)?;
                        } else if let Some(f) = n.as_f64() {
                            context = context.with_variable(key, f).map_err(me)?;
                        }
                    }
                    serde_json::Value::String(s) => {
                        context = context.with_variable(key, s.clone()).map_err(me)?;
                    }
                    serde_json::Value::Bool(b) => {
                        context = context.with_variable(key, *b).map_err(me)?;
                    }
                    _ => {
                        // For complex types, pass as JSON string
                        context = context.with_variable(key, value.to_string()).map_err(me)?;
                    }
                }
            }
        }

        let result = self.engine.execute(script, &context).await
            .map_err(|e| mofa_kernel::plugin::PluginError::ExecutionFailed(e.to_string()))?;

        Ok(json!({
            "success": true,
            "result": result.value,
            "execution_time_ms": result.execution_time_ms
        }))
    }
}
