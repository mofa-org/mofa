use async_trait::async_trait;
use mofa_runtime::agent::capabilities::AgentCapabilities;
use mofa_runtime::agent::context::AgentContext;
use mofa_runtime::agent::error::AgentResult;
use mofa_runtime::agent::execution::{ExecutionEngine, ExecutionOptions};
use mofa_runtime::agent::plugins::{CustomFunctionPlugin, HttpPlugin, PluginStage};
use mofa_runtime::agent::registry::AgentRegistry;
use mofa_runtime::agent::types::{AgentInput, AgentOutput, AgentState, InterruptResult};
use mofa_foundation::agent::BaseAgent;
use mofa_runtime::agent::core::MoFAAgent;

// 定义一个简单的LLM Agent
// Define a simple LLM Agent
struct SimpleLlmAgent {
    base: BaseAgent,
}

impl SimpleLlmAgent {
    fn new() -> Self {
        Self {
            base: BaseAgent::new("simple-llm", "Simple LLM Agent"),
        }
    }
}

#[async_trait]
impl MoFAAgent for SimpleLlmAgent {
    fn id(&self) -> &str {
        &self.base.id
    }

    fn name(&self) -> &str {
        &self.base.name
    }

    fn capabilities(&self) -> &AgentCapabilities {
        &self.base.capabilities
    }

    async fn initialize(&mut self, ctx: &AgentContext) -> AgentResult<()> {
        self.base.initialize(ctx).await
    }

    async fn execute(&mut self, input: AgentInput, ctx: &AgentContext) -> AgentResult<AgentOutput> {
        // 从上下文获取HTTP插件的响应
        // Get the HTTP plugin response from the context
        if let Some(http_response) = ctx.get::<String>("http_response").await {
            println!("LLM Agent received HTTP response from context: {}", http_response);
        }

        // 从上下文获取自定义插件的处理结果
        // Get the custom plugin processing result from the context
        if let Some(processed_input) = ctx.get::<String>("processed_input").await {
            println!("LLM Agent received processed input from context: {}", processed_input);
        }

        // 返回简单响应
        // Return a simple response
        Ok(AgentOutput::text("Hello from LLM Agent!"))
    }

    async fn interrupt(&mut self) -> AgentResult<InterruptResult> {
        Ok(InterruptResult::Acknowledged)
    }

    async fn shutdown(&mut self) -> AgentResult<()> {
        Ok(())
    }

    fn state(&self) -> AgentState {
        self.base.state.clone()
    }
}

#[tokio::main]
async fn main() -> AgentResult<()> {
    // 创建Agent注册中心
    // Create the Agent registry
    let registry = Arc::new(AgentRegistry::new());

    // 注册LLM Agent
    // Register the LLM Agent
    let llm_agent = Arc::new(tokio::sync::RwLock::new(SimpleLlmAgent::new()));
    registry.register(llm_agent).await?;

    // 创建执行引擎
    // Create the execution engine
    let engine = ExecutionEngine::new(registry);

    // 创建HTTP插件示例
    // Create an example HTTP plugin
    let http_plugin = Arc::new(HttpPlugin::new("https://example.com"));

    // 创建自定义函数插件示例
    // Create an example custom function plugin
    let custom_plugin = Arc::new(CustomFunctionPlugin::new(
        "custom-input-plugin",
        "Custom input processing plugin",
        |input: AgentInput, ctx: &AgentContext| {
            println!("Custom plugin received input: {}", input.to_text());

            // 在上下文存储处理后的输入
            // Store the processed input in the context
            tokio::spawn(async move {
                ctx.set("processed_input", format!("Processed: {}", input.to_text())).await;
            });

            Ok(input)
        }
    ));

    // 注册插件
    // Register the plugins
    engine.register_plugin(http_plugin)?;
    engine.register_plugin(custom_plugin)?;

    println!("插件数量: {}", engine.plugin_count());
    // Plugin count: {}
    println!("所有插件: {:?}", engine.list_plugins().iter().map(|p| p.name()).collect::<Vec<_>>());
    // All plugins: {:?}

    // 执行Agent
    // Execute the Agent
    let result = engine
        .execute(
            "simple-llm",
            AgentInput::text("Hello, how are you?"),
            ExecutionOptions::default(),
        )
        .await?;

    println!("\n执行结果:");
    // \nExecution result:
    println!("状态: {:?}", result.status);
    // Status: {:?}
    if let Some(output) = result.output {
        println!("输出: {}", output.to_text());
        // Output: {}
    }

    // 移除插件
    // Remove the plugin
    engine.unregister_plugin("http-plugin")?;
    println!("\n移除http-plugin后插件数量: {}", engine.plugin_count());
    // \nPlugin count after removing http-plugin: {}

    Ok(())
}
