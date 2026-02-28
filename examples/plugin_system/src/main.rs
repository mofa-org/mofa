//! 插件系统示例
//! Plugin system example
//!
//! 演示 MoFA 框架的插件机制，包括：
//! Demonstrate the MoFA framework's plugin mechanism, including:
//! - 插件管理器的使用
//! - Use of the plugin manager
//! - LLM 插件（文本生成、聊天）
//! - LLM plugins (text generation, chat)
//! - 工具插件（自定义工具）
//! - Tool plugins (custom tools)
//! - 存储插件（键值存储）
//! - Storage plugins (key-value storage)
//! - 记忆插件（智能体记忆管理）
//! - Memory plugins (agent memory management)
//! - 自定义插件开发
//! - Custom plugin development

// Rhai scripting
use mofa_sdk::rhai::{RhaiScriptEngine, ScriptContext, ScriptEngineConfig};
use mofa_sdk::kernel::plugin::PluginError;
use mofa_sdk::plugins::PluginPriority;
use mofa_sdk::plugins::{
    AgentPlugin, LLMPlugin, LLMPluginConfig, MemoryPlugin, MemoryStorage, PluginContext,
    PluginManager, PluginMetadata, PluginResult, PluginState, PluginType, StoragePlugin,
    ToolDefinition, ToolExecutor, ToolPlugin,
};
use std::any::Any;
use std::collections::HashMap;
use tracing::{info, warn};

// ============================================================================
// 自定义工具：计算器
// Custom Tool: Calculator
// ============================================================================

struct CalculatorTool {
    definition: ToolDefinition,
}

impl CalculatorTool {
    fn new() -> Self {
        Self {
            definition: ToolDefinition {
                name: "calculator".to_string(),
                description: "Perform basic arithmetic operations".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "operation": {
                            "type": "string",
                            "enum": ["add", "subtract", "multiply", "divide"]
                        },
                        "a": { "type": "number" },
                        "b": { "type": "number" }
                    },
                    "required": ["operation", "a", "b"]
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
        let op = arguments["operation"].as_str().unwrap_or("add");
        let a = arguments["a"].as_f64().unwrap_or(0.0);
        let b = arguments["b"].as_f64().unwrap_or(0.0);

        let result = match op {
            "add" => a + b,
            "subtract" => a - b,
            "multiply" => a * b,
            "divide" => {
                if b == 0.0 {
                    return Err(PluginError::ExecutionFailed("Division by zero".to_string()));
                }
                a / b
            }
            _ => return Err(PluginError::ExecutionFailed(format!("Unknown operation: {}", op))),
        };

        Ok(serde_json::json!({
            "result": result,
            "operation": op,
            "a": a,
            "b": b
        }))
    }
}

// ============================================================================
// 自定义工具：天气查询（模拟）
// Custom Tool: Weather Query (Mock)
// ============================================================================

struct WeatherTool {
    definition: ToolDefinition,
}

impl WeatherTool {
    fn new() -> Self {
        Self {
            definition: ToolDefinition {
                name: "weather".to_string(),
                description: "Get weather information for a location".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "location": { "type": "string" }
                    },
                    "required": ["location"]
                }),
                requires_confirmation: false,
            },
        }
    }
}

#[async_trait::async_trait]
impl ToolExecutor for WeatherTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, arguments: serde_json::Value) -> PluginResult<serde_json::Value> {
        let location = arguments["location"].as_str().unwrap_or("Unknown");

        // 模拟天气数据
        // Simulate weather data
        Ok(serde_json::json!({
            "location": location,
            "temperature": 22,
            "unit": "celsius",
            "condition": "sunny",
            "humidity": 65,
            "wind_speed": 12
        }))
    }
}

// ============================================================================
// 自定义插件：监控插件
// Custom Plugin: Monitor Plugin
// ============================================================================

struct MonitorPlugin {
    metadata: PluginMetadata,
    state: PluginState,
    metrics: HashMap<String, f64>,
    alert_threshold: f64,
}

impl MonitorPlugin {
    fn new(plugin_id: &str) -> Self {
        let metadata = PluginMetadata::new(plugin_id, "Monitor Plugin", PluginType::Monitor)
            .with_description("System monitoring and alerting plugin")
            .with_capability("metrics")
            .with_capability("alerting")
            .with_priority(PluginPriority::High);

        Self {
            metadata,
            state: PluginState::Unloaded,
            metrics: HashMap::new(),
            alert_threshold: 80.0,
        }
    }

    fn record_metric(&mut self, name: &str, value: f64) {
        self.metrics.insert(name.to_string(), value);
        if value > self.alert_threshold {
            warn!("ALERT: Metric {} exceeded threshold: {} > {}", name, value, self.alert_threshold);
        }
    }

    fn get_metric(&self, name: &str) -> Option<f64> {
        self.metrics.get(name).copied()
    }

    fn all_metrics(&self) -> &HashMap<String, f64> {
        &self.metrics
    }
}

#[async_trait::async_trait]
impl AgentPlugin for MonitorPlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    fn state(&self) -> PluginState {
        self.state.clone()
    }

    async fn load(&mut self, _ctx: &PluginContext) -> PluginResult<()> {
        self.state = PluginState::Loading;
        info!("Loading Monitor plugin: {}", self.metadata.id);
        self.state = PluginState::Loaded;
        Ok(())
    }

    async fn init_plugin(&mut self) -> PluginResult<()> {
        info!("Initializing Monitor plugin: {}", self.metadata.id);
        // 初始化基础指标
        // Initialize base metrics
        self.metrics.insert("cpu_usage".to_string(), 0.0);
        self.metrics.insert("memory_usage".to_string(), 0.0);
        self.metrics.insert("request_count".to_string(), 0.0);
        Ok(())
    }

    async fn start(&mut self) -> PluginResult<()> {
        self.state = PluginState::Running;
        info!("Monitor plugin {} started", self.metadata.id);
        Ok(())
    }

    async fn stop(&mut self) -> PluginResult<()> {
        self.state = PluginState::Paused;
        info!("Monitor plugin {} stopped", self.metadata.id);
        Ok(())
    }

    async fn unload(&mut self) -> PluginResult<()> {
        self.metrics.clear();
        self.state = PluginState::Unloaded;
        info!("Monitor plugin {} unloaded", self.metadata.id);
        Ok(())
    }

    async fn execute(&mut self, input: String) -> PluginResult<String> {
        let parts: Vec<&str> = input.as_str().splitn(3, ' ').collect();
        match parts.as_slice() {
            ["record", name, value] => {
                let v: f64 = value.parse().unwrap_or(0.0);
                self.record_metric(name, v);
                Ok(format!("Recorded {} = {}", name, v))
            }
            ["get", name] => {
                match self.get_metric(name) {
                    Some(v) => Ok(format!("{}", v)),
                    None => Ok("null".to_string()),
                }
            }
            ["list"] => {
                Ok(serde_json::to_string(&self.all_metrics())?)
            }
            _ => Err(PluginError::ExecutionFailed("Invalid command. Use: record <name> <value>, get <name>, list".to_string())),
        }
    }

    fn stats(&self) -> HashMap<String, serde_json::Value> {
        let mut stats = HashMap::new();
        stats.insert("metric_count".to_string(), serde_json::json!(self.metrics.len()));
        stats.insert("alert_threshold".to_string(), serde_json::json!(self.alert_threshold));
        stats
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }
}

// ============================================================================
// 主函数
// Main Function
// ============================================================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    info!("=== MoFA Plugin System Demo ===\n");

    // 1. 创建插件管理器
    // 1. Create plugin manager
    let manager = PluginManager::new("demo_agent");
    info!("Created PluginManager for agent: demo_agent");

    // 2. 注册 LLM 插件
    // 2. Register LLM plugin
    info!("\n--- Registering LLM Plugin ---");
    let llm_config = LLMPluginConfig {
        model: "gpt-4".to_string(),
        max_tokens: 4096,
        temperature: 0.7,
        ..Default::default()
    };
    let llm = LLMPlugin::new("llm_main").with_config(llm_config);
    manager.register(llm).await?;
    info!("LLM plugin registered");

    // 3. 注册工具插件并添加工具
    // 3. Register tool plugin and add tools
    info!("\n--- Registering Tool Plugin ---");
    let mut tool_plugin = ToolPlugin::new("tools_main");
    tool_plugin.register_tool(CalculatorTool::new());
    tool_plugin.register_tool(WeatherTool::new());
    manager.register(tool_plugin).await?;
    info!("Tool plugin registered with calculator and weather tools");

    // 4. 注册存储插件
    // 4. Register storage plugin
    info!("\n--- Registering Storage Plugin ---");
    let storage = StoragePlugin::new("storage_main")
        .with_backend(MemoryStorage::new());
    manager.register(storage).await?;
    info!("Storage plugin registered");

    // 5. 注册记忆插件
    // 5. Register memory plugin
    info!("\n--- Registering Memory Plugin ---");
    let memory = MemoryPlugin::new("memory_main").with_max_memories(500);
    manager.register(memory).await?;
    info!("Memory plugin registered");

    // 6. 注册自定义监控插件
    // 6. Register custom monitor plugin
    info!("\n--- Registering Custom Monitor Plugin ---");
    let monitor = MonitorPlugin::new("monitor_main");
    manager.register(monitor).await?;
    info!("Monitor plugin registered");

    // 7. 初始化所有插件
    // 7. Initialize all plugins
    info!("\n--- Initializing All Plugins ---");
    manager.load_all().await?;
    manager.init_all().await?;
    manager.start_all().await?;

    // 8. 列出所有已注册的插件
    // 8. List all registered plugins
    info!("\n--- Registered Plugins ---");
    let plugins = manager.list_plugins().await;
    for p in &plugins {
        info!(
            "  - {} ({}): {} [Priority: {:?}]",
            p.name, p.id, p.description, p.priority
        );
    }

    // 9. 使用 LLM 插件
    // 9. Use LLM plugin
    info!("\n--- Using LLM Plugin ---");
    let llm_response = manager.execute("llm_main", "What is the capital of France?".to_string()).await?;
    info!("LLM Response: {}", llm_response);

    // 10. 使用工具插件
    // 10. Use tool plugin
    info!("\n--- Using Tool Plugin ---");

    // 计算器工具
    // Calculator tool
    let calc_call = serde_json::json!({
        "name": "calculator",
        "arguments": {
            "operation": "multiply",
            "a": 7,
            "b": 8
        },
        "call_id": "calc_001"
    });
    let calc_result = manager.execute("tools_main", calc_call.to_string()).await?;
    info!("Calculator Result: {}", calc_result);

    // 天气工具
    // Weather tool
    let weather_call = serde_json::json!({
        "name": "weather",
        "arguments": {
            "location": "Beijing"
        },
        "call_id": "weather_001"
    });
    let weather_result = manager.execute("tools_main", weather_call.to_string()).await?;
    info!("Weather Result: {}", weather_result);

    // 11. 使用存储插件
    // 11. Use storage plugin
    info!("\n--- Using Storage Plugin ---");
    manager.execute("storage_main", "set user:name Alice".to_string()).await?;
    manager.execute("storage_main", "set user:age 30".to_string()).await?;
    manager.execute("storage_main", "set session:token abc123xyz".to_string()).await?;

    let name = manager.execute("storage_main", "get user:name".to_string()).await?;
    let age = manager.execute("storage_main", "get user:age".to_string()).await?;
    info!("Stored user: name={}, age={}", name, age);

    // 12. 使用记忆插件
    // 12. Use memory plugin
    info!("\n--- Using Memory Plugin ---");
    manager.execute("memory_main", "add User asked about weather in Beijing 0.8".to_string()).await?;
    manager.execute("memory_main", "add User calculated 7 * 8 = 56 0.6".to_string()).await?;
    manager.execute("memory_main", "add Important: User prefers Celsius for temperature 0.9".to_string()).await?;

    let memory_count = manager.execute("memory_main", "count".to_string()).await?;
    info!("Total memories: {}", memory_count);

    let search_result = manager.execute("memory_main", "search weather".to_string()).await?;
    info!("Memory search for 'weather': {}", search_result);

    // 13. 使用监控插件
    // 13. Use monitor plugin
    info!("\n--- Using Monitor Plugin ---");
    manager.execute("monitor_main", "record cpu_usage 45.5".to_string()).await?;
    manager.execute("monitor_main", "record memory_usage 72.3".to_string()).await?;
    manager.execute("monitor_main", "record request_count 1234".to_string()).await?;
    manager.execute("monitor_main", "record error_rate 85.0".to_string()).await?; // 会触发告警
    // This will trigger an alert

    let metrics = manager.execute("monitor_main", "list".to_string()).await?;
    info!("All metrics: {}", metrics);

    // 14. 健康检查
    // 14. Health check
    info!("\n--- Health Check ---");
    let health = manager.health_check_all().await;
    for (id, healthy) in &health {
        info!("  - {}: {}", id, if *healthy { "✓ Healthy" } else { "✗ Unhealthy" });
    }

    // 15. 获取插件统计
    // 15. Get plugin statistics
    info!("\n--- Plugin Statistics ---");
    for p in &plugins {
        if let Some(stats) = manager.stats(&p.id).await {
            info!("  {} stats: {:?}", p.id, stats);
        }
    }

    // 16. 按类型获取插件
    // 16. Get plugins by type
    info!("\n--- Plugins by Type ---");
    let llm_plugins = manager.get_by_type(PluginType::LLM).await;
    info!("LLM plugins: {:?}", llm_plugins);
    let tool_plugins = manager.get_by_type(PluginType::Tool).await;
    info!("Tool plugins: {:?}", tool_plugins);
    let storage_plugins = manager.get_by_type(PluginType::Storage).await;
    info!("Storage plugins: {:?}", storage_plugins);

    // 17. 演示插件上下文共享状态
    // 17. Demonstrate plugin context shared state
    info!("\n--- Plugin Context Shared State ---");
    let ctx = manager.context();
    ctx.set_state("conversation_id", "conv_12345".to_string()).await;
    ctx.set_state("turn_count", 5i32).await;

    let conv_id: Option<String> = ctx.get_state("conversation_id").await;
    let turn_count: Option<i32> = ctx.get_state("turn_count").await;
    info!("Shared state - conversation_id: {:?}, turn_count: {:?}", conv_id, turn_count);

    // 18. 停止和卸载
    // 18. Stop and unload
    info!("\n--- Stopping All Plugins ---");
    manager.stop_all().await?;
    manager.unload_all().await?;

    // 19. Rhai 脚本引擎示例
    // 19. Rhai scripting engine example
    info!("\n--- Rhai Scripting Engine Examples ---\n");

    // 创建脚本引擎
    // Create script engine
    let script_engine = RhaiScriptEngine::new(ScriptEngineConfig::default())?;

    // 编译时脚本：预编译并缓存，提高运行时性能
    // Compile-time script: pre-compiled and cached for better runtime performance
    info!("19.1 编译时脚本执行（预编译缓存）:");
    // 19.1 Compile-time script execution (pre-compiled cache):

    // 编译并缓存脚本
    // Compile and cache script
    script_engine.compile_and_cache(
        "greeting_script",
        "Greeting",
        r#"
            let greeting = "Hello, " + user_name + " from compile-time script!";
            let current_time = now();
            #{
                greeting: greeting,
                time: current_time,
                version: "1.0.0"
            }
        "#,
    ).await?;

    // 创建上下文
    // Create context
    let ctx = ScriptContext::new()
        .with_variable("user_name", "MoFA Plugin System")?;

    // 执行编译后的脚本
    // Execute pre-compiled script
    let result = script_engine.execute_compiled("greeting_script", &ctx).await?;
    info!("  编译时脚本结果: {}", serde_json::to_string_pretty(&result.value)?);
    // Compile-time script result: {}

    // 运行时脚本：动态执行，灵活性高
    // Runtime script: dynamic execution with high flexibility
    info!("\n19.2 运行时脚本执行（动态）:");
    // 19.2 Runtime script execution (dynamic):

    // 运行时动态生成并执行脚本
    // Dynamically generate and execute script at runtime
    let runtime_script = format!(
        r#"
            // 计算插件数量
            // Calculate plugin count
            let plugin_count = {};
            // 动态生成消息
            // Dynamically generate message
            "当前系统共有 " + plugin_count + " 个插件在运行"
        "#,
        plugins.len()
    );

    let result = script_engine.execute(&runtime_script, &ctx).await?;
    info!("  运行时脚本结果: {}", result.value);
    // Runtime script result: {}

    // 19.3 与插件系统结合：使用脚本调用插件
    // 19.3 Integration with plugin system: using script to call plugins
    info!("\n19.3 脚本与插件系统结合:");
    // 19.3 Integration of script and plugin system:

    // 创建一个使用脚本调用计算器工具的示例
    // Create an example of calling calculator tool via script
    let _tool_script = r#"
        // 使用插件系统的计算器工具
        // Use calculator tool from the plugin system
        let calc_result = call_tool("calculator", #{
            operation: "add",
            a: 100,
            b: 200
        });

        calc_result.result
    "#;

    // 创建一个包含工具调用能力的上下文
    // Create a context with tool calling capabilities
    // 注意：需要将工具调用函数注册到脚本引擎中
    // Note: tool calling functions need to be registered in the script engine
    info!("  脚本与插件结合示例已准备，可扩展实现工具调用接口\n");
    // Script and plugin combination example ready, extendable for tool call interfaces

    info!("\n=== Demo Completed ===");
    Ok(())
}
