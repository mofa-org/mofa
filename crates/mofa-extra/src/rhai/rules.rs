//! Rhai Rule Engine
//!
//! Provides flexible business rule definition and execution capabilities:
//! - Conditional rule evaluation
//! - Rule chains and rule groups
//! - Action triggers
//! - Rule priority management
//! - Rule hot-reloading

use super::engine::{RhaiScriptEngine, ScriptContext, ScriptEngineConfig};
use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

// ============================================================================
// Rule Definitions
// ============================================================================

/// Rule priority
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
pub enum RulePriority {
    Lowest = 0,
    Low = 25,
    #[default]
    Normal = 50,
    High = 75,
    Highest = 100,
    Critical = 200,
}

/// Rule matching mode
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum RuleMatchMode {
    /// Stop after executing the first matching rule
    #[default]
    FirstMatch,
    /// Execute all matching rules
    AllMatch,
    /// Execute all matching rules in priority order
    AllMatchOrdered,
    /// Execute until the first successful rule
    FirstSuccess,
}

/// Rule action type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum RuleAction {
    /// Return a fixed value
    ReturnValue { value: serde_json::Value },
    /// Execute a script and return the result
    ExecuteScript { script: String },
    /// Call a function
    CallFunction {
        function: String,
        args: Vec<serde_json::Value>,
    },
    /// Modify context variable
    SetVariable {
        name: String,
        value: serde_json::Value,
    },
    /// Trigger an event
    TriggerEvent {
        event_type: String,
        data: serde_json::Value,
    },
    /// Jump to another rule
    GotoRule { rule_id: String },
    /// Stop rule execution
    Stop,
    /// Combine multiple actions
    Composite { actions: Vec<RuleAction> },
}

/// Rule definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleDefinition {
    /// Rule ID
    pub id: String,
    /// Rule name
    pub name: String,
    /// Rule description
    #[serde(default)]
    pub description: String,
    /// Rule priority
    #[serde(default)]
    pub priority: RulePriority,
    /// Whether enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Condition script (returns bool)
    pub condition: String,
    /// Rule action
    pub action: RuleAction,
    /// Rule tags
    #[serde(default)]
    pub tags: Vec<String>,
    /// Metadata
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

fn default_true() -> bool {
    true
}

impl RuleDefinition {
    pub fn new(id: &str, name: &str, condition: &str, action: RuleAction) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            description: String::new(),
            priority: RulePriority::Normal,
            enabled: true,
            condition: condition.to_string(),
            action,
            tags: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    pub fn with_priority(mut self, priority: RulePriority) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_description(mut self, desc: &str) -> Self {
        self.description = desc.to_string();
        self
    }

    pub fn with_tag(mut self, tag: &str) -> Self {
        self.tags.push(tag.to_string());
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }
}

// ============================================================================
// Rule Groups
// ============================================================================

/// Rule group definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleGroupDefinition {
    /// Group ID
    pub id: String,
    /// Group name
    pub name: String,
    /// Group description
    #[serde(default)]
    pub description: String,
    /// Matching mode
    #[serde(default)]
    pub match_mode: RuleMatchMode,
    /// List of rule IDs in the group
    pub rule_ids: Vec<String>,
    /// Whether enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Default action (executed when no rule matches)
    pub default_action: Option<RuleAction>,
}

impl RuleGroupDefinition {
    pub fn new(id: &str, name: &str) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            description: String::new(),
            match_mode: RuleMatchMode::FirstMatch,
            rule_ids: Vec::new(),
            enabled: true,
            default_action: None,
        }
    }

    pub fn with_match_mode(mut self, mode: RuleMatchMode) -> Self {
        self.match_mode = mode;
        self
    }

    pub fn with_rules(mut self, rule_ids: Vec<&str>) -> Self {
        self.rule_ids = rule_ids.into_iter().map(|s| s.to_string()).collect();
        self
    }

    pub fn with_default_action(mut self, action: RuleAction) -> Self {
        self.default_action = Some(action);
        self
    }
}

// ============================================================================
// Rule Execution Results
// ============================================================================

/// Rule match result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleMatchResult {
    /// Rule ID
    pub rule_id: String,
    /// Whether matched
    pub matched: bool,
    /// Condition evaluation time (milliseconds)
    pub evaluation_time_ms: u64,
}

/// Rule execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleExecutionResult {
    /// Executed rule ID
    pub rule_id: String,
    /// Whether successful
    pub success: bool,
    /// Action result
    pub result: serde_json::Value,
    /// Error message
    pub error: Option<String>,
    /// Execution time (milliseconds)
    pub execution_time_ms: u64,
    /// Variable updates
    pub variable_updates: HashMap<String, serde_json::Value>,
    /// Triggered events
    pub triggered_events: Vec<(String, serde_json::Value)>,
}

/// Rule group execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleGroupExecutionResult {
    /// Group ID
    pub group_id: String,
    /// Match results list
    pub match_results: Vec<RuleMatchResult>,
    /// Execution results list
    pub execution_results: Vec<RuleExecutionResult>,
    /// Final result (if any)
    pub final_result: Option<serde_json::Value>,
    /// Whether any rule matched
    pub any_matched: bool,
    /// Whether default action was executed
    pub used_default: bool,
    /// Total execution time (milliseconds)
    pub total_time_ms: u64,
}

// ============================================================================
// Rule Engine
// ============================================================================
pub type HandlerMap =
    Arc<RwLock<HashMap<String, Vec<Box<dyn Fn(&str, &serde_json::Value) + Send + Sync>>>>>;
/// Rule engine
pub struct RuleEngine {
    /// Script engine
    engine: Arc<RhaiScriptEngine>,
    /// Rule storage
    rules: Arc<RwLock<HashMap<String, RuleDefinition>>>,
    /// Rule group storage
    groups: Arc<RwLock<HashMap<String, RuleGroupDefinition>>>,
    /// Event handlers
    event_handlers: HandlerMap,
}

impl RuleEngine {
    /// Create a rule engine
    pub fn new(engine_config: ScriptEngineConfig) -> Result<Self> {
        let engine = Arc::new(RhaiScriptEngine::new(engine_config)?);
        Ok(Self {
            engine,
            rules: Arc::new(RwLock::new(HashMap::new())),
            groups: Arc::new(RwLock::new(HashMap::new())),
            event_handlers: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Create with an existing engine
    pub fn with_engine(engine: Arc<RhaiScriptEngine>) -> Self {
        Self {
            engine,
            rules: Arc::new(RwLock::new(HashMap::new())),
            groups: Arc::new(RwLock::new(HashMap::new())),
            event_handlers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a rule
    pub async fn register_rule(&self, rule: RuleDefinition) -> Result<()> {
        let mut rules = self.rules.write().await;
        info!("Registered rule: {} ({})", rule.name, rule.id);
        rules.insert(rule.id.clone(), rule);
        Ok(())
    }

    /// Batch register rules
    pub async fn register_rules(&self, rules: Vec<RuleDefinition>) -> Result<()> {
        for rule in rules {
            self.register_rule(rule).await?;
        }
        Ok(())
    }

    /// Register a rule group
    pub async fn register_group(&self, group: RuleGroupDefinition) -> Result<()> {
        let mut groups = self.groups.write().await;
        info!("Registered rule group: {} ({})", group.name, group.id);
        groups.insert(group.id.clone(), group);
        Ok(())
    }

    /// Load rules from YAML
    pub async fn load_rules_from_yaml(&self, path: &str) -> Result<Vec<String>> {
        let content = tokio::fs::read_to_string(path).await?;
        let rules: Vec<RuleDefinition> = serde_yaml::from_str(&content)?;
        let ids: Vec<String> = rules.iter().map(|r| r.id.clone()).collect();
        self.register_rules(rules).await?;
        Ok(ids)
    }

    /// Load rules from JSON
    pub async fn load_rules_from_json(&self, path: &str) -> Result<Vec<String>> {
        let content = tokio::fs::read_to_string(path).await?;
        let rules: Vec<RuleDefinition> = serde_json::from_str(&content)?;
        let ids: Vec<String> = rules.iter().map(|r| r.id.clone()).collect();
        self.register_rules(rules).await?;
        Ok(ids)
    }

    /// Evaluate rule condition
    pub async fn evaluate_condition(
        &self,
        rule: &RuleDefinition,
        context: &ScriptContext,
    ) -> Result<bool> {
        if !rule.enabled {
            return Ok(false);
        }

        let result = self.engine.execute(&rule.condition, context).await?;

        if !result.success {
            warn!(
                "Rule {} condition evaluation failed: {:?}",
                rule.id, result.error
            );
            return Ok(false);
        }

        // Convert result to boolean
        Ok(match &result.value {
            serde_json::Value::Bool(b) => *b,
            serde_json::Value::Number(n) => n.as_i64().unwrap_or(0) != 0,
            serde_json::Value::String(s) => !s.is_empty() && s != "false" && s != "0",
            serde_json::Value::Array(arr) => !arr.is_empty(),
            serde_json::Value::Object(obj) => !obj.is_empty(),
            serde_json::Value::Null => false,
        })
    }

    /// Execute rule action
    pub fn execute_action<'a>(
        &'a self,
        action: &'a RuleAction,
        context: &'a mut ScriptContext,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<RuleExecutionResult>> + Send + 'a>>
    {
        Box::pin(async move {
            let start_time = std::time::Instant::now();
            let mut variable_updates = HashMap::new();
            let mut triggered_events = Vec::new();

            let result = match action {
                RuleAction::ReturnValue { value } => value.clone(),

                RuleAction::ExecuteScript { script } => {
                    let result = self.engine.execute(script, context).await?;
                    if !result.success {
                        return Ok(RuleExecutionResult {
                            rule_id: String::new(),
                            success: false,
                            result: serde_json::Value::Null,
                            error: result.error,
                            execution_time_ms: start_time.elapsed().as_millis() as u64,
                            variable_updates,
                            triggered_events,
                        });
                    }
                    result.value
                }

                RuleAction::CallFunction { function, args } => {
                    // Simplified handling: convert function call to script
                    let args_str = args
                        .iter()
                        .map(|a| a.to_string())
                        .collect::<Vec<_>>()
                        .join(", ");
                    let script = format!("{}({})", function, args_str);
                    let result = self.engine.execute(&script, context).await?;
                    if !result.success {
                        return Ok(RuleExecutionResult {
                            rule_id: String::new(),
                            success: false,
                            result: serde_json::Value::Null,
                            error: result.error,
                            execution_time_ms: start_time.elapsed().as_millis() as u64,
                            variable_updates,
                            triggered_events,
                        });
                    }
                    result.value
                }

                RuleAction::SetVariable { name, value } => {
                    context.set_variable(name, value.clone())?;
                    variable_updates.insert(name.clone(), value.clone());
                    serde_json::json!({ "set": name, "value": value })
                }

                RuleAction::TriggerEvent { event_type, data } => {
                    triggered_events.push((event_type.clone(), data.clone()));
                    // Call event handlers
                    let handlers = self.event_handlers.read().await;
                    if let Some(handlers) = handlers.get(event_type) {
                        for handler in handlers {
                            handler(event_type, data);
                        }
                    }
                    serde_json::json!({ "event": event_type, "data": data })
                }

                RuleAction::GotoRule { rule_id } => {
                    // Return special value indicating jump
                    serde_json::json!({ "goto": rule_id })
                }

                RuleAction::Stop => {
                    serde_json::json!({ "stop": true })
                }

                RuleAction::Composite { actions } => {
                    // For composite actions, execute all sub-actions sequentially
                    let mut results = Vec::new();
                    for sub_action in actions {
                        // Use non-recursive handling
                        let sub_result = self.execute_single_action(sub_action, context).await?;
                        if !sub_result.success {
                            return Ok(sub_result);
                        }
                        results.push(sub_result.result);
                        variable_updates.extend(sub_result.variable_updates);
                        triggered_events.extend(sub_result.triggered_events);
                    }
                    serde_json::json!(results)
                }
            };

            Ok(RuleExecutionResult {
                rule_id: String::new(),
                success: true,
                result,
                error: None,
                execution_time_ms: start_time.elapsed().as_millis() as u64,
                variable_updates,
                triggered_events,
            })
        })
    }

    /// Execute a single non-composite action (avoid recursion)
    async fn execute_single_action(
        &self,
        action: &RuleAction,
        context: &mut ScriptContext,
    ) -> Result<RuleExecutionResult> {
        let start_time = std::time::Instant::now();
        let mut variable_updates = HashMap::new();
        let mut triggered_events = Vec::new();

        let result = match action {
            RuleAction::ReturnValue { value } => value.clone(),

            RuleAction::ExecuteScript { script } => {
                let result = self.engine.execute(script, context).await?;
                if !result.success {
                    return Ok(RuleExecutionResult {
                        rule_id: String::new(),
                        success: false,
                        result: serde_json::Value::Null,
                        error: result.error,
                        execution_time_ms: start_time.elapsed().as_millis() as u64,
                        variable_updates,
                        triggered_events,
                    });
                }
                result.value
            }

            RuleAction::CallFunction { function, args } => {
                let args_str = args
                    .iter()
                    .map(|a| a.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                let script = format!("{}({})", function, args_str);
                let result = self.engine.execute(&script, context).await?;
                if !result.success {
                    return Ok(RuleExecutionResult {
                        rule_id: String::new(),
                        success: false,
                        result: serde_json::Value::Null,
                        error: result.error,
                        execution_time_ms: start_time.elapsed().as_millis() as u64,
                        variable_updates,
                        triggered_events,
                    });
                }
                result.value
            }

            RuleAction::SetVariable { name, value } => {
                context.set_variable(name, value.clone())?;
                variable_updates.insert(name.clone(), value.clone());
                serde_json::json!({ "set": name, "value": value })
            }

            RuleAction::TriggerEvent { event_type, data } => {
                triggered_events.push((event_type.clone(), data.clone()));
                let handlers = self.event_handlers.read().await;
                if let Some(handlers) = handlers.get(event_type) {
                    for handler in handlers {
                        handler(event_type, data);
                    }
                }
                serde_json::json!({ "event": event_type, "data": data })
            }

            RuleAction::GotoRule { rule_id } => {
                serde_json::json!({ "goto": rule_id })
            }

            RuleAction::Stop => {
                serde_json::json!({ "stop": true })
            }

            RuleAction::Composite { .. } => {
                // Composite actions are not handled recursively here, return error
                return Err(anyhow!("Nested composite actions are not supported"));
            }
        };

        Ok(RuleExecutionResult {
            rule_id: String::new(),
            success: true,
            result,
            error: None,
            execution_time_ms: start_time.elapsed().as_millis() as u64,
            variable_updates,
            triggered_events,
        })
    }

    /// Execute a single rule
    pub async fn execute_rule(
        &self,
        rule_id: &str,
        context: &mut ScriptContext,
    ) -> Result<Option<RuleExecutionResult>> {
        let rules = self.rules.read().await;
        let rule = rules
            .get(rule_id)
            .ok_or_else(|| anyhow!("Rule not found: {}", rule_id))?
            .clone();
        drop(rules);

        // Evaluate condition
        if !self.evaluate_condition(&rule, context).await? {
            return Ok(None);
        }

        // Execute action
        let mut result = self.execute_action(&rule.action, context).await?;
        result.rule_id = rule_id.to_string();
        Ok(Some(result))
    }

    /// Execute a rule group
    pub async fn execute_group(
        &self,
        group_id: &str,
        context: &mut ScriptContext,
    ) -> Result<RuleGroupExecutionResult> {
        let start_time = std::time::Instant::now();

        let groups = self.groups.read().await;
        let group = groups
            .get(group_id)
            .ok_or_else(|| anyhow!("Rule group not found: {}", group_id))?
            .clone();
        drop(groups);

        if !group.enabled {
            return Ok(RuleGroupExecutionResult {
                group_id: group_id.to_string(),
                match_results: Vec::new(),
                execution_results: Vec::new(),
                final_result: None,
                any_matched: false,
                used_default: false,
                total_time_ms: start_time.elapsed().as_millis() as u64,
            });
        }

        // Get rules and sort by priority
        let rules = self.rules.read().await;
        let mut group_rules: Vec<_> = group
            .rule_ids
            .iter()
            .filter_map(|id| rules.get(id).cloned())
            .collect();
        drop(rules);

        // Sort by priority (higher priority first)
        group_rules.sort_by(|a, b| b.priority.cmp(&a.priority));

        let mut match_results = Vec::new();
        let mut execution_results = Vec::new();
        let mut any_matched = false;
        let mut final_result = None;

        for rule in group_rules {
            let eval_start = std::time::Instant::now();
            let matched = self.evaluate_condition(&rule, context).await?;

            match_results.push(RuleMatchResult {
                rule_id: rule.id.clone(),
                matched,
                evaluation_time_ms: eval_start.elapsed().as_millis() as u64,
            });

            if !matched {
                continue;
            }

            any_matched = true;

            // Execute action
            let mut result = self.execute_action(&rule.action, context).await?;
            result.rule_id = rule.id.clone();

            // Check if execution should stop
            let should_stop = if let Some(obj) = result.result.as_object() {
                obj.contains_key("stop")
            } else {
                false
            };

            // Check for rule jump
            let goto_rule = if let Some(obj) = result.result.as_object() {
                obj.get("goto")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            } else {
                None
            };

            final_result = Some(result.result.clone());
            execution_results.push(result);

            if should_stop {
                break;
            }

            if let Some(target_rule_id) = goto_rule {
                // Execute target rule
                if let Some(goto_result) = self.execute_rule(&target_rule_id, context).await? {
                    final_result = Some(goto_result.result.clone());
                    execution_results.push(goto_result);
                }
                break;
            }

            // Decide whether to continue based on match mode
            match group.match_mode {
                RuleMatchMode::FirstMatch | RuleMatchMode::FirstSuccess => break,
                RuleMatchMode::AllMatch | RuleMatchMode::AllMatchOrdered => continue,
            }
        }

        // If no match and has default action
        let used_default = !any_matched && group.default_action.is_some();
        if let Some(ref default_action) = group.default_action
            && !any_matched
        {
            let mut result = self.execute_action(default_action, context).await?;
            result.rule_id = format!("{}_default", group_id);
            final_result = Some(result.result.clone());
            execution_results.push(result);
        }

        Ok(RuleGroupExecutionResult {
            group_id: group_id.to_string(),
            match_results,
            execution_results,
            final_result,
            any_matched,
            used_default,
            total_time_ms: start_time.elapsed().as_millis() as u64,
        })
    }

    /// Execute all matching rules
    pub async fn execute_all(
        &self,
        context: &mut ScriptContext,
    ) -> Result<Vec<RuleExecutionResult>> {
        let rules = self.rules.read().await;
        let mut all_rules: Vec<_> = rules.values().cloned().collect();
        drop(rules);

        // Sort by priority
        all_rules.sort_by(|a, b| b.priority.cmp(&a.priority));

        let mut results = Vec::new();

        for rule in all_rules {
            if !rule.enabled {
                continue;
            }

            if self.evaluate_condition(&rule, context).await? {
                let mut result = self.execute_action(&rule.action, context).await?;
                result.rule_id = rule.id.clone();
                results.push(result);
            }
        }

        Ok(results)
    }

    /// Get a rule
    pub async fn get_rule(&self, rule_id: &str) -> Option<RuleDefinition> {
        let rules = self.rules.read().await;
        rules.get(rule_id).cloned()
    }

    /// List all rules
    pub async fn list_rules(&self) -> Vec<RuleDefinition> {
        let rules = self.rules.read().await;
        rules.values().cloned().collect()
    }

    /// Filter rules by tag
    pub async fn list_rules_by_tag(&self, tag: &str) -> Vec<RuleDefinition> {
        let rules = self.rules.read().await;
        rules
            .values()
            .filter(|r| r.tags.contains(&tag.to_string()))
            .cloned()
            .collect()
    }

    /// Remove a rule
    pub async fn unregister_rule(&self, rule_id: &str) -> bool {
        let mut rules = self.rules.write().await;
        rules.remove(rule_id).is_some()
    }

    /// Enable a rule
    pub async fn enable_rule(&self, rule_id: &str) -> Result<()> {
        let mut rules = self.rules.write().await;
        if let Some(rule) = rules.get_mut(rule_id) {
            rule.enabled = true;
            Ok(())
        } else {
            Err(anyhow!("Rule not found: {}", rule_id))
        }
    }

    /// Disable a rule
    pub async fn disable_rule(&self, rule_id: &str) -> Result<()> {
        let mut rules = self.rules.write().await;
        if let Some(rule) = rules.get_mut(rule_id) {
            rule.enabled = false;
            Ok(())
        } else {
            Err(anyhow!("Rule not found: {}", rule_id))
        }
    }

    /// Rule count
    pub async fn rule_count(&self) -> usize {
        let rules = self.rules.read().await;
        rules.len()
    }

    /// Clear all rules
    pub async fn clear(&self) {
        let mut rules = self.rules.write().await;
        let mut groups = self.groups.write().await;
        rules.clear();
        groups.clear();
    }
}

// ============================================================================
// Convenience Builders
// ============================================================================

/// Rule builder
pub struct RuleBuilder {
    rule: RuleDefinition,
}

impl RuleBuilder {
    pub fn new(id: &str, name: &str) -> Self {
        Self {
            rule: RuleDefinition {
                id: id.to_string(),
                name: name.to_string(),
                description: String::new(),
                priority: RulePriority::Normal,
                enabled: true,
                condition: "true".to_string(),
                action: RuleAction::Stop,
                tags: Vec::new(),
                metadata: HashMap::new(),
            },
        }
    }

    pub fn description(mut self, desc: &str) -> Self {
        self.rule.description = desc.to_string();
        self
    }

    pub fn priority(mut self, priority: RulePriority) -> Self {
        self.rule.priority = priority;
        self
    }

    pub fn condition(mut self, condition: &str) -> Self {
        self.rule.condition = condition.to_string();
        self
    }

    pub fn when_true(mut self, condition: &str) -> Self {
        self.rule.condition = condition.to_string();
        self
    }

    pub fn then_return(mut self, value: serde_json::Value) -> Self {
        self.rule.action = RuleAction::ReturnValue { value };
        self
    }

    pub fn then_execute(mut self, script: &str) -> Self {
        self.rule.action = RuleAction::ExecuteScript {
            script: script.to_string(),
        };
        self
    }

    pub fn then_set(mut self, name: &str, value: serde_json::Value) -> Self {
        self.rule.action = RuleAction::SetVariable {
            name: name.to_string(),
            value,
        };
        self
    }

    pub fn then_trigger(mut self, event_type: &str, data: serde_json::Value) -> Self {
        self.rule.action = RuleAction::TriggerEvent {
            event_type: event_type.to_string(),
            data,
        };
        self
    }

    pub fn then_goto(mut self, rule_id: &str) -> Self {
        self.rule.action = RuleAction::GotoRule {
            rule_id: rule_id.to_string(),
        };
        self
    }

    pub fn then_stop(mut self) -> Self {
        self.rule.action = RuleAction::Stop;
        self
    }

    pub fn action(mut self, action: RuleAction) -> Self {
        self.rule.action = action;
        self
    }

    pub fn tag(mut self, tag: &str) -> Self {
        self.rule.tags.push(tag.to_string());
        self
    }

    pub fn disabled(mut self) -> Self {
        self.rule.enabled = false;
        self
    }

    pub fn build(self) -> RuleDefinition {
        self.rule
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rule_registration() {
        let engine = RuleEngine::new(ScriptEngineConfig::default()).unwrap();

        let rule = RuleBuilder::new("test_rule", "Test Rule")
            .condition("value > 10")
            .then_return(serde_json::json!("high"))
            .build();

        engine.register_rule(rule).await.unwrap();

        assert_eq!(engine.rule_count().await, 1);
    }

    #[tokio::test]
    async fn test_rule_execution() {
        let engine = RuleEngine::new(ScriptEngineConfig::default()).unwrap();

        let rule = RuleBuilder::new("check_value", "Check Value")
            .condition("value > 100")
            .then_execute(r#"value * 2"#)
            .build();

        engine.register_rule(rule).await.unwrap();

        let mut context = ScriptContext::new().with_variable("value", 150).unwrap();

        let result = engine
            .execute_rule("check_value", &mut context)
            .await
            .unwrap();

        assert!(result.is_some());
        let result = result.unwrap();
        assert!(result.success);
        assert_eq!(result.result, serde_json::json!(300));
    }

    #[tokio::test]
    async fn test_rule_condition_not_met() {
        let engine = RuleEngine::new(ScriptEngineConfig::default()).unwrap();

        let rule = RuleBuilder::new("check_value", "Check Value")
            .condition("value > 100")
            .then_return(serde_json::json!("high"))
            .build();

        engine.register_rule(rule).await.unwrap();

        let mut context = ScriptContext::new().with_variable("value", 50).unwrap();

        let result = engine
            .execute_rule("check_value", &mut context)
            .await
            .unwrap();

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_rule_group_first_match() {
        let engine = RuleEngine::new(ScriptEngineConfig::default()).unwrap();

        // Register multiple rules
        let rules = vec![
            RuleBuilder::new("rule_high", "High Value")
                .priority(RulePriority::High)
                .condition("value > 100")
                .then_return(serde_json::json!("high"))
                .build(),
            RuleBuilder::new("rule_medium", "Medium Value")
                .priority(RulePriority::Normal)
                .condition("value > 50")
                .then_return(serde_json::json!("medium"))
                .build(),
            RuleBuilder::new("rule_low", "Low Value")
                .priority(RulePriority::Low)
                .condition("value > 0")
                .then_return(serde_json::json!("low"))
                .build(),
        ];

        engine.register_rules(rules).await.unwrap();

        // Create rule group
        let group = RuleGroupDefinition::new("value_checker", "Value Checker")
            .with_match_mode(RuleMatchMode::FirstMatch)
            .with_rules(vec!["rule_high", "rule_medium", "rule_low"]);

        engine.register_group(group).await.unwrap();

        // Test high value
        let mut context = ScriptContext::new().with_variable("value", 150).unwrap();
        let result = engine
            .execute_group("value_checker", &mut context)
            .await
            .unwrap();

        assert!(result.any_matched);
        assert_eq!(result.execution_results.len(), 1);
        assert_eq!(result.final_result, Some(serde_json::json!("high")));

        // Test medium value
        let mut context = ScriptContext::new().with_variable("value", 75).unwrap();
        let result = engine
            .execute_group("value_checker", &mut context)
            .await
            .unwrap();

        assert!(result.any_matched);
        assert_eq!(result.final_result, Some(serde_json::json!("medium")));
    }

    #[tokio::test]
    async fn test_rule_with_default_action() {
        let engine = RuleEngine::new(ScriptEngineConfig::default()).unwrap();

        let rule = RuleBuilder::new("positive_rule", "Positive Only")
            .condition("value > 0")
            .then_return(serde_json::json!("positive"))
            .build();

        engine.register_rule(rule).await.unwrap();

        let group = RuleGroupDefinition::new("number_group", "Number Group")
            .with_rules(vec!["positive_rule"])
            .with_default_action(RuleAction::ReturnValue {
                value: serde_json::json!("non_positive"),
            });

        engine.register_group(group).await.unwrap();

        // Test negative value, should use default action
        let mut context = ScriptContext::new().with_variable("value", -10).unwrap();
        let result = engine
            .execute_group("number_group", &mut context)
            .await
            .unwrap();

        assert!(!result.any_matched);
        assert!(result.used_default);
        assert_eq!(result.final_result, Some(serde_json::json!("non_positive")));
    }

    #[tokio::test]
    async fn test_set_variable_action() {
        let engine = RuleEngine::new(ScriptEngineConfig::default()).unwrap();

        let rule = RuleBuilder::new("set_status", "Set Status")
            .condition("true")
            .then_set("status", serde_json::json!("processed"))
            .build();

        engine.register_rule(rule).await.unwrap();

        let mut context = ScriptContext::new();
        let result = engine
            .execute_rule("set_status", &mut context)
            .await
            .unwrap()
            .unwrap();

        assert!(result.success);
        assert!(result.variable_updates.contains_key("status"));
        assert_eq!(
            context.get_variable::<String>("status"),
            Some("processed".to_string())
        );
    }

    #[test]
    fn test_rule_builder() {
        let rule = RuleBuilder::new("my_rule", "My Rule")
            .description("A test rule")
            .priority(RulePriority::High)
            .condition("x > 10")
            .then_return(serde_json::json!({"result": "success"}))
            .tag("test")
            .build();

        assert_eq!(rule.id, "my_rule");
        assert_eq!(rule.priority, RulePriority::High);
        assert_eq!(rule.condition, "x > 10");
        assert!(rule.tags.contains(&"test".to_string()));
    }
}
