//! Rhai 工作流脚本节点
//!
//! 提供脚本化的工作流节点支持，允许用户通过 Rhai 脚本定义：
//! - 任务节点逻辑
//! - 条件判断逻辑
//! - 数据转换逻辑
//! - 循环控制逻辑

use super::engine::{RhaiScriptEngine, ScriptContext, ScriptEngineConfig, ScriptResult};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

// ============================================================================
// 脚本节点定义
// ============================================================================

/// 脚本节点类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ScriptNodeType {
    /// 任务脚本 - 执行具体业务逻辑
    Task,
    /// 条件脚本 - 返回布尔值用于分支判断
    Condition,
    /// 转换脚本 - 数据转换处理
    Transform,
    /// 验证脚本 - 数据验证
    Validator,
    /// 聚合脚本 - 多输入聚合处理
    Aggregator,
    /// 循环条件脚本 - 控制循环执行
    LoopCondition,
}

/// 脚本节点配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptNodeConfig {
    /// 节点 ID
    pub id: String,
    /// 节点名称
    pub name: String,
    /// 节点类型
    pub node_type: ScriptNodeType,
    /// 脚本源代码（内联方式）
    pub script_source: Option<String>,
    /// 脚本文件路径（文件方式）
    pub script_path: Option<String>,
    /// 入口函数名（默认为 "main"）
    pub entry_function: Option<String>,
    /// 是否启用缓存
    pub enable_cache: bool,
    /// 超时时间（毫秒）
    pub timeout_ms: u64,
    /// 重试次数
    pub max_retries: u32,
    /// 节点元数据
    pub metadata: HashMap<String, String>,
}

impl Default for ScriptNodeConfig {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            node_type: ScriptNodeType::Task,
            script_source: None,
            script_path: None,
            entry_function: None,
            enable_cache: true,
            timeout_ms: 30000,
            max_retries: 0,
            metadata: HashMap::new(),
        }
    }
}

impl ScriptNodeConfig {
    pub fn new(id: &str, name: &str, node_type: ScriptNodeType) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            node_type,
            ..Default::default()
        }
    }

    pub fn with_source(mut self, source: &str) -> Self {
        self.script_source = Some(source.to_string());
        self
    }

    pub fn with_path(mut self, path: &str) -> Self {
        self.script_path = Some(path.to_string());
        self
    }

    pub fn with_entry(mut self, function: &str) -> Self {
        self.entry_function = Some(function.to_string());
        self
    }

    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }
}

// ============================================================================
// 脚本节点执行器
// ============================================================================

/// 脚本节点执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptNodeResult {
    /// 节点 ID
    pub node_id: String,
    /// 是否成功
    pub success: bool,
    /// 输出值
    pub output: serde_json::Value,
    /// 错误信息
    pub error: Option<String>,
    /// 执行时间（毫秒）
    pub execution_time_ms: u64,
    /// 重试次数
    pub retry_count: u32,
    /// 脚本日志
    pub logs: Vec<String>,
}

/// 脚本工作流节点执行器
pub struct ScriptWorkflowNode {
    /// 节点配置
    config: ScriptNodeConfig,
    /// 脚本引擎
    engine: Arc<RhaiScriptEngine>,
    /// 编译后的脚本 ID（如果已缓存）
    cached_script_id: Option<String>,
}

impl ScriptWorkflowNode {
    /// 创建脚本节点
    pub async fn new(
        config: ScriptNodeConfig,
        engine: Arc<RhaiScriptEngine>,
    ) -> Result<Self> {
        let mut node = Self {
            config,
            engine,
            cached_script_id: None,
        };

        // 如果启用缓存，预编译脚本
        if node.config.enable_cache {
            node.compile_script().await?;
        }

        Ok(node)
    }

    /// 编译脚本
    async fn compile_script(&mut self) -> Result<()> {
        let source = self.get_script_source().await?;
        let script_id = format!("node_{}", self.config.id);

        self.engine
            .compile_and_cache(&script_id, &self.config.name, &source)
            .await?;

        self.cached_script_id = Some(script_id);
        Ok(())
    }

    /// 获取脚本源代码
    async fn get_script_source(&self) -> Result<String> {
        if let Some(ref source) = self.config.script_source {
            Ok(source.clone())
        } else if let Some(ref path) = self.config.script_path {
            tokio::fs::read_to_string(path)
                .await
                .map_err(|e| anyhow!("Failed to read script file: {}", e))
        } else {
            Err(anyhow!("No script source or path specified"))
        }
    }

    /// 执行节点
    pub async fn execute(&self, input: serde_json::Value) -> Result<ScriptNodeResult> {
        let start_time = std::time::Instant::now();
        let mut last_error = None;
        let mut retry_count = 0;

        // 准备上下文
        let mut context = ScriptContext::new()
            .with_node(&self.config.id)
            .with_variable("input", input.clone())?;

        // 添加元数据
        for (k, v) in &self.config.metadata {
            context.metadata.insert(k.clone(), v.clone());
        }

        // 带重试的执行
        while retry_count <= self.config.max_retries {
            let result = self.execute_once(&context).await;

            match result {
                Ok(script_result) if script_result.success => {
                    return Ok(ScriptNodeResult {
                        node_id: self.config.id.clone(),
                        success: true,
                        output: script_result.value,
                        error: None,
                        execution_time_ms: start_time.elapsed().as_millis() as u64,
                        retry_count,
                        logs: script_result.logs,
                    });
                }
                Ok(script_result) => {
                    last_error = script_result.error;
                }
                Err(e) => {
                    last_error = Some(e.to_string());
                }
            }

            if retry_count < self.config.max_retries {
                // 指数退避重试
                let delay = std::time::Duration::from_millis(100 * 2u64.pow(retry_count));
                tokio::time::sleep(delay).await;
            }
            retry_count += 1;
        }

        Ok(ScriptNodeResult {
            node_id: self.config.id.clone(),
            success: false,
            output: serde_json::Value::Null,
            error: last_error,
            execution_time_ms: start_time.elapsed().as_millis() as u64,
            retry_count: retry_count.saturating_sub(1),
            logs: Vec::new(),
        })
    }

    /// 执行一次（不重试）
    async fn execute_once(&self, context: &ScriptContext) -> Result<ScriptResult> {
        // 使用缓存的脚本或直接执行
        if let Some(ref script_id) = self.cached_script_id {
            // 如果有入口函数，调用函数
            if let Some(ref entry) = self.config.entry_function {
                let input = context.get_variable::<serde_json::Value>("input")
                    .unwrap_or(serde_json::Value::Null);

                let result: serde_json::Value = self.engine
                    .call_function(script_id, entry, vec![input], context)
                    .await?;

                Ok(ScriptResult::success(result, 0))
            } else {
                self.engine.execute_compiled(script_id, context).await
            }
        } else {
            let source = self.get_script_source().await?;
            self.engine.execute(&source, context).await
        }
    }

    /// 作为条件节点执行（返回布尔值）
    pub async fn execute_as_condition(&self, input: serde_json::Value) -> Result<bool> {
        let result = self.execute(input).await?;

        if !result.success {
            return Err(anyhow!(result.error.unwrap_or_else(|| "Condition execution failed".into())));
        }

        // 尝试将结果转换为布尔值
        match &result.output {
            serde_json::Value::Bool(b) => Ok(*b),
            serde_json::Value::Number(n) => Ok(n.as_i64().unwrap_or(0) != 0),
            serde_json::Value::String(s) => Ok(!s.is_empty() && s != "false" && s != "0"),
            serde_json::Value::Array(arr) => Ok(!arr.is_empty()),
            serde_json::Value::Object(obj) => Ok(!obj.is_empty()),
            serde_json::Value::Null => Ok(false),
        }
    }

    /// 获取节点配置
    pub fn config(&self) -> &ScriptNodeConfig {
        &self.config
    }

    /// 获取节点 ID
    pub fn id(&self) -> &str {
        &self.config.id
    }

    /// 获取节点名称
    pub fn name(&self) -> &str {
        &self.config.name
    }
}

// ============================================================================
// 脚本工作流构建器
// ============================================================================

/// 脚本工作流定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptWorkflowDefinition {
    /// 工作流 ID
    pub id: String,
    /// 工作流名称
    pub name: String,
    /// 工作流描述
    pub description: String,
    /// 节点配置列表
    pub nodes: Vec<ScriptNodeConfig>,
    /// 边定义：(源节点ID, 目标节点ID, 可选条件)
    pub edges: Vec<(String, String, Option<String>)>,
    /// 开始节点 ID
    pub start_node: String,
    /// 结束节点 ID 列表
    pub end_nodes: Vec<String>,
    /// 全局变量
    pub global_variables: HashMap<String, serde_json::Value>,
}

impl ScriptWorkflowDefinition {
    pub fn new(id: &str, name: &str) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            description: String::new(),
            nodes: Vec::new(),
            edges: Vec::new(),
            start_node: String::new(),
            end_nodes: Vec::new(),
            global_variables: HashMap::new(),
        }
    }

    /// 从 YAML 文件加载
    pub async fn from_yaml(path: &str) -> Result<Self> {
        let content = tokio::fs::read_to_string(path).await?;
        serde_yaml::from_str(&content).map_err(|e| anyhow!("Failed to parse YAML: {}", e))
    }

    /// 从 JSON 文件加载
    pub async fn from_json(path: &str) -> Result<Self> {
        let content = tokio::fs::read_to_string(path).await?;
        serde_json::from_str(&content).map_err(|e| anyhow!("Failed to parse JSON: {}", e))
    }

    /// 添加节点
    pub fn add_node(&mut self, config: ScriptNodeConfig) -> &mut Self {
        self.nodes.push(config);
        self
    }

    /// 添加边
    pub fn add_edge(&mut self, from: &str, to: &str) -> &mut Self {
        self.edges.push((from.to_string(), to.to_string(), None));
        self
    }

    /// 添加条件边
    pub fn add_conditional_edge(&mut self, from: &str, to: &str, condition: &str) -> &mut Self {
        self.edges.push((from.to_string(), to.to_string(), Some(condition.to_string())));
        self
    }

    /// 设置开始节点
    pub fn set_start(&mut self, node_id: &str) -> &mut Self {
        self.start_node = node_id.to_string();
        self
    }

    /// 添加结束节点
    pub fn add_end(&mut self, node_id: &str) -> &mut Self {
        self.end_nodes.push(node_id.to_string());
        self
    }

    /// 验证工作流定义
    pub fn validate(&self) -> Result<Vec<String>> {
        let mut errors = Vec::new();

        if self.id.is_empty() {
            errors.push("Workflow ID is required".to_string());
        }

        if self.start_node.is_empty() {
            errors.push("Start node is not specified".to_string());
        }

        if self.end_nodes.is_empty() {
            errors.push("At least one end node is required".to_string());
        }

        // 检查所有引用的节点是否存在
        let node_ids: std::collections::HashSet<_> = self.nodes.iter().map(|n| &n.id).collect();

        if !node_ids.contains(&self.start_node) {
            errors.push(format!("Start node '{}' not found", self.start_node));
        }

        for end_node in &self.end_nodes {
            if !node_ids.contains(end_node) {
                errors.push(format!("End node '{}' not found", end_node));
            }
        }

        for (from, to, _) in &self.edges {
            if !node_ids.contains(from) {
                errors.push(format!("Edge source node '{}' not found", from));
            }
            if !node_ids.contains(to) {
                errors.push(format!("Edge target node '{}' not found", to));
            }
        }

        Ok(errors)
    }
}

// ============================================================================
// 脚本工作流执行器
// ============================================================================

/// 脚本工作流执行器
pub struct ScriptWorkflowExecutor {
    /// 脚本引擎
    #[allow(dead_code)]
    engine: Arc<RhaiScriptEngine>,
    /// 已加载的节点
    nodes: HashMap<String, ScriptWorkflowNode>,
    /// 工作流定义
    definition: ScriptWorkflowDefinition,
    /// 执行状态
    state: Arc<RwLock<WorkflowExecutionState>>,
}

/// 工作流执行状态
#[derive(Debug, Clone, Default)]
pub struct WorkflowExecutionState {
    /// 当前节点 ID
    pub current_node: Option<String>,
    /// 节点输出
    pub node_outputs: HashMap<String, serde_json::Value>,
    /// 全局变量
    pub variables: HashMap<String, serde_json::Value>,
    /// 执行历史
    pub execution_history: Vec<String>,
    /// 是否完成
    pub completed: bool,
    /// 最终结果
    pub final_result: Option<serde_json::Value>,
    /// 错误信息
    pub error: Option<String>,
}

impl ScriptWorkflowExecutor {
    /// 创建工作流执行器
    pub async fn new(
        definition: ScriptWorkflowDefinition,
        engine_config: ScriptEngineConfig,
    ) -> Result<Self> {
        let engine = Arc::new(RhaiScriptEngine::new(engine_config)?);
        let mut nodes = HashMap::new();

        // 创建所有节点
        for node_config in &definition.nodes {
            let node = ScriptWorkflowNode::new(node_config.clone(), engine.clone()).await?;
            nodes.insert(node_config.id.clone(), node);
        }

        // 初始化状态
        let mut state = WorkflowExecutionState::default();
        state.variables = definition.global_variables.clone();

        Ok(Self {
            engine,
            nodes,
            definition,
            state: Arc::new(RwLock::new(state)),
        })
    }

    /// 执行工作流
    pub async fn execute(&self, input: serde_json::Value) -> Result<serde_json::Value> {
        let mut state = self.state.write().await;
        state.current_node = Some(self.definition.start_node.clone());
        state.variables.insert("input".to_string(), input.clone());

        let mut current_value = input;

        while let Some(ref node_id) = state.current_node.clone() {
            // 获取节点
            let node = self.nodes.get(node_id).ok_or_else(|| {
                anyhow!("Node not found: {}", node_id)
            })?;

            // 检查是否为结束节点
            if self.definition.end_nodes.contains(node_id) {
                // 执行结束节点的脚本
                let result = node.execute(current_value.clone()).await?;

                if !result.success {
                    state.error = result.error;
                    return Err(anyhow!("Node {} execution failed", node_id));
                }

                // 保存节点输出
                state.node_outputs.insert(node_id.clone(), result.output.clone());

                state.completed = true;
                state.final_result = Some(result.output.clone());
                break;
            }

            // 记录执行历史
            state.execution_history.push(node_id.clone());

            // 执行节点
            let result = node.execute(current_value.clone()).await?;

            if !result.success {
                let error = result.error.clone(); // Clone the error before moving it
                state.error = error.clone();
                let error_detail = error.unwrap_or_else(|| "unknown error".to_string());
                return Err(anyhow!("Node {} execution failed: {}", node_id, error_detail));
            }

            // 保存节点输出
            state.node_outputs.insert(node_id.clone(), result.output.clone());
            current_value = result.output;

            // 确定下一个节点
            let next_node = self.determine_next_node(node_id, &current_value).await?;
            state.current_node = next_node;
        }

        Ok(state.final_result.clone().unwrap_or(serde_json::Value::Null))
    }

    /// 确定下一个节点
    async fn determine_next_node(
        &self,
        current_node_id: &str,
        output: &serde_json::Value,
    ) -> Result<Option<String>> {
        // 查找从当前节点出发的边
        let candidate_edges: Vec<_> = self.definition.edges
            .iter()
            .filter(|(from, _, _)| from == current_node_id)
            .collect();

        if candidate_edges.is_empty() {
            return Ok(None);
        }

        // 如果只有一条边，直接返回
        if candidate_edges.len() == 1 && candidate_edges[0].2.is_none() {
            return Ok(Some(candidate_edges[0].1.clone()));
        }

        // 检查条件边
        for (_, to, condition) in &candidate_edges {
            if let Some(cond) = condition {
                // 评估条件
                // Parse and evaluate the condition (supports expressions like "rating == \"excellent\"")
                let condition_value = {
                    // Simple implementation for equality checks on object fields
                    if cond.contains("==") {
                        let parts: Vec<_> = cond.split("==").map(|s| s.trim().replace("\"", "")).collect();
                        if parts.len() == 2 {
                            let field = parts[0].clone();
                            let value = parts[1].clone();

                            // Try to get the field from the output object
                            match output {
                                serde_json::Value::Object(obj) => {
                                    if let Some(serde_json::Value::String(v)) = obj.get(&field) {
                                        *v == value
                                    } else if let Some(serde_json::Value::Number(n)) = obj.get(&field) {
                                        n.to_string() == value
                                    } else {
                                        false
                                    }
                                }
                                _ => false,
                            }
                        } else {
                            // Fall back to original comparison
                            match output {
                                serde_json::Value::String(s) => s == cond,
                                serde_json::Value::Bool(b) => (*b && cond == "true") || (!*b && cond == "false"),
                                _ => false,
                            }
                        }
                    } else {
                        // Fall back to original comparison
                        match output {
                            serde_json::Value::String(s) => s == cond,
                            serde_json::Value::Bool(b) => (*b && cond == "true") || (!*b && cond == "false"),
                            _ => false,
                        }
                    }
                };

                if condition_value {
                    return Ok(Some(to.clone()));
                }
            }
        }

        // 返回无条件边（如果存在）
        for (_, to, condition) in &candidate_edges {
            if condition.is_none() {
                return Ok(Some(to.clone()));
            }
        }

        Ok(None)
    }

    /// 获取执行状态
    pub async fn state(&self) -> WorkflowExecutionState {
        self.state.read().await.clone()
    }

    /// 重置执行器
    pub async fn reset(&self) {
        let mut state = self.state.write().await;
        *state = WorkflowExecutionState::default();
        state.variables = self.definition.global_variables.clone();
    }
}

// ============================================================================
// 便捷函数
// ============================================================================

/// 创建任务脚本节点
pub fn task_script(id: &str, name: &str, script: &str) -> ScriptNodeConfig {
    ScriptNodeConfig::new(id, name, ScriptNodeType::Task).with_source(script)
}

/// 创建条件脚本节点
pub fn condition_script(id: &str, name: &str, script: &str) -> ScriptNodeConfig {
    ScriptNodeConfig::new(id, name, ScriptNodeType::Condition).with_source(script)
}

/// 创建转换脚本节点
pub fn transform_script(id: &str, name: &str, script: &str) -> ScriptNodeConfig {
    ScriptNodeConfig::new(id, name, ScriptNodeType::Transform).with_source(script)
}

/// 创建验证脚本节点
pub fn validator_script(id: &str, name: &str, script: &str) -> ScriptNodeConfig {
    ScriptNodeConfig::new(id, name, ScriptNodeType::Validator).with_source(script)
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_script_node_execution() {
        let engine = Arc::new(RhaiScriptEngine::new(ScriptEngineConfig::default()).unwrap());

        let config = task_script(
            "double_node",
            "Double Value",
            r#"
                let result = input * 2;
                result
            "#,
        );

        let node = ScriptWorkflowNode::new(config, engine).await.unwrap();
        let result = node.execute(serde_json::json!(21)).await.unwrap();

        assert!(result.success);
        assert_eq!(result.output, serde_json::json!(42));
    }

    #[tokio::test]
    async fn test_condition_node() {
        let engine = Arc::new(RhaiScriptEngine::new(ScriptEngineConfig::default()).unwrap());

        let config = condition_script(
            "check_positive",
            "Check Positive",
            "input > 0",
        );

        let node = ScriptWorkflowNode::new(config, engine).await.unwrap();

        assert!(node.execute_as_condition(serde_json::json!(10)).await.unwrap());
        assert!(!node.execute_as_condition(serde_json::json!(-5)).await.unwrap());
    }

    #[tokio::test]
    async fn test_workflow_definition() {
        let mut workflow = ScriptWorkflowDefinition::new("test_wf", "Test Workflow");

        workflow
            .add_node(task_script("start", "Start", "input"))
            .add_node(task_script("process", "Process", "input * 2"))
            .add_node(task_script("end", "End", "input"))
            .add_edge("start", "process")
            .add_edge("process", "end")
            .set_start("start")
            .add_end("end");

        let errors = workflow.validate().unwrap();
        assert!(errors.is_empty(), "Validation errors: {:?}", errors);
    }

    #[tokio::test]
    async fn test_simple_workflow_execution() {
        let mut workflow = ScriptWorkflowDefinition::new("calc_wf", "Calculator Workflow");

        workflow
            .add_node(task_script("double", "Double", "input * 2"))
            .add_node(task_script("add_ten", "Add Ten", "input + 10"))
            .add_node(task_script("done", "Done", "input"))
            .add_edge("double", "add_ten")
            .add_edge("add_ten", "done")
            .set_start("double")
            .add_end("done");

        let executor = ScriptWorkflowExecutor::new(workflow, ScriptEngineConfig::default())
            .await
            .unwrap();

        let result = executor.execute(serde_json::json!(5)).await.unwrap();
        // 5 * 2 = 10, 10 + 10 = 20
        assert_eq!(result, serde_json::json!(20));
    }

    #[tokio::test]
    async fn test_conditional_workflow() {
        let mut workflow = ScriptWorkflowDefinition::new("cond_wf", "Conditional Workflow");

        workflow
            .add_node(condition_script(
                "check",
                "Check Value",
                r#"if input > 10 { "high" } else { "low" }"#,
            ))
            .add_node(task_script("high_path", "High Path", r#""HIGH: " + input"#))
            .add_node(task_script("low_path", "Low Path", r#""LOW: " + input"#))
            .add_node(task_script("end", "End", "input"))
            .add_conditional_edge("check", "high_path", "high")
            .add_conditional_edge("check", "low_path", "low")
            .add_edge("high_path", "end")
            .add_edge("low_path", "end")
            .set_start("check")
            .add_end("end");

        let executor = ScriptWorkflowExecutor::new(workflow, ScriptEngineConfig::default())
            .await
            .unwrap();

        let result = executor.execute(serde_json::json!(20)).await.unwrap();
        assert!(result.as_str().unwrap().starts_with("HIGH:"));

        executor.reset().await;

        let result = executor.execute(serde_json::json!(5)).await.unwrap();
        assert!(result.as_str().unwrap().starts_with("LOW:"));
    }
}
