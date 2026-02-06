//! Rhai 动态工具系统
//!
//! 允许通过 Rhai 脚本动态定义和执行工具，实现：
//! - 脚本化的工具定义
//! - 运行时工具注册
//! - 工具参数验证
//! - 工具执行沙箱

use super::engine::{RhaiScriptEngine, ScriptContext, ScriptEngineConfig};
use anyhow::{Result, anyhow};
#[allow(unused_imports)]
use rhai::{Dynamic, Engine, Map, Scope};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

// ============================================================================
// 工具参数定义
// ============================================================================

/// 参数类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum ParameterType {
    #[default]
    String,
    Integer,
    Float,
    Boolean,
    Array,
    Object,
    Any,
}

/// 工具参数定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolParameter {
    /// 参数名称
    pub name: String,
    /// 参数类型
    #[serde(default)]
    pub param_type: ParameterType,
    /// 参数描述
    #[serde(default)]
    pub description: String,
    /// 是否必需
    #[serde(default)]
    pub required: bool,
    /// 默认值
    pub default: Option<serde_json::Value>,
    /// 枚举值（如果有）
    pub enum_values: Option<Vec<serde_json::Value>>,
    /// 最小值（数字类型）
    pub minimum: Option<f64>,
    /// 最大值（数字类型）
    pub maximum: Option<f64>,
    /// 最小长度（字符串/数组）
    pub min_length: Option<usize>,
    /// 最大长度（字符串/数组）
    pub max_length: Option<usize>,
    /// 正则表达式模式（字符串）
    pub pattern: Option<String>,
}

impl ToolParameter {
    pub fn new(name: &str, param_type: ParameterType) -> Self {
        Self {
            name: name.to_string(),
            param_type,
            description: String::new(),
            required: false,
            default: None,
            enum_values: None,
            minimum: None,
            maximum: None,
            min_length: None,
            max_length: None,
            pattern: None,
        }
    }

    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }

    pub fn with_description(mut self, desc: &str) -> Self {
        self.description = desc.to_string();
        self
    }

    pub fn with_default<T: Serialize>(mut self, value: T) -> Self {
        self.default = serde_json::to_value(value).ok();
        self
    }

    pub fn with_enum(mut self, values: Vec<serde_json::Value>) -> Self {
        self.enum_values = Some(values);
        self
    }

    pub fn with_range(mut self, min: f64, max: f64) -> Self {
        self.minimum = Some(min);
        self.maximum = Some(max);
        self
    }

    /// 验证参数值
    pub fn validate(&self, value: &serde_json::Value) -> Result<()> {
        // 检查类型
        match (&self.param_type, value) {
            (ParameterType::String, serde_json::Value::String(_)) => {}
            (ParameterType::Integer, serde_json::Value::Number(n)) if n.is_i64() => {}
            (ParameterType::Float, serde_json::Value::Number(_)) => {}
            (ParameterType::Boolean, serde_json::Value::Bool(_)) => {}
            (ParameterType::Array, serde_json::Value::Array(_)) => {}
            (ParameterType::Object, serde_json::Value::Object(_)) => {}
            (ParameterType::Any, _) => {}
            (ParameterType::String, serde_json::Value::Null) if !self.required => {}
            _ => {
                return Err(anyhow!(
                    "Parameter '{}' has invalid type, expected {:?}",
                    self.name,
                    self.param_type
                ));
            }
        }

        // 检查枚举值
        if let Some(ref enum_values) = self.enum_values
            && !enum_values.contains(value)
        {
            return Err(anyhow!(
                "Parameter '{}' value must be one of {:?}",
                self.name,
                enum_values
            ));
        }

        // 检查数值范围
        if let serde_json::Value::Number(n) = value
            && let Some(f) = n.as_f64()
        {
            if let Some(min) = self.minimum
                && f < min
            {
                return Err(anyhow!("Parameter '{}' must be >= {}", self.name, min));
            }
            if let Some(max) = self.maximum
                && f > max
            {
                return Err(anyhow!("Parameter '{}' must be <= {}", self.name, max));
            }
        }

        // 检查字符串长度
        if let serde_json::Value::String(s) = value {
            if let Some(min) = self.min_length
                && s.len() < min
            {
                return Err(anyhow!(
                    "Parameter '{}' length must be >= {}",
                    self.name,
                    min
                ));
            }
            if let Some(max) = self.max_length
                && s.len() > max
            {
                return Err(anyhow!(
                    "Parameter '{}' length must be <= {}",
                    self.name,
                    max
                ));
            }
            // 检查正则表达式
            if let Some(ref pattern) = self.pattern {
                let re = regex::Regex::new(pattern)
                    .map_err(|e| anyhow!("Invalid regex pattern: {}", e))?;
                if !re.is_match(s) {
                    return Err(anyhow!(
                        "Parameter '{}' does not match pattern: {}",
                        self.name,
                        pattern
                    ));
                }
            }
        }

        // 检查数组长度
        if let serde_json::Value::Array(arr) = value {
            if let Some(min) = self.min_length
                && arr.len() < min
            {
                return Err(anyhow!(
                    "Parameter '{}' array length must be >= {}",
                    self.name,
                    min
                ));
            }
            if let Some(max) = self.max_length
                && arr.len() > max
            {
                return Err(anyhow!(
                    "Parameter '{}' array length must be <= {}",
                    self.name,
                    max
                ));
            }
        }

        Ok(())
    }
}

// ============================================================================
// 脚本工具定义
// ============================================================================

/// 脚本工具定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptToolDefinition {
    /// 工具 ID
    pub id: String,
    /// 工具名称
    pub name: String,
    /// 工具描述
    pub description: String,
    /// 参数定义
    pub parameters: Vec<ToolParameter>,
    /// 脚本源代码
    pub script: String,
    /// 入口函数名（默认 "execute"）
    #[serde(default = "default_entry_function")]
    pub entry_function: String,
    /// 是否启用缓存
    #[serde(default = "default_true")]
    pub enable_cache: bool,
    /// 超时时间（毫秒）
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
    /// 工具标签
    #[serde(default)]
    pub tags: Vec<String>,
    /// 元数据
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

fn default_entry_function() -> String {
    "execute".to_string()
}

fn default_true() -> bool {
    true
}

fn default_timeout() -> u64 {
    30000
}

impl ScriptToolDefinition {
    pub fn new(id: &str, name: &str, script: &str) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            description: String::new(),
            parameters: Vec::new(),
            script: script.to_string(),
            entry_function: "execute".to_string(),
            enable_cache: true,
            timeout_ms: 30000,
            tags: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    pub fn with_description(mut self, desc: &str) -> Self {
        self.description = desc.to_string();
        self
    }

    pub fn with_parameter(mut self, param: ToolParameter) -> Self {
        self.parameters.push(param);
        self
    }

    pub fn with_entry(mut self, function: &str) -> Self {
        self.entry_function = function.to_string();
        self
    }

    pub fn with_tag(mut self, tag: &str) -> Self {
        self.tags.push(tag.to_string());
        self
    }

    /// 验证输入参数
    pub fn validate_input(&self, input: &HashMap<String, serde_json::Value>) -> Result<()> {
        for param in &self.parameters {
            if let Some(value) = input.get(&param.name) {
                param.validate(value)?;
            } else if param.required && param.default.is_none() {
                return Err(anyhow!("Required parameter '{}' is missing", param.name));
            }
        }
        Ok(())
    }

    /// 获取带默认值的输入
    pub fn apply_defaults(&self, input: &mut HashMap<String, serde_json::Value>) {
        for param in &self.parameters {
            if !input.contains_key(&param.name)
                && let Some(ref default) = param.default
            {
                input.insert(param.name.clone(), default.clone());
            }
        }
    }

    /// 生成 JSON Schema 格式的参数描述
    pub fn to_json_schema(&self) -> serde_json::Value {
        let mut properties = serde_json::Map::new();
        let mut required = Vec::new();

        for param in &self.parameters {
            let mut prop = serde_json::Map::new();

            let type_str = match param.param_type {
                ParameterType::String => "string",
                ParameterType::Integer => "integer",
                ParameterType::Float => "number",
                ParameterType::Boolean => "boolean",
                ParameterType::Array => "array",
                ParameterType::Object => "object",
                ParameterType::Any => "any",
            };

            prop.insert("type".to_string(), serde_json::json!(type_str));

            if !param.description.is_empty() {
                prop.insert(
                    "description".to_string(),
                    serde_json::json!(param.description),
                );
            }

            if let Some(ref enum_values) = param.enum_values {
                prop.insert("enum".to_string(), serde_json::json!(enum_values));
            }

            if let Some(min) = param.minimum {
                prop.insert("minimum".to_string(), serde_json::json!(min));
            }

            if let Some(max) = param.maximum {
                prop.insert("maximum".to_string(), serde_json::json!(max));
            }

            properties.insert(param.name.clone(), serde_json::Value::Object(prop));

            if param.required {
                required.push(param.name.clone());
            }
        }

        serde_json::json!({
            "type": "object",
            "properties": properties,
            "required": required
        })
    }
}

// ============================================================================
// 工具执行结果
// ============================================================================

/// 工具执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExecutionResult {
    /// 工具 ID
    pub tool_id: String,
    /// 是否成功
    pub success: bool,
    /// 返回值
    pub result: serde_json::Value,
    /// 错误信息
    pub error: Option<String>,
    /// 执行时间（毫秒）
    pub execution_time_ms: u64,
    /// 执行日志
    pub logs: Vec<String>,
}

// ============================================================================
// 脚本工具注册表
// ============================================================================

/// 脚本工具注册表
pub struct ScriptToolRegistry {
    /// 脚本引擎
    engine: Arc<RhaiScriptEngine>,
    /// 已注册的工具
    tools: Arc<RwLock<HashMap<String, ScriptToolDefinition>>>,
}

impl ScriptToolRegistry {
    /// 创建工具注册表
    pub fn new(engine_config: ScriptEngineConfig) -> Result<Self> {
        let engine = Arc::new(RhaiScriptEngine::new(engine_config)?);
        Ok(Self {
            engine,
            tools: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// 使用已有引擎创建注册表
    pub fn with_engine(engine: Arc<RhaiScriptEngine>) -> Self {
        Self {
            engine,
            tools: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 注册工具
    pub async fn register(&self, tool: ScriptToolDefinition) -> Result<()> {
        // 预编译脚本（如果启用缓存）
        if tool.enable_cache {
            let script_id = format!("tool_{}", tool.id);
            self.engine
                .compile_and_cache(&script_id, &tool.name, &tool.script)
                .await?;
        }

        // 注册到工具表
        let mut tools = self.tools.write().await;
        info!("Registered script tool: {} ({})", tool.name, tool.id);
        tools.insert(tool.id.clone(), tool);

        Ok(())
    }

    /// 批量注册工具
    pub async fn register_batch(&self, tools: Vec<ScriptToolDefinition>) -> Result<Vec<String>> {
        let mut registered = Vec::new();
        for tool in tools {
            let id = tool.id.clone();
            self.register(tool).await?;
            registered.push(id);
        }
        Ok(registered)
    }

    /// 从 YAML 文件加载工具
    pub async fn load_from_yaml(&self, path: &str) -> Result<String> {
        let content = tokio::fs::read_to_string(path).await?;
        let tool: ScriptToolDefinition = serde_yaml::from_str(&content)?;
        let id = tool.id.clone();
        self.register(tool).await?;
        Ok(id)
    }

    /// 从 JSON 文件加载工具
    pub async fn load_from_json(&self, path: &str) -> Result<String> {
        let content = tokio::fs::read_to_string(path).await?;
        let tool: ScriptToolDefinition = serde_json::from_str(&content)?;
        let id = tool.id.clone();
        self.register(tool).await?;
        Ok(id)
    }

    /// 从目录加载所有工具
    pub async fn load_from_directory(&self, dir_path: &str) -> Result<Vec<String>> {
        let mut loaded = Vec::new();
        let mut entries = tokio::fs::read_dir(dir_path).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if let Some(ext) = path.extension() {
                let id = match ext.to_str() {
                    Some("yaml") | Some("yml") => {
                        self.load_from_yaml(path.to_str().unwrap()).await.ok()
                    }
                    Some("json") => self.load_from_json(path.to_str().unwrap()).await.ok(),
                    _ => None,
                };
                if let Some(id) = id {
                    loaded.push(id);
                }
            }
        }

        info!("Loaded {} tools from directory: {}", loaded.len(), dir_path);
        Ok(loaded)
    }

    /// 执行工具
    pub async fn execute(
        &self,
        tool_id: &str,
        input: HashMap<String, serde_json::Value>,
    ) -> Result<ToolExecutionResult> {
        let start_time = std::time::Instant::now();

        // 获取工具定义
        let tools = self.tools.read().await;
        let tool = tools
            .get(tool_id)
            .ok_or_else(|| anyhow!("Tool not found: {}", tool_id))?
            .clone();
        drop(tools);

        // 准备输入
        let mut params = input;
        tool.apply_defaults(&mut params);

        // 验证输入
        tool.validate_input(&params)?;

        // 准备上下文
        let mut context = ScriptContext::new();
        for (key, value) in &params {
            context.set_variable(key, value.clone())?;
        }

        // 将所有参数作为一个 object 传入
        context.set_variable("params", serde_json::json!(params))?;

        // 执行脚本
        let script_id = format!("tool_{}", tool_id);

        if tool.enable_cache {
            // 尝试调用入口函数
            let input_value = serde_json::json!(params);
            match self
                .engine
                .call_function::<serde_json::Value>(
                    &script_id,
                    &tool.entry_function,
                    vec![input_value],
                    &context,
                )
                .await
            {
                Ok(value) => Ok(ToolExecutionResult {
                    tool_id: tool_id.to_string(),
                    success: true,
                    result: value,
                    error: None,
                    execution_time_ms: start_time.elapsed().as_millis() as u64,
                    logs: Vec::new(),
                }),
                Err(_e) => {
                    // 如果函数调用失败，尝试直接执行
                    let script_result = self.engine.execute_compiled(&script_id, &context).await?;
                    if script_result.success {
                        Ok(ToolExecutionResult {
                            tool_id: tool_id.to_string(),
                            success: true,
                            result: script_result.value,
                            error: None,
                            execution_time_ms: start_time.elapsed().as_millis() as u64,
                            logs: script_result.logs,
                        })
                    } else {
                        Ok(ToolExecutionResult {
                            tool_id: tool_id.to_string(),
                            success: false,
                            result: serde_json::Value::Null,
                            error: script_result.error,
                            execution_time_ms: start_time.elapsed().as_millis() as u64,
                            logs: script_result.logs,
                        })
                    }
                }
            }
        } else {
            let script_result = self.engine.execute(&tool.script, &context).await?;
            Ok(ToolExecutionResult {
                tool_id: tool_id.to_string(),
                success: script_result.success,
                result: script_result.value,
                error: script_result.error,
                execution_time_ms: start_time.elapsed().as_millis() as u64,
                logs: script_result.logs,
            })
        }
    }

    /// 获取工具定义
    pub async fn get_tool(&self, tool_id: &str) -> Option<ScriptToolDefinition> {
        let tools = self.tools.read().await;
        tools.get(tool_id).cloned()
    }

    /// 列出所有工具
    pub async fn list_tools(&self) -> Vec<ScriptToolDefinition> {
        let tools = self.tools.read().await;
        tools.values().cloned().collect()
    }

    /// 按标签过滤工具
    pub async fn list_tools_by_tag(&self, tag: &str) -> Vec<ScriptToolDefinition> {
        let tools = self.tools.read().await;
        tools
            .values()
            .filter(|t| t.tags.contains(&tag.to_string()))
            .cloned()
            .collect()
    }

    /// 移除工具
    pub async fn unregister(&self, tool_id: &str) -> bool {
        let mut tools = self.tools.write().await;
        let removed = tools.remove(tool_id).is_some();

        if removed {
            // 清除缓存的脚本
            let script_id = format!("tool_{}", tool_id);
            self.engine.remove_cached(&script_id).await;
            info!("Unregistered script tool: {}", tool_id);
        }

        removed
    }

    /// 清空所有工具
    pub async fn clear(&self) {
        let mut tools = self.tools.write().await;
        tools.clear();
        self.engine.clear_cache().await;
    }

    /// 获取工具数量
    pub async fn tool_count(&self) -> usize {
        let tools = self.tools.read().await;
        tools.len()
    }

    /// 生成所有工具的 JSON Schema 描述（用于 LLM function calling）
    pub async fn generate_tool_schemas(&self) -> Vec<serde_json::Value> {
        let tools = self.tools.read().await;
        tools
            .values()
            .map(|tool| {
                serde_json::json!({
                    "name": tool.name,
                    "description": tool.description,
                    "parameters": tool.to_json_schema()
                })
            })
            .collect()
    }
}

// ============================================================================
// 便捷构建器
// ============================================================================

/// 工具定义构建器
pub struct ToolBuilder {
    definition: ScriptToolDefinition,
}

impl ToolBuilder {
    pub fn new(id: &str, name: &str) -> Self {
        Self {
            definition: ScriptToolDefinition::new(id, name, ""),
        }
    }

    pub fn description(mut self, desc: &str) -> Self {
        self.definition.description = desc.to_string();
        self
    }

    pub fn script(mut self, script: &str) -> Self {
        self.definition.script = script.to_string();
        self
    }

    pub fn entry(mut self, function: &str) -> Self {
        self.definition.entry_function = function.to_string();
        self
    }

    pub fn param(mut self, param: ToolParameter) -> Self {
        self.definition.parameters.push(param);
        self
    }

    pub fn string_param(self, name: &str, required: bool) -> Self {
        let mut param = ToolParameter::new(name, ParameterType::String);
        if required {
            param = param.required();
        }
        self.param(param)
    }

    pub fn int_param(self, name: &str, required: bool) -> Self {
        let mut param = ToolParameter::new(name, ParameterType::Integer);
        if required {
            param = param.required();
        }
        self.param(param)
    }

    pub fn bool_param(self, name: &str, required: bool) -> Self {
        let mut param = ToolParameter::new(name, ParameterType::Boolean);
        if required {
            param = param.required();
        }
        self.param(param)
    }

    pub fn tag(mut self, tag: &str) -> Self {
        self.definition.tags.push(tag.to_string());
        self
    }

    pub fn timeout(mut self, timeout_ms: u64) -> Self {
        self.definition.timeout_ms = timeout_ms;
        self
    }

    pub fn build(self) -> ScriptToolDefinition {
        self.definition
    }
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_tool_registration() {
        let registry = ScriptToolRegistry::new(ScriptEngineConfig::default()).unwrap();

        let tool = ToolBuilder::new("add", "Add Numbers")
            .description("Adds two numbers together")
            .string_param("a", true)
            .string_param("b", true)
            .script(
                r#"
                fn execute(params) {
                    let a = params.a.parse_int();
                    let b = params.b.parse_int();
                    #{
                        result: a + b,
                        operation: "addition"
                    }
                }
            "#,
            )
            .build();

        registry.register(tool).await.unwrap();

        assert_eq!(registry.tool_count().await, 1);
    }

    #[tokio::test]
    async fn test_tool_execution() {
        let registry = ScriptToolRegistry::new(ScriptEngineConfig::default()).unwrap();

        let tool = ScriptToolDefinition::new(
            "multiply",
            "Multiply",
            r#"
                let result = params.x * params.y;
                result
            "#,
        )
        .with_parameter(ToolParameter::new("x", ParameterType::Integer).required())
        .with_parameter(ToolParameter::new("y", ParameterType::Integer).required());

        registry.register(tool).await.unwrap();

        let mut input = HashMap::new();
        input.insert("x".to_string(), serde_json::json!(6));
        input.insert("y".to_string(), serde_json::json!(7));

        let result = registry.execute("multiply", input).await.unwrap();

        assert!(result.success);
        assert_eq!(result.result, serde_json::json!(42));
    }

    #[tokio::test]
    async fn test_parameter_validation() {
        let param = ToolParameter::new("age", ParameterType::Integer)
            .required()
            .with_range(0.0, 150.0);

        // 有效值
        assert!(param.validate(&serde_json::json!(25)).is_ok());

        // 超出范围
        assert!(param.validate(&serde_json::json!(200)).is_err());

        // 错误类型
        assert!(param.validate(&serde_json::json!("not a number")).is_err());
    }

    #[tokio::test]
    async fn test_tool_with_defaults() {
        let registry = ScriptToolRegistry::new(ScriptEngineConfig::default()).unwrap();

        let tool = ScriptToolDefinition::new(
            "greet",
            "Greeting",
            r#"
                let name = params.name;
                let greeting = params.greeting;
                greeting + ", " + name + "!"
            "#,
        )
        .with_parameter(ToolParameter::new("name", ParameterType::String).required())
        .with_parameter(
            ToolParameter::new("greeting", ParameterType::String).with_default("Hello"),
        );

        registry.register(tool).await.unwrap();

        // 不提供 greeting 参数，使用默认值
        let mut input = HashMap::new();
        input.insert("name".to_string(), serde_json::json!("World"));

        let result = registry.execute("greet", input).await.unwrap();

        assert!(result.success);
        assert_eq!(result.result, serde_json::json!("Hello, World!"));
    }

    #[tokio::test]
    async fn test_tool_json_schema() {
        let tool = ToolBuilder::new("search", "Search")
            .description("Search for items")
            .param(
                ToolParameter::new("query", ParameterType::String)
                    .required()
                    .with_description("Search query"),
            )
            .param(
                ToolParameter::new("limit", ParameterType::Integer)
                    .with_default(10)
                    .with_range(1.0, 100.0),
            )
            .param(
                ToolParameter::new("sort", ParameterType::String).with_enum(vec![
                    serde_json::json!("relevance"),
                    serde_json::json!("date"),
                    serde_json::json!("name"),
                ]),
            )
            .script("")
            .build();

        let schema = tool.to_json_schema();

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["query"].is_object());
        assert_eq!(schema["required"], serde_json::json!(["query"]));
    }

    #[test]
    fn test_tool_builder() {
        let tool = ToolBuilder::new("test", "Test Tool")
            .description("A test tool")
            .string_param("input", true)
            .int_param("count", false)
            .bool_param("verbose", false)
            .tag("test")
            .tag("example")
            .timeout(5000)
            .script("input")
            .build();

        assert_eq!(tool.id, "test");
        assert_eq!(tool.parameters.len(), 3);
        assert_eq!(tool.tags.len(), 2);
        assert_eq!(tool.timeout_ms, 5000);
    }
}
