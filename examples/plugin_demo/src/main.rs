use async_trait::async_trait;
use mofa_kernel::agent::context::AgentContext;
use mofa_kernel::agent::execution::{ExecutionEngine, ExecutionOptions};
use mofa_kernel::agent::plugins::{CustomFunctionPlugin, HttpPlugin, PluginStage};
use mofa_kernel::agent::registry::AgentRegistry;
use mofa_foundation::agent::BaseAgent;
use mofa_kernel::agent::traits::UnifiedAgent;
use mofa_kernel::agent::types::{AgentInput, AgentOutput};

// 定义一个简单的LLM Agent
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
impl UnifiedAgent for SimpleLlmAgent {
    fn id(&self) -> &str {
        &self.base.id
    }

    fn name(&self) -> &str {
        &self.base.name
    }

    fn capabilities(&self) -> &mofa_kernel::agent::capabilities::AgentCapabilities {
        &self.base.capabilities
    }

    async fn initialize(&mut self, ctx: &AgentContext) -> mofa_kernel::agent::error::AgentResult<()> {
        self.base.initialize(ctx).await
    }

    async fn execute(&mut self, input: AgentInput, ctx: &AgentContext) -> mofa_kernel::agent::error::AgentResult<AgentOutput> {
        // 从上下文获取HTTP插件的响应
        if let Some(http_response) = ctx.get::<String>("http_response").await {
            println!("LLM Agent received HTTP response from context: {}", http_response);
        }

        // 从上下文获取自定义插件的处理结果
        if let Some(processed_input) = ctx.get::<String>("processed_input").await {
            println!("LLM Agent received processed input from context: {}", processed_input);
        }

        // 返回简单响应
        Ok(AgentOutput::text("Hello from LLM Agent!"))
    }

    async fn interrupt(&mut self) -> mofa_kernel::agent::error::AgentResult<mofa_kernel::agent::types::InterruptResult> {
        Ok(mofa_kernel::agent::types::InterruptResult::Acknowledged)
    }

    async fn shutdown(&mut self) -> mofa_kernel::agent::error::AgentResult<()> {
        Ok(())
    }

    fn state(&self) -> mofa_kernel::agent::types::AgentState {
        self.base.state.clone()
    }
}

#[tokio::main]
async fn main() -> mofa_kernel::agent::error::AgentResult<()> {
    // 创建Agent注册中心
    let registry = Arc::new(AgentRegistry::new());

    // 注册LLM Agent
    let llm_agent = Arc::new(tokio::sync::RwLock::new(SimpleLlmAgent::new()));
    registry.register(llm_agent).await?;

    // 创建执行引擎
    let engine = ExecutionEngine::new(registry);

    // 创建HTTP插件示例
    let http_plugin = Arc::new(HttpPlugin::new("https://example.com"));

    // 创建自定义函数插件示例
    let custom_plugin = Arc::new(CustomFunctionPlugin::new(
        "custom-input-plugin",
        "Custom input processing plugin",
        |input: AgentInput, ctx: &AgentContext| {
            println!("Custom plugin received input: {}", input.to_text());

            // 在上下文存储处理后的输入
            tokio::spawn(async move {
                ctx.set("processed_input", format!("Processed: {}", input.to_text())).await;
            });

            Ok(input)
        }
    ));

    // 注册插件
    engine.register_plugin(http_plugin)?;
    engine.register_plugin(custom_plugin)?;

    println!("插件数量: {}", engine.plugin_count());
    println!("所有插件: {:?}", engine.list_plugins().iter().map(|p| p.name()).collect::<Vec<_>>());

    // 执行Agent
    let result = engine
        .execute(
            "simple-llm",
            AgentInput::text("Hello, how are you?"),
            ExecutionOptions::default(),
        )
        .await?;

    println!("\n执行结果:");
    println!("状态: {:?}", result.status);
    if let Some(output) = result.output {
        println!("输出: {}", output.to_text());
    }

    // 移除插件
    engine.unregister_plugin("http-plugin")?;
    println!("\n移除http-plugin后插件数量: {}", engine.plugin_count());

    Ok(())
}
