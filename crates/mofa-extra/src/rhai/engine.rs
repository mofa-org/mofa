//! Rhai 脚本引擎核心实现
//!
//! 提供安全的、可扩展的脚本执行环境

use anyhow::{anyhow, Result};
use rhai::{Dynamic, Engine, Map, Scope, AST};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

// ============================================================================
// 脚本引擎配置
// ============================================================================

/// 脚本引擎安全配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptSecurityConfig {
    /// 最大执行时间（毫秒）
    pub max_execution_time_ms: u64,
    /// 最大调用栈深度
    pub max_call_stack_depth: usize,
    /// 最大运算次数
    pub max_operations: u64,
    /// 最大数组大小
    pub max_array_size: usize,
    /// 最大字符串长度
    pub max_string_size: usize,
    /// 是否允许循环
    pub allow_loops: bool,
    /// 是否允许文件操作
    pub allow_file_operations: bool,
    /// 是否允许网络操作
    pub allow_network_operations: bool,
}

impl Default for ScriptSecurityConfig {
    fn default() -> Self {
        Self {
            max_execution_time_ms: 5000,
            max_call_stack_depth: 64,
            max_operations: 100_000,
            max_array_size: 10_000,
            max_string_size: 1_000_000,
            allow_loops: true,
            allow_file_operations: false,
            allow_network_operations: false,
        }
    }
}

/// 脚本引擎配置
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScriptEngineConfig {
    /// 安全配置
    pub security: ScriptSecurityConfig,
    /// 脚本目录
    pub script_dirs: Vec<String>,
    /// 是否启用调试
    pub debug_mode: bool,
    /// 是否启用严格模式
    pub strict_mode: bool,
    /// 预加载模块列表
    pub preload_modules: Vec<String>,
}

// ============================================================================
// 脚本上下文
// ============================================================================

/// 脚本执行上下文
#[derive(Debug, Clone, Default)]
pub struct ScriptContext {
    /// 上下文变量
    pub variables: HashMap<String, serde_json::Value>,
    /// Agent ID
    pub agent_id: Option<String>,
    /// 工作流 ID
    pub workflow_id: Option<String>,
    /// 节点 ID
    pub node_id: Option<String>,
    /// 执行 ID
    pub execution_id: Option<String>,
    /// 自定义元数据
    pub metadata: HashMap<String, String>,
}

impl ScriptContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_agent(mut self, agent_id: &str) -> Self {
        self.agent_id = Some(agent_id.to_string());
        self
    }

    pub fn with_workflow(mut self, workflow_id: &str) -> Self {
        self.workflow_id = Some(workflow_id.to_string());
        self
    }

    pub fn with_node(mut self, node_id: &str) -> Self {
        self.node_id = Some(node_id.to_string());
        self
    }

    pub fn with_variable<T: Serialize>(mut self, key: &str, value: T) -> Result<Self> {
        let json_value = serde_json::to_value(value)?;
        self.variables.insert(key.to_string(), json_value);
        Ok(self)
    }

    pub fn set_variable<T: Serialize>(&mut self, key: &str, value: T) -> Result<()> {
        let json_value = serde_json::to_value(value)?;
        self.variables.insert(key.to_string(), json_value);
        Ok(())
    }

    pub fn get_variable<T: for<'de> Deserialize<'de>>(&self, key: &str) -> Option<T> {
        self.variables
            .get(key)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
    }
}

// ============================================================================
// 脚本执行结果
// ============================================================================

/// 脚本执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptResult {
    /// 是否成功
    pub success: bool,
    /// 返回值
    pub value: serde_json::Value,
    /// 错误信息
    pub error: Option<String>,
    /// 执行时间（毫秒）
    pub execution_time_ms: u64,
    /// 运算次数
    pub operations_count: u64,
    /// 日志输出
    pub logs: Vec<String>,
}

impl ScriptResult {
    pub fn success(value: serde_json::Value, execution_time_ms: u64) -> Self {
        Self {
            success: true,
            value,
            error: None,
            execution_time_ms,
            operations_count: 0,
            logs: Vec::new(),
        }
    }

    pub fn failure(error: String) -> Self {
        Self {
            success: false,
            value: serde_json::Value::Null,
            error: Some(error),
            execution_time_ms: 0,
            operations_count: 0,
            logs: Vec::new(),
        }
    }

    /// 转换为指定类型
    pub fn into_typed<T: for<'de> Deserialize<'de>>(self) -> Result<T> {
        if !self.success {
            return Err(anyhow!(self.error.unwrap_or_else(|| "Unknown error".into())));
        }
        serde_json::from_value(self.value).map_err(|e| anyhow!("Failed to deserialize: {}", e))
    }

    /// 获取布尔值
    pub fn as_bool(&self) -> Option<bool> {
        self.value.as_bool()
    }

    /// 获取字符串
    pub fn as_str(&self) -> Option<&str> {
        self.value.as_str()
    }

    /// 获取整数
    pub fn as_i64(&self) -> Option<i64> {
        self.value.as_i64()
    }

    /// 获取浮点数
    pub fn as_f64(&self) -> Option<f64> {
        self.value.as_f64()
    }
}

// ============================================================================
// 已编译脚本
// ============================================================================

/// 已编译的脚本
pub struct CompiledScript {
    /// 脚本 ID
    pub id: String,
    /// 脚本名称
    pub name: String,
    /// 编译后的 AST
    ast: AST,
    /// 源代码（用于调试）
    source: String,
    /// 编译时间戳
    pub compiled_at: u64,
}

impl CompiledScript {
    pub fn new(id: &str, name: &str, ast: AST, source: String) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            ast,
            source,
            compiled_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }

    pub fn source(&self) -> &str {
        &self.source
    }
}

// ============================================================================
// Rhai 脚本引擎
// ============================================================================

/// MoFA Rhai 脚本引擎
pub struct RhaiScriptEngine {
    /// Rhai 引擎实例
    engine: Engine,
    /// 引擎配置
    #[allow(dead_code)]
    config: ScriptEngineConfig,
    /// 已编译脚本缓存
    script_cache: Arc<RwLock<HashMap<String, CompiledScript>>>,
    /// 全局作用域（预定义函数和变量）
    global_scope: Scope<'static>,
    /// 日志收集器
    logs: Arc<RwLock<Vec<String>>>,
}

impl RhaiScriptEngine {
    /// 创建新的脚本引擎
    pub fn new(config: ScriptEngineConfig) -> Result<Self> {
        let mut engine = Engine::new();

        // 应用安全限制
        Self::apply_security_limits(&mut engine, &config.security);

        // 注册内置函数
        let logs = Arc::new(RwLock::new(Vec::new()));
        Self::register_builtin_functions(&mut engine, logs.clone());

        // 创建全局作用域
        let global_scope = Scope::new();

        Ok(Self {
            engine,
            config,
            script_cache: Arc::new(RwLock::new(HashMap::new())),
            global_scope,
            logs,
        })
    }

    /// 应用安全限制
    fn apply_security_limits(engine: &mut Engine, security: &ScriptSecurityConfig) {
        engine.set_max_call_levels(security.max_call_stack_depth);
        engine.set_max_operations(security.max_operations);
        engine.set_max_array_size(security.max_array_size);
        engine.set_max_string_size(security.max_string_size);

        if !security.allow_loops {
            engine.set_allow_looping(false);
        }

        // 禁用严格模式，以便在运行时可以使用上下文变量
        engine.set_strict_variables(false);
    }

    /// 注册内置函数
    fn register_builtin_functions(engine: &mut Engine, logs: Arc<RwLock<Vec<String>>>) {
        // 日志函数
        let logs_clone = logs.clone();
        engine.register_fn("log", move |msg: &str| {
            if let Ok(mut l) = logs_clone.try_write() {
                l.push(format!("[LOG] {}", msg));
            }
        });

        let logs_clone = logs.clone();
        engine.register_fn("debug", move |msg: &str| {
            if let Ok(mut l) = logs_clone.try_write() {
                l.push(format!("[DEBUG] {}", msg));
            }
            debug!("Script debug: {}", msg);
        });

        let logs_clone = logs.clone();
        engine.register_fn("warn", move |msg: &str| {
            if let Ok(mut l) = logs_clone.try_write() {
                l.push(format!("[WARN] {}", msg));
            }
            warn!("Script warn: {}", msg);
        });

        let logs_clone = logs.clone();
        engine.register_fn("error", move |msg: &str| {
            if let Ok(mut l) = logs_clone.try_write() {
                l.push(format!("[ERROR] {}", msg));
            }
            error!("Script error: {}", msg);
        });

        // JSON 操作函数
        engine.register_fn("to_json", |value: Dynamic| -> String {
            serde_json::to_string(&value).unwrap_or_else(|_| "null".to_string())
        });

        engine.register_fn("from_json", |json: &str| -> Dynamic {
            serde_json::from_str::<serde_json::Value>(json)
                .map(|v| json_to_dynamic(&v))
                .unwrap_or(Dynamic::UNIT)
        });

        // 字符串操作
        engine.register_fn("trim", |s: &str| -> String { s.trim().to_string() });

        engine.register_fn("upper", |s: &str| -> String { s.to_uppercase() });

        engine.register_fn("lower", |s: &str| -> String { s.to_lowercase() });

        engine.register_fn("contains", |s: &str, pattern: &str| -> bool {
            s.contains(pattern)
        });

        engine.register_fn("starts_with", |s: &str, pattern: &str| -> bool {
            s.starts_with(pattern)
        });

        engine.register_fn("ends_with", |s: &str, pattern: &str| -> bool {
            s.ends_with(pattern)
        });

        engine.register_fn("replace", |s: &str, from: &str, to: &str| -> String {
            s.replace(from, to)
        });

        engine.register_fn("split", |s: &str, delimiter: &str| -> Vec<Dynamic> {
            s.split(delimiter)
                .map(|part| Dynamic::from(part.to_string()))
                .collect()
        });

        // 数学函数
        engine.register_fn("abs", |x: i64| -> i64 { x.abs() });
        engine.register_fn("abs_f", |x: f64| -> f64 { x.abs() });
        engine.register_fn("min", |a: i64, b: i64| -> i64 { a.min(b) });
        engine.register_fn("max", |a: i64, b: i64| -> i64 { a.max(b) });
        engine.register_fn("clamp", |value: i64, min: i64, max: i64| -> i64 {
            value.clamp(min, max)
        });

        // 时间函数
        engine.register_fn("now", || -> i64 {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64
        });

        engine.register_fn("now_ms", || -> i64 {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64
        });

        // UUID 生成
        engine.register_fn("uuid", || -> String { uuid::Uuid::now_v7().to_string() });

        // 类型检查
        engine.register_fn("is_null", |v: Dynamic| -> bool { v.is_unit() });
        engine.register_fn("is_string", |v: Dynamic| -> bool { v.is_string() });
        engine.register_fn("is_int", |v: Dynamic| -> bool { v.is_int() });
        engine.register_fn("is_float", |v: Dynamic| -> bool { v.is_float() });
        engine.register_fn("is_bool", |v: Dynamic| -> bool { v.is_bool() });
        engine.register_fn("is_array", |v: Dynamic| -> bool { v.is_array() });
        engine.register_fn("is_map", |v: Dynamic| -> bool { v.is_map() });

        // 类型转换
        engine.register_fn("to_string", |v: i64| -> String { v.to_string() });
        engine.register_fn("to_string", |v: f64| -> String { v.to_string() });
        engine.register_fn("to_string", |v: bool| -> String { v.to_string() });
        engine.register_fn("to_string", |v: &str| -> String { v.to_string() });
    }

    /// 编译脚本
    pub fn compile(&self, id: &str, name: &str, source: &str) -> Result<CompiledScript> {
        let ast = self
            .engine
            .compile(source)
            .map_err(|e| anyhow!("Compile error: {}", e))?;

        Ok(CompiledScript::new(id, name, ast, source.to_string()))
    }

    /// 编译并缓存脚本
    pub async fn compile_and_cache(&self, id: &str, name: &str, source: &str) -> Result<()> {
        let compiled = self.compile(id, name, source)?;
        let mut cache = self.script_cache.write().await;
        cache.insert(id.to_string(), compiled);
        info!("Script compiled and cached: {} ({})", name, id);
        Ok(())
    }

    /// 从文件加载脚本
    pub async fn load_from_file(&self, path: &Path) -> Result<String> {
        let source = tokio::fs::read_to_string(path).await?;
        let id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unnamed");
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("unnamed");

        self.compile_and_cache(id, name, &source).await?;
        Ok(id.to_string())
    }

    /// 执行脚本
    pub async fn execute(&self, source: &str, context: &ScriptContext) -> Result<ScriptResult> {
        let start_time = std::time::Instant::now();

        // 清空日志
        {
            let mut logs = self.logs.write().await;
            logs.clear();
        }

        // 准备作用域
        let mut scope = self.global_scope.clone();
        self.prepare_scope(&mut scope, context);

        // 执行脚本
        let result = self.engine.eval_with_scope::<Dynamic>(&mut scope, source);

        let execution_time_ms = start_time.elapsed().as_millis() as u64;
        let logs = self.logs.read().await.clone();

        match result {
            Ok(value) => {
                let json_value = dynamic_to_json(&value);
                Ok(ScriptResult {
                    success: true,
                    value: json_value,
                    error: None,
                    execution_time_ms,
                    operations_count: 0,
                    logs,
                })
            }
            Err(e) => Ok(ScriptResult {
                success: false,
                value: serde_json::Value::Null,
                error: Some(format!("{}", e)),
                execution_time_ms,
                operations_count: 0,
                logs,
            }),
        }
    }

    /// 执行已编译的脚本
    pub async fn execute_compiled(
        &self,
        script_id: &str,
        context: &ScriptContext,
    ) -> Result<ScriptResult> {
        let cache = self.script_cache.read().await;
        let compiled = cache
            .get(script_id)
            .ok_or_else(|| anyhow!("Script not found: {}", script_id))?;

        let start_time = std::time::Instant::now();

        // 清空日志
        {
            let mut logs = self.logs.write().await;
            logs.clear();
        }

        // 准备作用域
        let mut scope = self.global_scope.clone();
        self.prepare_scope(&mut scope, context);

        // 执行已编译的 AST
        let result = self.engine.eval_ast_with_scope::<Dynamic>(&mut scope, &compiled.ast);

        let execution_time_ms = start_time.elapsed().as_millis() as u64;
        let logs = self.logs.read().await.clone();

        match result {
            Ok(value) => {
                let json_value = dynamic_to_json(&value);
                Ok(ScriptResult {
                    success: true,
                    value: json_value,
                    error: None,
                    execution_time_ms,
                    operations_count: 0,
                    logs,
                })
            }
            Err(e) => Ok(ScriptResult {
                success: false,
                value: serde_json::Value::Null,
                error: Some(format!("{}", e)),
                execution_time_ms,
                operations_count: 0,
                logs,
            }),
        }
    }

    /// 调用脚本函数
    pub async fn call_function<T: for<'de> Deserialize<'de>>(
        &self,
        script_id: &str,
        function_name: &str,
        args: Vec<serde_json::Value>,
        context: &ScriptContext,
    ) -> Result<T> {
        let cache = self.script_cache.read().await;
        let compiled = cache
            .get(script_id)
            .ok_or_else(|| anyhow!("Script not found: {}", script_id))?;

        // 准备作用域
        let mut scope = self.global_scope.clone();
        self.prepare_scope(&mut scope, context);

        // 转换参数
        let dynamic_args: Vec<Dynamic> = args.iter().map(json_to_dynamic).collect();

        // 调用函数
        let result: Dynamic = self
            .engine
            .call_fn(&mut scope, &compiled.ast, function_name, dynamic_args)
            .map_err(|e| anyhow!("Function call error: {}", e))?;

        // 转换结果
        let json_value = dynamic_to_json(&result);
        serde_json::from_value(json_value).map_err(|e| anyhow!("Result conversion error: {}", e))
    }

    /// 准备执行作用域
    fn prepare_scope(&self, scope: &mut Scope, context: &ScriptContext) {
        // 添加上下文信息
        if let Some(ref agent_id) = context.agent_id {
            scope.push_constant("AGENT_ID", agent_id.clone());
        }
        if let Some(ref workflow_id) = context.workflow_id {
            scope.push_constant("WORKFLOW_ID", workflow_id.clone());
        }
        if let Some(ref node_id) = context.node_id {
            scope.push_constant("NODE_ID", node_id.clone());
        }
        if let Some(ref execution_id) = context.execution_id {
            scope.push_constant("EXECUTION_ID", execution_id.clone());
        }

        // 添加上下文变量
        for (key, value) in &context.variables {
            let dynamic_value = json_to_dynamic(value);
            scope.push(key.clone(), dynamic_value);
        }

        // 添加元数据
        let mut metadata_map = Map::new();
        for (k, v) in &context.metadata {
            metadata_map.insert(k.clone().into(), Dynamic::from(v.clone()));
        }
        scope.push_constant("metadata", metadata_map);
    }

    /// 验证脚本语法
    pub fn validate(&self, source: &str) -> Result<Vec<String>> {
        match self.engine.compile(source) {
            Ok(_) => Ok(Vec::new()),
            Err(e) => {
                let errors = vec![format!("{}", e)];
                Ok(errors)
            }
        }
    }

    /// 获取缓存的脚本 ID 列表
    pub async fn cached_scripts(&self) -> Vec<String> {
        let cache = self.script_cache.read().await;
        cache.keys().cloned().collect()
    }

    /// 移除缓存的脚本
    pub async fn remove_cached(&self, script_id: &str) -> bool {
        let mut cache = self.script_cache.write().await;
        cache.remove(script_id).is_some()
    }

    /// 清空脚本缓存
    pub async fn clear_cache(&self) {
        let mut cache = self.script_cache.write().await;
        cache.clear();
    }

    /// 获取引擎引用（用于高级自定义）
    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    /// 获取可变引擎引用
    pub fn engine_mut(&mut self) -> &mut Engine {
        &mut self.engine
    }

}


// ============================================================================
// 辅助函数
// ============================================================================

/// JSON Value 转换为 Rhai Dynamic
pub fn json_to_dynamic(value: &serde_json::Value) -> Dynamic {
    match value {
        serde_json::Value::Null => Dynamic::UNIT,
        serde_json::Value::Bool(b) => Dynamic::from(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Dynamic::from(i)
            } else if let Some(f) = n.as_f64() {
                Dynamic::from(f)
            } else {
                Dynamic::UNIT
            }
        }
        serde_json::Value::String(s) => Dynamic::from(s.clone()),
        serde_json::Value::Array(arr) => {
            let vec: Vec<Dynamic> = arr.iter().map(json_to_dynamic).collect();
            Dynamic::from(vec)
        }
        serde_json::Value::Object(obj) => {
            let mut map = Map::new();
            for (k, v) in obj {
                map.insert(k.clone().into(), json_to_dynamic(v));
            }
            Dynamic::from(map)
        }
    }
}

/// Rhai Dynamic 转换为 JSON Value
pub fn dynamic_to_json(value: &Dynamic) -> serde_json::Value {
    if value.is_unit() {
        serde_json::Value::Null
    } else if let Some(b) = value.clone().try_cast::<bool>() {
        serde_json::Value::Bool(b)
    } else if let Some(i) = value.clone().try_cast::<i64>() {
        serde_json::json!(i)
    } else if let Some(f) = value.clone().try_cast::<f64>() {
        serde_json::json!(f)
    } else if let Some(s) = value.clone().try_cast::<String>() {
        serde_json::Value::String(s)
    } else if value.is_array() {
        let arr = value.clone().cast::<rhai::Array>();
        let json_arr: Vec<serde_json::Value> = arr.iter().map(dynamic_to_json).collect();
        serde_json::Value::Array(json_arr)
    } else if value.is_map() {
        let map = value.clone().cast::<Map>();
        let mut json_obj = serde_json::Map::new();
        for (k, v) in map.iter() {
            json_obj.insert(k.to_string(), dynamic_to_json(v));
        }
        serde_json::Value::Object(json_obj)
    } else {
        // 尝试转换为字符串
        serde_json::Value::String(value.to_string())
    }
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_basic_script_execution() {
        let engine = RhaiScriptEngine::new(ScriptEngineConfig::default()).unwrap();
        let context = ScriptContext::new();

        let result = engine.execute("1 + 2", &context).await.unwrap();

        assert!(result.success);
        assert_eq!(result.value, serde_json::json!(3));
    }

    #[tokio::test]
    async fn test_script_with_variables() {
        let engine = RhaiScriptEngine::new(ScriptEngineConfig::default()).unwrap();
        let context = ScriptContext::new()
            .with_variable("x", 10)
            .unwrap()
            .with_variable("y", 20)
            .unwrap();

        let result = engine.execute("x + y", &context).await.unwrap();

        assert!(result.success);
        assert_eq!(result.value, serde_json::json!(30));
    }

    #[tokio::test]
    async fn test_script_with_function() {
        let engine = RhaiScriptEngine::new(ScriptEngineConfig::default()).unwrap();
        let context = ScriptContext::new();

        let script = r#"
            fn double(n) {
                n * 2
            }
            double(21)
        "#;

        let result = engine.execute(script, &context).await.unwrap();

        assert!(result.success);
        assert_eq!(result.value, serde_json::json!(42));
    }

    #[tokio::test]
    async fn test_compiled_script() {
        let engine = RhaiScriptEngine::new(ScriptEngineConfig::default()).unwrap();

        engine
            .compile_and_cache(
                "test_script",
                "Test Script",
                r#"
                fn process(input) {
                    let result = #{};
                    result.doubled = input.value * 2;
                    result.message = "processed: " + input.name;
                    result
                }
                process(input)
            "#,
            )
            .await
            .unwrap();

        let context = ScriptContext::new()
            .with_variable(
                "input",
                serde_json::json!({
                    "name": "test",
                    "value": 21
                }),
            )
            .unwrap();

        let result = engine
            .execute_compiled("test_script", &context)
            .await
            .unwrap();

        assert!(result.success);
        assert_eq!(result.value["doubled"], 42);
        assert_eq!(result.value["message"], "processed: test");
    }

    #[tokio::test]
    async fn test_builtin_functions() {
        let engine = RhaiScriptEngine::new(ScriptEngineConfig::default()).unwrap();
        let context = ScriptContext::new();

        // 测试字符串函数
        let result = engine
            .execute(r#"upper("hello")"#, &context)
            .await
            .unwrap();
        assert_eq!(result.value, "HELLO");

        // 测试 JSON 函数
        let result = engine
            .execute(r#"to_json(#{name: "test", value: 42})"#, &context)
            .await
            .unwrap();
        assert!(result.value.as_str().is_some());

        // 测试时间函数
        let result = engine.execute("now()", &context).await.unwrap();
        assert!(result.value.as_i64().is_some());
    }

    #[test]
    fn test_json_conversion() {
        let json = serde_json::json!({
            "name": "test",
            "values": [1, 2, 3],
            "nested": {
                "flag": true
            }
        });

        let dynamic = json_to_dynamic(&json);
        let back = dynamic_to_json(&dynamic);

        assert_eq!(json, back);
    }
}
