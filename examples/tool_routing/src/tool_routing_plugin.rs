use std::sync::Arc;

use mofa_sdk::kernel::{
    AgentPlugin, PluginContext, PluginMetadata, PluginResult, PluginState, PluginType,
};
use mofa_sdk::kernel::plugin::PluginPriority;
use std::any::Any;
use std::collections::HashMap;

use crate::route_rules::{RouteRuleManager, create_default_rules};

/// 工具路由插件
/// Tool routing plugin
pub struct ToolRoutingPlugin {
    metadata: PluginMetadata,
    state: PluginState,
    rule_manager: Arc<RouteRuleManager>,
}

impl ToolRoutingPlugin {
    /// 创建新的工具路由插件
    /// Create a new tool routing plugin
    pub fn new() -> Self {
        let metadata = PluginMetadata::new("tool_routing", "Tool Routing Plugin", PluginType::Tool)
            .with_description("Context-aware tool routing plugin")
            .with_priority(PluginPriority::High)
            .with_capability("tool_routing");

        let plugin = Self {
            metadata,
            state: PluginState::Unloaded,
            rule_manager: Arc::new(RouteRuleManager::new()),
        };

        // 添加默认规则
        // Add default rules
        plugin.add_default_rules();

        plugin
    }

    /// 获取规则管理器
    /// Get the rule manager
    pub fn rule_manager(&self) -> Arc<RouteRuleManager> {
        self.rule_manager.clone()
    }

    /// 添加默认规则
    /// Add default rules
    fn add_default_rules(&self) {
        let default_rules = create_default_rules();
        for rule in default_rules {
            self.rule_manager.add_rule(rule);
        }
    }

    /// 路由分析：根据用户输入和上下文选择合适的工具
    /// Route analysis: select appropriate tools based on user input and context
    pub async fn route_analysis(&self, user_input: &str, context: &str) -> Option<String> {
        // 合并用户输入和上下文进行分析
        // Merge user input and context for analysis
        let combined_text = format!("{}\n{}", context, user_input);

        // 查找匹配的规则
        // Find matching rules
        if let Some(rule) = self.rule_manager.find_match(&combined_text) {
            println!("Route rule matched: {} -> {}", rule.name, rule.target_tool);
            Some(rule.target_tool)
        } else {
            println!("No route rule matched for input: {}", user_input);
            None
        }
    }
}

#[async_trait::async_trait]
impl AgentPlugin for ToolRoutingPlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    fn state(&self) -> PluginState {
        self.state.clone()
    }

    async fn load(&mut self, _ctx: &PluginContext) -> PluginResult<()> {
        self.state = PluginState::Loading;
        // 加载插件资源
        // Load plugin resources
        self.state = PluginState::Loaded;
        Ok(())
    }

    async fn init_plugin(&mut self) -> PluginResult<()> {
        // 初始化插件配置
        // Initialize plugin configuration
        Ok(())
    }

    async fn start(&mut self) -> PluginResult<()> {
        self.state = PluginState::Running;
        Ok(())
    }

    async fn stop(&mut self) -> PluginResult<()> {
        self.state = PluginState::Paused;
        Ok(())
    }

    async fn unload(&mut self) -> PluginResult<()> {
        self.state = PluginState::Unloaded;
        Ok(())
    }

    async fn execute(&mut self, input: String) -> PluginResult<String> {
        // 执行路由分析
        // Execute route analysis
        if let Some(tool) = self.route_analysis(&input, "").await {
            Ok(format!("ROUTE_TOOL:{}", tool))
        } else {
            Ok("ROUTE_TOOL:None".to_string())
        }
    }

    fn stats(&self) -> HashMap<String, serde_json::Value> {
        let rules = self.rule_manager.get_all_rules();
        let mut stats = HashMap::new();
        stats.insert("rule_count".to_string(), serde_json::json!(rules.len()));

        let active_rules: Vec<String> = rules
            .iter()
            .map(|rule| format!("{} -> {}", rule.name, rule.target_tool))
            .collect();

        stats.insert("active_rules".to_string(), serde_json::json!(active_rules));
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
