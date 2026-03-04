//! ReAct 内置工具实现
//! ReAct Built-in Tool Implementations
//!
//! 提供常用工具的实现示例
//! Provides implementation examples for common tools

use super::core::ReActTool;
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value;

/// Type alias for tool handler function
pub type ToolHandler = Box<dyn Fn(&str) -> Result<String, String> + Send + Sync>;

/// 计算器工具
/// Calculator Tool
///
/// 支持基本的数学表达式计算
/// Supports basic mathematical expression evaluation
pub struct CalculatorTool;

#[async_trait]
impl ReActTool for CalculatorTool {
    fn name(&self) -> &str {
        "calculator"
    }

    fn description(&self) -> &str {
        "Perform mathematical calculations. Input should be a mathematical expression like '2 + 2' or '(10 * 5) / 2'"
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(serde_json::json!({
            "type": "object",
            "properties": {
                "expression": {
                    "type": "string",
                    "description": "The mathematical expression to evaluate"
                }
            },
            "required": ["expression"]
        }))
    }

    async fn execute(&self, input: &str) -> Result<String, String> {
        // 尝试解析 JSON 输入
        // Attempt to parse JSON input
        let expression = if let Ok(json) = serde_json::from_str::<Value>(input) {
            json.get("expression")
                .and_then(|v| v.as_str())
                .unwrap_or(input)
                .to_string()
        } else {
            input.to_string()
        };

        // 简单的表达式计算 (仅支持基本运算)
        // Simple expression calculation (only supports basic operations)
        match evaluate_expression(&expression) {
            Ok(result) => Ok(format!("{}", result)),
            Err(e) => Err(format!("Calculation error: {}", e)),
        }
    }
}

/// 简单的表达式求值器
/// Simple expression evaluator
fn evaluate_expression(expr: &str) -> Result<f64, String> {
    let expr = expr.trim();

    // 处理括号
    // Handle parentheses
    if expr.starts_with('(') && expr.ends_with(')') {
        // 检查是否是完整的括号表达式
        // Check if it is a complete parenthetical expression
        let inner = &expr[1..expr.len() - 1];
        if is_balanced(inner) {
            return evaluate_expression(inner);
        }
    }

    // 查找最低优先级的运算符 (从右向左，处理左结合性)
    // Find the lowest priority operator (right-to-left for left-associativity)
    let mut paren_depth = 0;
    let mut last_add_sub = None;
    let mut last_mul_div = None;

    let chars: Vec<char> = expr.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        match c {
            '(' => paren_depth += 1,
            ')' => paren_depth -= 1,
            '+' | '-' if paren_depth == 0 && i > 0 => {
                // 确保不是负号
                // Ensure it is not a negative sign
                let prev = chars.get(i.saturating_sub(1)).copied().unwrap_or(' ');
                if !matches!(prev, '+' | '-' | '*' | '/' | '(') {
                    last_add_sub = Some(i);
                }
            }
            '*' | '/' if paren_depth == 0 => {
                last_mul_div = Some(i);
            }
            _ => {}
        }
    }

    // 先处理加减，再处理乘除
    // Handle addition/subtraction first, then multiplication/division
    if let Some(pos) = last_add_sub {
        let left = evaluate_expression(&expr[..pos])?;
        let right = evaluate_expression(&expr[pos + 1..])?;
        return match chars[pos] {
            '+' => Ok(left + right),
            '-' => Ok(left - right),
            _ => unreachable!(),
        };
    }

    if let Some(pos) = last_mul_div {
        let left = evaluate_expression(&expr[..pos])?;
        let right = evaluate_expression(&expr[pos + 1..])?;
        return match chars[pos] {
            '*' => Ok(left * right),
            '/' => {
                if right == 0.0 {
                    Err("Division by zero".to_string())
                } else {
                    Ok(left / right)
                }
            }
            _ => unreachable!(),
        };
    }

    // 尝试解析为数字
    // Try to parse as a number
    expr.parse::<f64>()
        .map_err(|_| format!("Invalid expression: {}", expr))
}

/// 检查括号是否平衡
/// Check if parentheses are balanced
fn is_balanced(s: &str) -> bool {
    let mut depth = 0;
    for c in s.chars() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth < 0 {
                    return false;
                }
            }
            _ => {}
        }
    }
    depth == 0
}

/// 字符串工具
/// String Tool
///
/// 提供字符串处理功能
/// Provides string processing functionality
pub struct StringTool;

#[async_trait]
impl ReActTool for StringTool {
    fn name(&self) -> &str {
        "string"
    }

    fn description(&self) -> &str {
        "Perform string operations. Operations: 'length', 'upper', 'lower', 'reverse', 'count'"
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(serde_json::json!({
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "enum": ["length", "upper", "lower", "reverse", "count"],
                    "description": "The string operation to perform"
                },
                "text": {
                    "type": "string",
                    "description": "The text to operate on"
                },
                "pattern": {
                    "type": "string",
                    "description": "Pattern for count operation (optional)"
                }
            },
            "required": ["operation", "text"]
        }))
    }

    async fn execute(&self, input: &str) -> Result<String, String> {
        #[derive(Deserialize)]
        struct StringInput {
            operation: String,
            text: String,
            pattern: Option<String>,
        }

        // 尝试解析 JSON
        // Attempt to parse JSON
        let params: StringInput = if let Ok(p) = serde_json::from_str(input) {
            p
        } else {
            // 简单格式: operation:text
            // Simple format: operation:text
            let parts: Vec<&str> = input.splitn(2, ':').collect();
            if parts.len() < 2 {
                return Err("Invalid input format. Use JSON or 'operation:text'".to_string());
            }
            StringInput {
                operation: parts[0].trim().to_string(),
                text: parts[1].trim().to_string(),
                pattern: None,
            }
        };

        match params.operation.as_str() {
            "length" => Ok(params.text.len().to_string()),
            "upper" => Ok(params.text.to_uppercase()),
            "lower" => Ok(params.text.to_lowercase()),
            "reverse" => Ok(params.text.chars().rev().collect()),
            "count" => {
                let pattern = params.pattern.as_deref().unwrap_or(" ");
                Ok(params.text.matches(pattern).count().to_string())
            }
            _ => Err(format!("Unknown operation: {}", params.operation)),
        }
    }
}

/// JSON 工具
/// JSON Tool
///
/// 提供 JSON 解析和查询功能
/// Provides JSON parsing and querying functionality
pub struct JsonTool;

#[async_trait]
impl ReActTool for JsonTool {
    fn name(&self) -> &str {
        "json"
    }

    fn description(&self) -> &str {
        "Parse and query JSON data. Operations: 'parse', 'get', 'keys', 'stringify'"
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(serde_json::json!({
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "enum": ["parse", "get", "keys", "stringify"],
                    "description": "The JSON operation to perform"
                },
                "data": {
                    "type": "string",
                    "description": "The JSON data to operate on"
                },
                "path": {
                    "type": "string",
                    "description": "JSON path for 'get' operation (e.g., 'user.name')"
                }
            },
            "required": ["operation", "data"]
        }))
    }

    async fn execute(&self, input: &str) -> Result<String, String> {
        #[derive(Deserialize)]
        struct JsonInput {
            operation: String,
            data: String,
            path: Option<String>,
        }

        let params: JsonInput =
            serde_json::from_str(input).map_err(|e| format!("Invalid JSON input: {}", e))?;

        let json: Value =
            serde_json::from_str(&params.data).map_err(|e| format!("Invalid JSON data: {}", e))?;

        match params.operation.as_str() {
            "parse" => Ok(format!("Parsed successfully: {}", json)),
            "get" => {
                let path = params.path.ok_or("Path required for 'get' operation")?;
                let mut current = &json;
                for key in path.split('.') {
                    current = current
                        .get(key)
                        .ok_or_else(|| format!("Key '{}' not found", key))?;
                }
                Ok(current.to_string())
            }
            "keys" => {
                if let Some(obj) = json.as_object() {
                    let keys: Vec<&str> = obj.keys().map(|s| s.as_str()).collect();
                    Ok(format!("{:?}", keys))
                } else {
                    Err("Not a JSON object".to_string())
                }
            }
            "stringify" => {
                serde_json::to_string_pretty(&json).map_err(|e| format!("Stringify error: {}", e))
            }
            _ => Err(format!("Unknown operation: {}", params.operation)),
        }
    }
}

/// 日期时间工具
/// DateTime Tool
///
/// 提供日期和时间相关功能
/// Provides date and time related functionality
pub struct DateTimeTool;

#[async_trait]
impl ReActTool for DateTimeTool {
    fn name(&self) -> &str {
        "datetime"
    }

    fn description(&self) -> &str {
        "Get current date/time information. Operations: 'now', 'timestamp', 'format'"
    }

    async fn execute(&self, input: &str) -> Result<String, String> {
        let operation = input.trim().to_lowercase();

        use std::time::{SystemTime, UNIX_EPOCH};

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?;

        match operation.as_str() {
            "now" | "current" => {
                let secs = now.as_secs();
                // 简单的 UTC 时间格式化
                // Simple UTC time formatting
                let days_since_epoch = secs / 86400;
                let time_of_day = secs % 86400;
                let hours = time_of_day / 3600;
                let minutes = (time_of_day % 3600) / 60;
                let seconds = time_of_day % 60;

                // 简化的日期计算 (从 1970-01-01 开始)
                // Simplified date calculation (starting from 1970-01-01)
                let (year, month, day) = days_to_date(days_since_epoch);

                Ok(format!(
                    "{:04}-{:02}-{:02} {:02}:{:02}:{:02} UTC",
                    year, month, day, hours, minutes, seconds
                ))
            }
            "timestamp" | "unix" => Ok(now.as_secs().to_string()),
            "millis" | "milliseconds" => Ok(now.as_millis().to_string()),
            _ => Err(format!(
                "Unknown operation: {}. Use 'now', 'timestamp', or 'millis'",
                operation
            )),
        }
    }
}

/// 简化的日期计算
/// Simplified date calculation
fn days_to_date(days: u64) -> (u64, u64, u64) {
    // 从 1970-01-01 计算
    // Calculate from 1970-01-01
    let mut remaining = days as i64;
    let mut year = 1970i64;

    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if remaining < days_in_year {
            break;
        }
        remaining -= days_in_year;
        year += 1;
    }

    let mut month = 1u64;
    let days_in_months = if is_leap_year(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    for days_in_month in days_in_months {
        if remaining < days_in_month {
            break;
        }
        remaining -= days_in_month;
        month += 1;
    }

    (year as u64, month, remaining as u64 + 1)
}

fn is_leap_year(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

/// Echo 工具 (测试用)
/// Echo Tool (for testing)
///
/// 简单地回显输入
/// Simply echoes the input back
pub struct EchoTool;

#[async_trait]
impl ReActTool for EchoTool {
    fn name(&self) -> &str {
        "echo"
    }

    fn description(&self) -> &str {
        "Echo the input back. Useful for testing."
    }

    async fn execute(&self, input: &str) -> Result<String, String> {
        Ok(format!("Echo: {}", input))
    }
}

/// 工具注册表便捷函数
/// Convenience functions for tool registry
pub mod prelude {
    use super::*;
    use std::sync::Arc;

    /// 创建计算器工具
    /// Creates a calculator tool
    pub fn calculator() -> Arc<dyn ReActTool> {
        Arc::new(CalculatorTool)
    }

    /// 创建字符串工具
    /// Creates a string tool
    pub fn string_tool() -> Arc<dyn ReActTool> {
        Arc::new(StringTool)
    }

    /// 创建 JSON 工具
    /// Creates a JSON tool
    pub fn json_tool() -> Arc<dyn ReActTool> {
        Arc::new(JsonTool)
    }

    /// 创建日期时间工具
    /// Creates a datetime tool
    pub fn datetime_tool() -> Arc<dyn ReActTool> {
        Arc::new(DateTimeTool)
    }

    /// 创建 Echo 工具
    /// Creates an echo tool
    pub fn echo_tool() -> Arc<dyn ReActTool> {
        Arc::new(EchoTool)
    }

    /// 获取所有内置工具
    /// Gets all built-in tools
    pub fn all_builtin_tools() -> Vec<Arc<dyn ReActTool>> {
        vec![
            calculator(),
            string_tool(),
            json_tool(),
            datetime_tool(),
            echo_tool(),
        ]
    }
}

/// 自定义工具构建器
/// Custom Tool Builder
///
/// 方便创建简单的自定义工具
/// Facilitates the creation of simple custom tools
pub struct CustomToolBuilder {
    name: String,
    description: String,
    parameters_schema: Option<Value>,
    handler: Option<ToolHandler>,
}

impl CustomToolBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: String::new(),
            parameters_schema: None,
            handler: None,
        }
    }

    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    pub fn parameters(mut self, schema: Value) -> Self {
        self.parameters_schema = Some(schema);
        self
    }

    pub fn handler<F>(mut self, f: F) -> Self
    where
        F: Fn(&str) -> Result<String, String> + Send + Sync + 'static,
    {
        self.handler = Some(Box::new(f));
        self
    }

    pub fn build(self) -> Option<CustomTool> {
        Some(CustomTool {
            name: self.name,
            description: self.description,
            parameters_schema: self.parameters_schema,
            handler: self.handler?,
        })
    }
}

/// 自定义工具
/// Custom Tool
pub struct CustomTool {
    name: String,
    description: String,
    parameters_schema: Option<Value>,
    handler: ToolHandler,
}

#[async_trait]
impl ReActTool for CustomTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn parameters_schema(&self) -> Option<Value> {
        self.parameters_schema.clone()
    }

    async fn execute(&self, input: &str) -> Result<String, String> {
        (self.handler)(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculator() {
        assert_eq!(evaluate_expression("2 + 2").unwrap(), 4.0);
        assert_eq!(evaluate_expression("10 * 5").unwrap(), 50.0);
        assert_eq!(evaluate_expression("(2 + 3) * 4").unwrap(), 20.0);
        assert_eq!(evaluate_expression("100 / 4").unwrap(), 25.0);
    }

    #[test]
    fn test_date_calculation() {
        // 1970-01-01 is day 0 since epoch
        let (y, m, d) = days_to_date(0);
        assert_eq!(y, 1970);
        assert_eq!(m, 1);
        assert_eq!(d, 1);

        // 1970-01-02 is day 1
        let (y, m, d) = days_to_date(1);
        assert_eq!(y, 1970);
        assert_eq!(m, 1);
        assert_eq!(d, 2);
    }

    #[tokio::test]
    async fn test_echo_tool() {
        let tool = EchoTool;
        let result = tool.execute("hello").await.unwrap();
        assert_eq!(result, "Echo: hello");
    }

    #[tokio::test]
    async fn test_string_tool() {
        let tool = StringTool;
        let result = tool.execute("upper:hello").await.unwrap();
        assert_eq!(result, "HELLO");
    }
}
