use super::*;
use serde_json::json;

/// 计算器工具 - 数学表达式计算
/// Calculator tool - Mathematical expression calculation
pub struct CalculatorTool {
    definition: ToolDefinition,
}

impl Default for CalculatorTool {
    fn default() -> Self {
        Self::new()
    }
}

impl CalculatorTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                name: "calculator".to_string(),
                description: "Perform mathematical calculations: basic arithmetic, powers, roots, trigonometry.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "operation": {
                            "type": "string",
                            "enum": ["add", "subtract", "multiply", "divide", "power", "sqrt", "sin", "cos", "tan", "log", "ln", "abs", "floor", "ceil", "round"],
                            "description": "Mathematical operation"
                        },
                        "a": {
                            "type": "number",
                            "description": "First operand"
                        },
                        "b": {
                            "type": "number",
                            "description": "Second operand (for binary operations)"
                        }
                    },
                    "required": ["operation", "a"]
                }),
                requires_confirmation: false,
            },
        }
    }
}

#[async_trait::async_trait]
impl ToolExecutor for CalculatorTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, arguments: serde_json::Value) -> PluginResult<serde_json::Value> {
        let operation = arguments["operation"]
            .as_str()
            .ok_or_else(|| mofa_kernel::plugin::PluginError::ExecutionFailed(format!("Operation is required")))?;
        let a = arguments["a"]
            .as_f64()
            .ok_or_else(|| mofa_kernel::plugin::PluginError::ExecutionFailed(format!("Operand 'a' is required")))?;

        let result = match operation {
            "add" => {
                let b = arguments["b"]
                    .as_f64()
                    .ok_or_else(|| mofa_kernel::plugin::PluginError::ExecutionFailed(format!("Operand 'b' is required for add")))?;
                a + b
            }
            "subtract" => {
                let b = arguments["b"]
                    .as_f64()
                    .ok_or_else(|| mofa_kernel::plugin::PluginError::ExecutionFailed(format!("Operand 'b' is required for subtract")))?;
                a - b
            }
            "multiply" => {
                let b = arguments["b"]
                    .as_f64()
                    .ok_or_else(|| mofa_kernel::plugin::PluginError::ExecutionFailed(format!("Operand 'b' is required for multiply")))?;
                a * b
            }
            "divide" => {
                let b = arguments["b"]
                    .as_f64()
                    .ok_or_else(|| mofa_kernel::plugin::PluginError::ExecutionFailed(format!("Operand 'b' is required for divide")))?;
                if b == 0.0 {
                    return Err(mofa_kernel::plugin::PluginError::ExecutionFailed(format!("Division by zero")));
                }
                a / b
            }
            "power" => {
                let b = arguments["b"]
                    .as_f64()
                    .ok_or_else(|| mofa_kernel::plugin::PluginError::ExecutionFailed(format!("Operand 'b' is required for power")))?;
                a.powf(b)
            }
            "sqrt" => {
                if a < 0.0 {
                    return Err(mofa_kernel::plugin::PluginError::ExecutionFailed(
                        "Cannot compute square root of negative number".into(),
                    ));
                }
                a.sqrt()
            }
            "sin" => a.sin(),
            "cos" => a.cos(),
            "tan" => a.tan(),
            "log" => {
                let base = arguments["b"].as_f64().unwrap_or(10.0);
                a.log(base)
            }
            "ln" => a.ln(),
            "abs" => a.abs(),
            "floor" => a.floor(),
            "ceil" => a.ceil(),
            "round" => a.round(),
            _ => return Err(mofa_kernel::plugin::PluginError::ExecutionFailed(format!("Unknown operation: {}", operation))),
        };

        Ok(json!({
            "operation": operation,
            "operands": { "a": a, "b": arguments.get("b") },
            "result": result
        }))
    }
}
