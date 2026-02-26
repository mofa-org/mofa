//! Function-calling bridge between LLM tool-use responses and the Rhai engine.
//!
//! [`FunctionCallingAdapter`] performs two jobs:
//!
//! 1. **Schema export** — converts registered [`ToolDefinition`]s into the
//!    OpenAI function-calling JSON schema so they can be attached to a chat
//!    completion request.
//!
//! 2. **Call routing** — when the LLM responds with a [`ToolCall`], the adapter
//!    dispatches it to the corresponding Rhai function inside the loaded script
//!    and returns a [`ToolResult`] that can be fed back to the conversation.
//!
//! # Wiring a Rhai plugin script
//!
//! The Rhai script must expose a function whose name matches every registered
//! tool name and that accepts a single JSON-encoded argument map:
//!
//! ```rhai
//! fn search_web(params) {
//!     // params.query is the value the LLM passed
//!     "Results for: " + params.query
//! }
//! ```

use mofa_extra::rhai::{RhaiScriptEngine, ScriptContext};
use mofa_kernel::plugin::PluginResult;
use std::collections::HashMap;
use std::sync::Arc;

use crate::{ToolCall, ToolDefinition, ToolResult};

// ============================================================================
// FunctionCallingAdapter
// ============================================================================

/// Bridges LLM function-calling with the Rhai scripting engine.
///
/// Register one [`ToolDefinition`] per Rhai function you want the LLM to be
/// able to invoke.  Then:
///
/// * Call [`to_openai_tools`](Self::to_openai_tools) to get the `tools` array
///   to include in the OpenAI request.
/// * Call [`route_tool_call`](Self::route_tool_call) for every
///   `tool_calls` entry the LLM returns to execute the matching Rhai function
///   and obtain the `ToolResult` to append to the message history.
pub struct FunctionCallingAdapter {
    /// Registered tools, keyed by name.
    tools: HashMap<String, ToolDefinition>,
    /// Shared Rhai script engine.
    engine: Arc<RhaiScriptEngine>,
    /// Cache key for the compiled plugin script.
    script_id: String,
}

impl FunctionCallingAdapter {
    /// Create a new adapter backed by the given engine and script cache entry.
    ///
    /// `script_id` must match the id used when the script was compiled with
    /// [`RhaiScriptEngine::compile_and_cache`]; the adapter does **not**
    /// compile the script itself.
    pub fn new(engine: Arc<RhaiScriptEngine>, script_id: impl Into<String>) -> Self {
        Self {
            tools: HashMap::new(),
            engine,
            script_id: script_id.into(),
        }
    }

    /// Register a tool definition.
    ///
    /// The adapter will include it in the OpenAI schema and route calls for it
    /// to the Rhai function of the same name.
    pub fn register_tool(&mut self, def: ToolDefinition) {
        self.tools.insert(def.name.clone(), def);
    }

    /// Remove a previously registered tool by name.
    pub fn unregister_tool(&mut self, name: &str) {
        self.tools.remove(name);
    }

    /// Return all registered [`ToolDefinition`]s.
    pub fn registered_tools(&self) -> impl Iterator<Item = &ToolDefinition> {
        self.tools.values()
    }

    // -------------------------------------------------------------------------
    // Schema helpers
    // -------------------------------------------------------------------------

    /// Convert all registered tools into the OpenAI `tools` array format.
    ///
    /// The returned value can be serialised directly as the `tools` field of an
    /// OpenAI chat completion request body.
    ///
    /// ```json
    /// [
    ///   {
    ///     "type": "function",
    ///     "function": {
    ///       "name": "search_web",
    ///       "description": "Search the web for a query",
    ///       "parameters": { "type": "object", "properties": { ... } }
    ///     }
    ///   }
    /// ]
    /// ```
    pub fn to_openai_tools(&self) -> serde_json::Value {
        let schemas: Vec<serde_json::Value> = self
            .tools
            .values()
            .map(Self::tool_to_openai_schema)
            .collect();
        serde_json::json!(schemas)
    }

    /// Convert a single [`ToolDefinition`] to the OpenAI function schema object.
    pub fn tool_to_openai_schema(def: &ToolDefinition) -> serde_json::Value {
        serde_json::json!({
            "type": "function",
            "function": {
                "name": def.name,
                "description": def.description,
                "parameters": def.parameters,
            }
        })
    }

    /// Full JSON-Schema representation of a tool (includes all definition fields).
    pub fn definition_to_json_schema(def: &ToolDefinition) -> serde_json::Value {
        serde_json::json!({
            "name": def.name,
            "description": def.description,
            "parameters": def.parameters,
            "requires_confirmation": def.requires_confirmation,
        })
    }

    // -------------------------------------------------------------------------
    // Call routing
    // -------------------------------------------------------------------------

    /// Dispatch an LLM [`ToolCall`] to the matching Rhai function and return
    /// the [`ToolResult`] to feed back into the conversation.
    ///
    /// The Rhai function receives `call.arguments` as its sole argument and
    /// must return a JSON-serialisable value.
    ///
    /// If the tool is **not registered**, or the Rhai function is **not found**
    /// in the script, a [`ToolResult`] with `success: false` is returned
    /// (no `Err` is propagated) so the conversation can continue gracefully.
    pub async fn route_tool_call(&self, call: &ToolCall) -> PluginResult<ToolResult> {
        // Verify the tool is known.
        if !self.tools.contains_key(&call.name) {
            return Ok(ToolResult {
                call_id: call.call_id.clone(),
                success: false,
                result: serde_json::Value::Null,
                error: Some(format!(
                    "Tool '{}' is not registered in this adapter",
                    call.name
                )),
            });
        }

        let context = ScriptContext::new();
        let args = vec![call.arguments.clone()];

        match self
            .engine
            .call_function::<serde_json::Value>(&self.script_id, &call.name, args, &context)
            .await
        {
            Ok(result) => Ok(ToolResult {
                call_id: call.call_id.clone(),
                success: true,
                result,
                error: None,
            }),
            Err(e) => {
                // Distinguish "function not found" (script author omission) from
                // genuine runtime errors so callers can decide how to handle each.
                let msg = e.to_string().to_lowercase();
                let is_missing = msg.contains("function not found")
                    || msg.contains("not found in module")
                    || msg.contains("undefined");

                Ok(ToolResult {
                    call_id: call.call_id.clone(),
                    success: false,
                    result: serde_json::Value::Null,
                    error: Some(if is_missing {
                        format!(
                            "Rhai function '{}' not found in script '{}' \
                             — add `fn {}(params){{}}` to the script",
                            call.name, self.script_id, call.name
                        )
                    } else {
                        e.to_string()
                    }),
                })
            }
        }
    }

    /// Compile the plugin script into the engine cache under `script_id`.
    ///
    /// Call this once after constructing the adapter (or after hot-reloading
    /// the script content) so that [`route_tool_call`](Self::route_tool_call)
    /// can find the compiled AST.
    pub async fn compile_script(&self, script_content: &str) -> PluginResult<()> {
        self.engine
            .compile_and_cache(&self.script_id, "function_calling_script", script_content)
            .await
            .map_err(|e| mofa_kernel::plugin::PluginError::ExecutionFailed(format!("Failed to compile script '{}': {}", self.script_id, e)))
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use mofa_extra::rhai::ScriptEngineConfig;

    fn make_tool(name: &str, description: &str) -> ToolDefinition {
        ToolDefinition {
            name: name.to_string(),
            description: description.to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Search query" }
                },
                "required": ["query"]
            }),
            requires_confirmation: false,
        }
    }

    #[test]
    fn test_tool_to_openai_schema() {
        let tool = make_tool("search_web", "Search the web");
        let schema = FunctionCallingAdapter::tool_to_openai_schema(&tool);

        assert_eq!(schema["type"], "function");
        assert_eq!(schema["function"]["name"], "search_web");
        assert_eq!(schema["function"]["description"], "Search the web");
        // parameters should be forwarded verbatim
        assert_eq!(schema["function"]["parameters"]["type"], "object");
    }

    #[test]
    fn test_definition_to_json_schema_includes_confirmation() {
        let mut tool = make_tool("delete_file", "Delete a file");
        tool.requires_confirmation = true;
        let schema = FunctionCallingAdapter::definition_to_json_schema(&tool);

        assert_eq!(schema["requires_confirmation"], true);
    }

    #[test]
    fn test_to_openai_tools_multiple() {
        let engine =
            Arc::new(RhaiScriptEngine::new(ScriptEngineConfig::default()).expect("engine"));
        let mut adapter = FunctionCallingAdapter::new(engine, "test_script");
        adapter.register_tool(make_tool("tool_a", "A"));
        adapter.register_tool(make_tool("tool_b", "B"));

        let tools = adapter.to_openai_tools();
        let arr = tools.as_array().expect("array");
        assert_eq!(arr.len(), 2);
        let names: Vec<&str> = arr
            .iter()
            .map(|t| t["function"]["name"].as_str().unwrap())
            .collect();
        assert!(names.contains(&"tool_a"));
        assert!(names.contains(&"tool_b"));
    }

    #[tokio::test]
    async fn test_route_unknown_tool_returns_error_result() {
        let engine =
            Arc::new(RhaiScriptEngine::new(ScriptEngineConfig::default()).expect("engine"));
        let adapter = FunctionCallingAdapter::new(engine, "test_script");

        let call = ToolCall {
            name: "nonexistent".to_string(),
            arguments: serde_json::json!({}),
            call_id: "call-1".to_string(),
        };

        let result = adapter.route_tool_call(&call).await.unwrap();
        assert!(!result.success);
        assert!(result.error.is_some());
    }

    #[tokio::test]
    async fn test_route_registered_tool_calls_rhai_function() {
        let script = r#"
            fn greet(params) {
                "Hello, " + params.name
            }
        "#;

        let engine =
            Arc::new(RhaiScriptEngine::new(ScriptEngineConfig::default()).expect("engine"));
        let mut adapter = FunctionCallingAdapter::new(engine, "greet_script");
        adapter.register_tool(ToolDefinition {
            name: "greet".to_string(),
            description: "Greet someone".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string" }
                }
            }),
            requires_confirmation: false,
        });

        adapter.compile_script(script).await.unwrap();

        let call = ToolCall {
            name: "greet".to_string(),
            arguments: serde_json::json!({ "name": "World" }),
            call_id: "call-2".to_string(),
        };

        let result = adapter.route_tool_call(&call).await.unwrap();
        assert!(result.success, "expected success, got: {:?}", result.error);
        assert_eq!(result.result, serde_json::json!("Hello, World"));
    }
}
