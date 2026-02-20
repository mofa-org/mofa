use std::sync::RwLock;

/// 路由规则
#[derive(Clone)]
pub struct RouteRule {
    /// 规则名称
    pub name: String,
    /// 上下文关键词模式
    pub context_pattern: String,
    /// 目标工具名称
    pub target_tool: String,
    /// 优先级 (0-100)
    pub priority: u8,
}

impl RouteRule {
    /// 创建新规则
    pub fn new(name: impl Into<String>, context_pattern: impl Into<String>, target_tool: impl Into<String>, priority: u8) -> Self {
        Self {
            name: name.into(),
            context_pattern: context_pattern.into(),
            target_tool: target_tool.into(),
            priority,
        }
    }

    /// 检查文本是否匹配规则
    pub fn match_text(&self, text: &str) -> bool {
        // 简单的关键词匹配，实际项目中可以使用正则
        let lower_text = text.to_lowercase();
        let lower_pattern = self.context_pattern.to_lowercase();

        // 检查是否包含所有关键词
        let keywords: Vec<&str> = lower_pattern.split_whitespace().collect();

        keywords.iter().all(|keyword| lower_text.contains(keyword))
    }
}

/// 路由规则管理器
pub struct RouteRuleManager {
    rules: RwLock<Vec<RouteRule>>,
}

impl Default for RouteRuleManager {
    fn default() -> Self {
        Self {
            rules: RwLock::new(vec![]),
        }
    }
}

impl RouteRuleManager {
    /// 创建新的规则管理器
    pub fn new() -> Self {
        Self::default()
    }

    /// 添加规则
    pub fn add_rule(&self, rule: RouteRule) {
        let mut rules = self.rules.write().unwrap();
        rules.push(rule);

        // 按优先级降序排序
        rules.sort_by(|a, b| b.priority.cmp(&a.priority));
    }

    /// 移除规则
    pub fn remove_rule(&self, rule_name: &str) {
        let mut rules = self.rules.write().unwrap();
        rules.retain(|rule| rule.name != rule_name);
    }

    /// 更新规则
    pub fn update_rule(&self, rule: RouteRule) {
        let mut rules = self.rules.write().unwrap();
        if let Some(pos) = rules.iter().position(|r| r.name == rule.name) {
            rules[pos] = rule;
            // 重新排序
            rules.sort_by(|a, b| b.priority.cmp(&a.priority));
        } else {
            // 如果规则不存在，添加它
            rules.push(rule);
            rules.sort_by(|a, b| b.priority.cmp(&a.priority));
        }
    }

    /// 查找匹配的规则
    pub fn find_match(&self, text: &str) -> Option<RouteRule> {
        let rules = self.rules.read().unwrap();

        // 按优先级顺序查找第一个匹配的规则
        for rule in rules.iter() {
            if rule.match_text(text) {
                return Some(rule.clone());
            }
        }

        None
    }

    /// 获取所有规则
    pub fn get_all_rules(&self) -> Vec<RouteRule> {
        let rules = self.rules.read().unwrap();
        rules.clone()
    }
}

/// 创建默认规则集
pub fn create_default_rules() -> Vec<RouteRule> {
    vec![
        // 规则1: 如果用户提及"最近"且涉及事件，自动路由到新闻API
        RouteRule::new("news_recent_events", "最近 事件", "news_query", 90),
        // 规则2: 如果涉及数字计算，自动路由到计算器
        RouteRule::new("calculator_arithmetic", "计算 数字 +-*/", "calculator", 85),
        // 规则3: 如果涉及天气查询
        RouteRule::new("weather_query_rule", "天气 温度", "weather_query", 80),
    ]
}

