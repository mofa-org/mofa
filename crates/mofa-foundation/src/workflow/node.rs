//! 工作流节点定义
//!
//! 定义各种工作流节点类型

use super::state::{NodeResult, WorkflowContext, WorkflowValue};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tracing::{debug, info, warn};

/// 节点执行函数类型
pub type NodeExecutorFn = Arc<
    dyn Fn(
            WorkflowContext,
            WorkflowValue,
        ) -> Pin<Box<dyn Future<Output = Result<WorkflowValue, String>> + Send>>
        + Send
        + Sync,
>;

/// 条件判断函数类型
pub type ConditionFn = Arc<
    dyn Fn(WorkflowContext, WorkflowValue) -> Pin<Box<dyn Future<Output = bool> + Send>>
        + Send
        + Sync,
>;

/// 数据转换函数类型
pub type TransformFn = Arc<
    dyn Fn(HashMap<String, WorkflowValue>) -> Pin<Box<dyn Future<Output = WorkflowValue> + Send>>
        + Send
        + Sync,
>;

/// 节点类型枚举
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeType {
    /// 开始节点
    Start,
    /// 结束节点
    End,
    /// 任务节点 - 执行具体任务
    Task,
    /// 智能体节点 - 调用智能体
    Agent,
    /// 条件节点 - 分支判断
    Condition,
    /// 并行节点 - 并行执行多个分支
    Parallel,
    /// 聚合节点 - 等待多个分支完成
    Join,
    /// 循环节点 - 循环执行
    Loop,
    /// 子工作流节点
    SubWorkflow,
    /// 等待节点 - 等待外部事件
    Wait,
    /// 转换节点 - 数据转换
    Transform,
}

/// 重试策略
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    /// 最大重试次数
    pub max_retries: u32,
    /// 重试间隔（毫秒）
    pub retry_delay_ms: u64,
    /// 是否指数退避
    pub exponential_backoff: bool,
    /// 最大重试间隔（毫秒）
    pub max_delay_ms: u64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            retry_delay_ms: 1000,
            exponential_backoff: true,
            max_delay_ms: 30000,
        }
    }
}

impl RetryPolicy {
    pub fn no_retry() -> Self {
        Self {
            max_retries: 0,
            ..Default::default()
        }
    }

    pub fn with_retries(max_retries: u32) -> Self {
        Self {
            max_retries,
            ..Default::default()
        }
    }

    /// 计算第 n 次重试的延迟
    pub fn get_delay(&self, retry_count: u32) -> u64 {
        if self.exponential_backoff {
            let delay = self.retry_delay_ms * 2u64.pow(retry_count);
            delay.min(self.max_delay_ms)
        } else {
            self.retry_delay_ms
        }
    }
}

/// 超时配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeoutConfig {
    /// 执行超时（毫秒）
    pub execution_timeout_ms: u64,
    /// 是否在超时时取消
    pub cancel_on_timeout: bool,
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self {
            execution_timeout_ms: 60000, // 1 分钟
            cancel_on_timeout: true,
        }
    }
}

/// 节点配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeConfig {
    /// 节点 ID
    pub id: String,
    /// 节点名称
    pub name: String,
    /// 节点类型
    pub node_type: NodeType,
    /// 节点描述
    pub description: String,
    /// 重试策略
    pub retry_policy: RetryPolicy,
    /// 超时配置
    pub timeout: TimeoutConfig,
    /// 自定义元数据
    pub metadata: HashMap<String, String>,
}

impl NodeConfig {
    pub fn new(id: &str, name: &str, node_type: NodeType) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            node_type,
            description: String::new(),
            retry_policy: RetryPolicy::default(),
            timeout: TimeoutConfig::default(),
            metadata: HashMap::new(),
        }
    }

    pub fn with_description(mut self, desc: &str) -> Self {
        self.description = desc.to_string();
        self
    }

    pub fn with_retry_policy(mut self, policy: RetryPolicy) -> Self {
        self.retry_policy = policy;
        self
    }

    pub fn with_timeout(mut self, timeout: TimeoutConfig) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }
}

/// 工作流节点
pub struct WorkflowNode {
    /// 节点配置
    pub config: NodeConfig,
    /// 节点执行器（根据类型不同）
    executor: Option<NodeExecutorFn>,
    /// 条件函数（用于条件节点）
    condition: Option<ConditionFn>,
    /// 数据转换函数
    transform: Option<TransformFn>,
    /// 循环条件（用于循环节点）
    loop_condition: Option<ConditionFn>,
    /// 最大循环次数
    max_iterations: Option<u32>,
    /// 并行分支 ID 列表
    parallel_branches: Vec<String>,
    /// 聚合等待的节点 ID 列表
    join_nodes: Vec<String>,
    /// 子工作流 ID
    sub_workflow_id: Option<String>,
    /// 等待事件类型
    wait_event_type: Option<String>,
    /// 条件分支映射：条件名 -> 目标节点 ID
    condition_branches: HashMap<String, String>,
}

impl WorkflowNode {
    /// 创建开始节点
    pub fn start(id: &str) -> Self {
        Self {
            config: NodeConfig::new(id, "Start", NodeType::Start),
            executor: None,
            condition: None,
            transform: None,
            loop_condition: None,
            max_iterations: None,
            parallel_branches: Vec::new(),
            join_nodes: Vec::new(),
            sub_workflow_id: None,
            wait_event_type: None,
            condition_branches: HashMap::new(),
        }
    }

    /// 创建结束节点
    pub fn end(id: &str) -> Self {
        Self {
            config: NodeConfig::new(id, "End", NodeType::End),
            executor: None,
            condition: None,
            transform: None,
            loop_condition: None,
            max_iterations: None,
            parallel_branches: Vec::new(),
            join_nodes: Vec::new(),
            sub_workflow_id: None,
            wait_event_type: None,
            condition_branches: HashMap::new(),
        }
    }

    /// 创建任务节点
    pub fn task<F, Fut>(id: &str, name: &str, executor: F) -> Self
    where
        F: Fn(WorkflowContext, WorkflowValue) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<WorkflowValue, String>> + Send + 'static,
    {
        Self {
            config: NodeConfig::new(id, name, NodeType::Task),
            executor: Some(Arc::new(move |ctx, input| Box::pin(executor(ctx, input)))),
            condition: None,
            transform: None,
            loop_condition: None,
            max_iterations: None,
            parallel_branches: Vec::new(),
            join_nodes: Vec::new(),
            sub_workflow_id: None,
            wait_event_type: None,
            condition_branches: HashMap::new(),
        }
    }

    /// 创建智能体节点
    pub fn agent<F, Fut>(id: &str, name: &str, agent_executor: F) -> Self
    where
        F: Fn(WorkflowContext, WorkflowValue) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<WorkflowValue, String>> + Send + 'static,
    {
        Self {
            config: NodeConfig::new(id, name, NodeType::Agent),
            executor: Some(Arc::new(move |ctx, input| {
                Box::pin(agent_executor(ctx, input))
            })),
            condition: None,
            transform: None,
            loop_condition: None,
            max_iterations: None,
            parallel_branches: Vec::new(),
            join_nodes: Vec::new(),
            sub_workflow_id: None,
            wait_event_type: None,
            condition_branches: HashMap::new(),
        }
    }

    /// 创建条件节点
    pub fn condition<F, Fut>(id: &str, name: &str, condition_fn: F) -> Self
    where
        F: Fn(WorkflowContext, WorkflowValue) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = bool> + Send + 'static,
    {
        Self {
            config: NodeConfig::new(id, name, NodeType::Condition),
            executor: None,
            condition: Some(Arc::new(move |ctx, input| {
                Box::pin(condition_fn(ctx, input))
            })),
            transform: None,
            loop_condition: None,
            max_iterations: None,
            parallel_branches: Vec::new(),
            join_nodes: Vec::new(),
            sub_workflow_id: None,
            wait_event_type: None,
            condition_branches: HashMap::new(),
        }
    }

    /// 创建并行节点
    pub fn parallel(id: &str, name: &str, branches: Vec<&str>) -> Self {
        Self {
            config: NodeConfig::new(id, name, NodeType::Parallel),
            executor: None,
            condition: None,
            transform: None,
            loop_condition: None,
            max_iterations: None,
            parallel_branches: branches.into_iter().map(|s| s.to_string()).collect(),
            join_nodes: Vec::new(),
            sub_workflow_id: None,
            wait_event_type: None,
            condition_branches: HashMap::new(),
        }
    }

    /// 创建聚合节点
    pub fn join(id: &str, name: &str, wait_for: Vec<&str>) -> Self {
        Self {
            config: NodeConfig::new(id, name, NodeType::Join),
            executor: None,
            condition: None,
            transform: None,
            loop_condition: None,
            max_iterations: None,
            parallel_branches: Vec::new(),
            join_nodes: wait_for.into_iter().map(|s| s.to_string()).collect(),
            sub_workflow_id: None,
            wait_event_type: None,
            condition_branches: HashMap::new(),
        }
    }

    /// 创建聚合节点（带转换函数）
    pub fn join_with_transform<F, Fut>(
        id: &str,
        name: &str,
        wait_for: Vec<&str>,
        transform: F,
    ) -> Self
    where
        F: Fn(HashMap<String, WorkflowValue>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = WorkflowValue> + Send + 'static,
    {
        Self {
            config: NodeConfig::new(id, name, NodeType::Join),
            executor: None,
            condition: None,
            transform: Some(Arc::new(move |inputs| Box::pin(transform(inputs)))),
            loop_condition: None,
            max_iterations: None,
            parallel_branches: Vec::new(),
            join_nodes: wait_for.into_iter().map(|s| s.to_string()).collect(),
            sub_workflow_id: None,
            wait_event_type: None,
            condition_branches: HashMap::new(),
        }
    }

    /// 创建循环节点
    pub fn loop_node<F, Fut, C, CFut>(
        id: &str,
        name: &str,
        body: F,
        condition: C,
        max_iterations: u32,
    ) -> Self
    where
        F: Fn(WorkflowContext, WorkflowValue) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<WorkflowValue, String>> + Send + 'static,
        C: Fn(WorkflowContext, WorkflowValue) -> CFut + Send + Sync + 'static,
        CFut: Future<Output = bool> + Send + 'static,
    {
        Self {
            config: NodeConfig::new(id, name, NodeType::Loop),
            executor: Some(Arc::new(move |ctx, input| Box::pin(body(ctx, input)))),
            condition: None,
            transform: None,
            loop_condition: Some(Arc::new(move |ctx, input| Box::pin(condition(ctx, input)))),
            max_iterations: Some(max_iterations),
            parallel_branches: Vec::new(),
            join_nodes: Vec::new(),
            sub_workflow_id: None,
            wait_event_type: None,
            condition_branches: HashMap::new(),
        }
    }

    /// 创建子工作流节点
    pub fn sub_workflow(id: &str, name: &str, sub_workflow_id: &str) -> Self {
        Self {
            config: NodeConfig::new(id, name, NodeType::SubWorkflow),
            executor: None,
            condition: None,
            transform: None,
            loop_condition: None,
            max_iterations: None,
            parallel_branches: Vec::new(),
            join_nodes: Vec::new(),
            sub_workflow_id: Some(sub_workflow_id.to_string()),
            wait_event_type: None,
            condition_branches: HashMap::new(),
        }
    }

    /// 创建等待节点
    pub fn wait(id: &str, name: &str, event_type: &str) -> Self {
        Self {
            config: NodeConfig::new(id, name, NodeType::Wait),
            executor: None,
            condition: None,
            transform: None,
            loop_condition: None,
            max_iterations: None,
            parallel_branches: Vec::new(),
            join_nodes: Vec::new(),
            sub_workflow_id: None,
            wait_event_type: Some(event_type.to_string()),
            condition_branches: HashMap::new(),
        }
    }

    /// 创建数据转换节点
    pub fn transform<F, Fut>(id: &str, name: &str, transform_fn: F) -> Self
    where
        F: Fn(HashMap<String, WorkflowValue>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = WorkflowValue> + Send + 'static,
    {
        Self {
            config: NodeConfig::new(id, name, NodeType::Transform),
            executor: None,
            condition: None,
            transform: Some(Arc::new(move |inputs| Box::pin(transform_fn(inputs)))),
            loop_condition: None,
            max_iterations: None,
            parallel_branches: Vec::new(),
            join_nodes: Vec::new(),
            sub_workflow_id: None,
            wait_event_type: None,
            condition_branches: HashMap::new(),
        }
    }

    /// 设置描述
    pub fn with_description(mut self, desc: &str) -> Self {
        self.config.description = desc.to_string();
        self
    }

    /// 设置重试策略
    pub fn with_retry(mut self, policy: RetryPolicy) -> Self {
        self.config.retry_policy = policy;
        self
    }

    /// 设置超时
    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.config.timeout.execution_timeout_ms = timeout_ms;
        self
    }

    /// 添加条件分支
    pub fn with_branch(mut self, condition_name: &str, target_node_id: &str) -> Self {
        self.condition_branches
            .insert(condition_name.to_string(), target_node_id.to_string());
        self
    }

    /// 获取节点 ID
    pub fn id(&self) -> &str {
        &self.config.id
    }

    /// 获取节点类型
    pub fn node_type(&self) -> &NodeType {
        &self.config.node_type
    }

    /// 获取并行分支
    pub fn parallel_branches(&self) -> &[String] {
        &self.parallel_branches
    }

    /// 获取聚合等待节点
    pub fn join_nodes(&self) -> &[String] {
        &self.join_nodes
    }

    /// 获取条件分支
    pub fn condition_branches(&self) -> &HashMap<String, String> {
        &self.condition_branches
    }

    /// 获取子工作流 ID
    pub fn sub_workflow_id(&self) -> Option<&str> {
        self.sub_workflow_id.as_deref()
    }

    /// 获取等待事件类型
    pub fn wait_event_type(&self) -> Option<&str> {
        self.wait_event_type.as_deref()
    }

    /// 执行节点
    pub async fn execute(&self, ctx: &WorkflowContext, input: WorkflowValue) -> NodeResult {
        let start_time = std::time::Instant::now();
        let node_id = &self.config.id;

        info!("Executing node: {} ({})", node_id, self.config.name);

        match self.config.node_type {
            NodeType::Start => {
                // 开始节点直接传递输入
                NodeResult::success(node_id, input, start_time.elapsed().as_millis() as u64)
            }
            NodeType::End => {
                // 结束节点直接传递输入
                NodeResult::success(node_id, input, start_time.elapsed().as_millis() as u64)
            }
            NodeType::Task | NodeType::Agent => {
                self.execute_with_retry(ctx, input, start_time).await
            }
            NodeType::Condition => {
                // 条件节点评估条件
                if let Some(ref condition_fn) = self.condition {
                    let result = condition_fn(ctx.clone(), input.clone()).await;
                    let branch = if result { "true" } else { "false" };
                    debug!("Condition {} evaluated to: {}", node_id, branch);
                    NodeResult::success(
                        node_id,
                        WorkflowValue::String(branch.to_string()),
                        start_time.elapsed().as_millis() as u64,
                    )
                } else {
                    NodeResult::failed(node_id, "No condition function", 0)
                }
            }
            NodeType::Parallel => {
                // 并行节点只是标记，实际并行执行由 executor 处理
                NodeResult::success(
                    node_id,
                    WorkflowValue::List(
                        self.parallel_branches
                            .iter()
                            .map(|b| WorkflowValue::String(b.clone()))
                            .collect(),
                    ),
                    start_time.elapsed().as_millis() as u64,
                )
            }
            NodeType::Join => {
                // 聚合节点等待所有依赖完成
                let outputs = ctx
                    .get_node_outputs(
                        &self
                            .join_nodes
                            .iter()
                            .map(|s| s.as_str())
                            .collect::<Vec<_>>(),
                    )
                    .await;

                let result = if let Some(ref transform_fn) = self.transform {
                    transform_fn(outputs).await
                } else {
                    // 默认将所有输入合并为 Map
                    WorkflowValue::Map(outputs)
                };

                NodeResult::success(node_id, result, start_time.elapsed().as_millis() as u64)
            }
            NodeType::Loop => self.execute_loop(ctx, input, start_time).await,
            NodeType::Transform => {
                if let Some(ref transform_fn) = self.transform {
                    let mut inputs = HashMap::new();
                    inputs.insert("input".to_string(), input);
                    let result = transform_fn(inputs).await;
                    NodeResult::success(node_id, result, start_time.elapsed().as_millis() as u64)
                } else {
                    NodeResult::failed(node_id, "No transform function", 0)
                }
            }
            NodeType::SubWorkflow => {
                // 子工作流执行由 executor 处理
                NodeResult::success(node_id, input, start_time.elapsed().as_millis() as u64)
            }
            NodeType::Wait => {
                // 等待节点由 executor 处理
                NodeResult::success(node_id, input, start_time.elapsed().as_millis() as u64)
            }
        }
    }

    /// 带重试的执行
    async fn execute_with_retry(
        &self,
        ctx: &WorkflowContext,
        input: WorkflowValue,
        start_time: std::time::Instant,
    ) -> NodeResult {
        let node_id = &self.config.id;
        let policy = &self.config.retry_policy;

        let executor = match &self.executor {
            Some(e) => e,
            None => return NodeResult::failed(node_id, "No executor function", 0),
        };

        let mut retry_count = 0;
        loop {
            match executor(ctx.clone(), input.clone()).await {
                Ok(output) => {
                    let mut result = NodeResult::success(
                        node_id,
                        output,
                        start_time.elapsed().as_millis() as u64,
                    );
                    result.retry_count = retry_count;
                    return result;
                }
                Err(e) => {
                    if retry_count < policy.max_retries {
                        let delay = policy.get_delay(retry_count);
                        warn!(
                            "Node {} failed (attempt {}), retrying in {}ms: {}",
                            node_id,
                            retry_count + 1,
                            delay,
                            e
                        );
                        tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
                        retry_count += 1;
                    } else {
                        let mut result = NodeResult::failed(
                            node_id,
                            &e,
                            start_time.elapsed().as_millis() as u64,
                        );
                        result.retry_count = retry_count;
                        return result;
                    }
                }
            }
        }
    }

    /// 执行循环
    async fn execute_loop(
        &self,
        ctx: &WorkflowContext,
        mut input: WorkflowValue,
        start_time: std::time::Instant,
    ) -> NodeResult {
        let node_id = &self.config.id;
        let max_iter = self.max_iterations.unwrap_or(1000);

        let executor = match &self.executor {
            Some(e) => e,
            None => return NodeResult::failed(node_id, "No executor function", 0),
        };

        let condition = match &self.loop_condition {
            Some(c) => c,
            None => return NodeResult::failed(node_id, "No loop condition", 0),
        };

        let mut iteration = 0;
        while iteration < max_iter {
            // 检查条件
            if !condition(ctx.clone(), input.clone()).await {
                debug!(
                    "Loop {} condition false, exiting after {} iterations",
                    node_id, iteration
                );
                break;
            }

            // 执行循环体
            match executor(ctx.clone(), input.clone()).await {
                Ok(output) => {
                    input = output;
                    ctx.set_variable(
                        &format!("{}_iteration", node_id),
                        WorkflowValue::Int(iteration as i64),
                    )
                    .await;
                }
                Err(e) => {
                    return NodeResult::failed(
                        node_id,
                        &format!("Loop failed at iteration {}: {}", iteration, e),
                        start_time.elapsed().as_millis() as u64,
                    );
                }
            }

            iteration += 1;
        }

        if iteration >= max_iter {
            warn!("Loop {} reached max iterations: {}", node_id, max_iter);
        }

        NodeResult::success(node_id, input, start_time.elapsed().as_millis() as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_task_node() {
        let node = WorkflowNode::task("task1", "Test Task", |_ctx, input| async move {
            let value = input.as_i64().unwrap_or(0);
            Ok(WorkflowValue::Int(value * 2))
        });

        let ctx = WorkflowContext::new("test");
        let result = node.execute(&ctx, WorkflowValue::Int(21)).await;

        assert!(result.status.is_success());
        assert_eq!(result.output.as_i64(), Some(42));
    }

    #[tokio::test]
    async fn test_condition_node() {
        let node = WorkflowNode::condition("cond1", "Check Value", |_ctx, input| async move {
            input.as_i64().unwrap_or(0) > 10
        });

        let ctx = WorkflowContext::new("test");

        let result = node.execute(&ctx, WorkflowValue::Int(20)).await;
        assert_eq!(result.output.as_str(), Some("true"));

        let result = node.execute(&ctx, WorkflowValue::Int(5)).await;
        assert_eq!(result.output.as_str(), Some("false"));
    }

    #[tokio::test]
    async fn test_loop_node() {
        let node = WorkflowNode::loop_node(
            "loop1",
            "Counter Loop",
            |_ctx, input| async move {
                let value = input.as_i64().unwrap_or(0);
                Ok(WorkflowValue::Int(value + 1))
            },
            |_ctx, input| async move { input.as_i64().unwrap_or(0) < 5 },
            100,
        );

        let ctx = WorkflowContext::new("test");
        let result = node.execute(&ctx, WorkflowValue::Int(0)).await;

        assert!(result.status.is_success());
        assert_eq!(result.output.as_i64(), Some(5));
    }

    #[test]
    fn test_retry_policy() {
        let policy = RetryPolicy::default();

        assert_eq!(policy.get_delay(0), 1000);
        assert_eq!(policy.get_delay(1), 2000);
        assert_eq!(policy.get_delay(2), 4000);
        assert_eq!(policy.get_delay(10), 30000); // capped at max
    }
}
