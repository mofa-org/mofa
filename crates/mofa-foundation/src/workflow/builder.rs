//! 工作流构建器
//!
//! 提供流式 API 构建工作流

use super::graph::{EdgeConfig, WorkflowGraph};
use super::node::{RetryPolicy, WorkflowNode};
use super::state::WorkflowValue;
use std::collections::HashMap;
use std::future::Future;

/// 工作流构建器
pub struct WorkflowBuilder {
    graph: WorkflowGraph,
    current_node: Option<String>,
}

impl WorkflowBuilder {
    /// 创建新的工作流构建器
    pub fn new(id: &str, name: &str) -> Self {
        Self {
            graph: WorkflowGraph::new(id, name),
            current_node: None,
        }
    }

    /// 设置描述
    pub fn description(mut self, desc: &str) -> Self {
        self.graph = self.graph.with_description(desc);
        self
    }

    /// 添加开始节点
    pub fn start(mut self) -> Self {
        let node = WorkflowNode::start("start");
        self.graph.add_node(node);
        self.current_node = Some("start".to_string());
        self
    }

    /// 添加开始节点（自定义 ID）
    pub fn start_with_id(mut self, id: &str) -> Self {
        let node = WorkflowNode::start(id);
        self.graph.add_node(node);
        self.current_node = Some(id.to_string());
        self
    }

    /// 添加结束节点
    pub fn end(mut self) -> Self {
        let node = WorkflowNode::end("end");
        self.graph.add_node(node);

        // 连接当前节点到结束节点
        if let Some(ref current) = self.current_node {
            self.graph.connect(current, "end");
        }

        self.current_node = Some("end".to_string());
        self
    }

    /// 添加结束节点（自定义 ID）
    pub fn end_with_id(mut self, id: &str) -> Self {
        let node = WorkflowNode::end(id);
        self.graph.add_node(node);

        if let Some(ref current) = self.current_node {
            self.graph.connect(current, id);
        }

        self.current_node = Some(id.to_string());
        self
    }

    /// 添加任务节点
    pub fn task<F, Fut>(mut self, id: &str, name: &str, executor: F) -> Self
    where
        F: Fn(super::state::WorkflowContext, WorkflowValue) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<WorkflowValue, String>> + Send + 'static,
    {
        let node = WorkflowNode::task(id, name, executor);
        self.graph.add_node(node);

        // 连接当前节点
        if let Some(ref current) = self.current_node {
            self.graph.connect(current, id);
        }

        self.current_node = Some(id.to_string());
        self
    }

    /// 添加任务节点（带配置）
    pub fn task_with_config<F, Fut>(
        mut self,
        id: &str,
        name: &str,
        executor: F,
        retry: RetryPolicy,
        timeout_ms: u64,
    ) -> Self
    where
        F: Fn(super::state::WorkflowContext, WorkflowValue) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<WorkflowValue, String>> + Send + 'static,
    {
        let node = WorkflowNode::task(id, name, executor)
            .with_retry(retry)
            .with_timeout(timeout_ms);
        self.graph.add_node(node);

        if let Some(ref current) = self.current_node {
            self.graph.connect(current, id);
        }

        self.current_node = Some(id.to_string());
        self
    }

    /// 添加智能体节点
    pub fn agent<F, Fut>(mut self, id: &str, name: &str, agent_fn: F) -> Self
    where
        F: Fn(super::state::WorkflowContext, WorkflowValue) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<WorkflowValue, String>> + Send + 'static,
    {
        let node = WorkflowNode::agent(id, name, agent_fn);
        self.graph.add_node(node);

        if let Some(ref current) = self.current_node {
            self.graph.connect(current, id);
        }

        self.current_node = Some(id.to_string());
        self
    }

    /// 添加条件节点
    pub fn condition<F, Fut>(mut self, id: &str, name: &str, condition_fn: F) -> ConditionBuilder
    where
        F: Fn(super::state::WorkflowContext, WorkflowValue) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = bool> + Send + 'static,
    {
        let node = WorkflowNode::condition(id, name, condition_fn);
        self.graph.add_node(node);

        if let Some(ref current) = self.current_node {
            self.graph.connect(current, id);
        }

        ConditionBuilder {
            parent: self,
            condition_node: id.to_string(),
            true_branch: None,
            false_branch: None,
        }
    }

    /// 添加并行节点
    pub fn parallel(mut self, id: &str, name: &str) -> ParallelBuilder {
        let node = WorkflowNode::parallel(id, name, vec![]);
        self.graph.add_node(node);

        if let Some(ref current) = self.current_node {
            self.graph.connect(current, id);
        }

        ParallelBuilder {
            parent: self,
            parallel_node: id.to_string(),
            branches: Vec::new(),
        }
    }

    /// 添加循环节点
    pub fn loop_node<F, Fut, C, CFut>(
        mut self,
        id: &str,
        name: &str,
        body: F,
        condition: C,
        max_iterations: u32,
    ) -> Self
    where
        F: Fn(super::state::WorkflowContext, WorkflowValue) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<WorkflowValue, String>> + Send + 'static,
        C: Fn(super::state::WorkflowContext, WorkflowValue) -> CFut + Send + Sync + 'static,
        CFut: Future<Output = bool> + Send + 'static,
    {
        let node = WorkflowNode::loop_node(id, name, body, condition, max_iterations);
        self.graph.add_node(node);

        if let Some(ref current) = self.current_node {
            self.graph.connect(current, id);
        }

        self.current_node = Some(id.to_string());
        self
    }

    /// 添加子工作流节点
    pub fn sub_workflow(mut self, id: &str, name: &str, sub_workflow_id: &str) -> Self {
        let node = WorkflowNode::sub_workflow(id, name, sub_workflow_id);
        self.graph.add_node(node);

        if let Some(ref current) = self.current_node {
            self.graph.connect(current, id);
        }

        self.current_node = Some(id.to_string());
        self
    }

    /// 添加等待节点
    pub fn wait(mut self, id: &str, name: &str, event_type: &str) -> Self {
        let node = WorkflowNode::wait(id, name, event_type);
        self.graph.add_node(node);

        if let Some(ref current) = self.current_node {
            self.graph.connect(current, id);
        }

        self.current_node = Some(id.to_string());
        self
    }

    /// 添加数据转换节点
    pub fn transform<F, Fut>(mut self, id: &str, name: &str, transform_fn: F) -> Self
    where
        F: Fn(HashMap<String, WorkflowValue>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = WorkflowValue> + Send + 'static,
    {
        let node = WorkflowNode::transform(id, name, transform_fn);
        self.graph.add_node(node);

        if let Some(ref current) = self.current_node {
            self.graph.connect(current, id);
        }

        self.current_node = Some(id.to_string());
        self
    }

    /// 添加自定义节点
    pub fn node(mut self, node: WorkflowNode) -> Self {
        let node_id = node.id().to_string();
        self.graph.add_node(node);

        if let Some(ref current) = self.current_node {
            self.graph.connect(current, &node_id);
        }

        self.current_node = Some(node_id);
        self
    }

    /// 添加边（不改变当前节点）
    pub fn edge(mut self, from: &str, to: &str) -> Self {
        self.graph.connect(from, to);
        self
    }

    /// 添加条件边
    pub fn conditional_edge(mut self, from: &str, to: &str, condition: &str) -> Self {
        self.graph.connect_conditional(from, to, condition);
        self
    }

    /// 添加错误处理边
    pub fn error_edge(mut self, from: &str, to: &str) -> Self {
        self.graph.add_edge(EdgeConfig::error(from, to));
        self
    }

    /// 跳转到指定节点（设置当前节点）
    pub fn goto(mut self, node_id: &str) -> Self {
        self.current_node = Some(node_id.to_string());
        self
    }

    /// 从当前节点连接到指定节点
    pub fn then(mut self, node_id: &str) -> Self {
        if let Some(ref current) = self.current_node {
            self.graph.connect(current, node_id);
        }
        self.current_node = Some(node_id.to_string());
        self
    }

    /// 构建工作流图
    pub fn build(self) -> WorkflowGraph {
        self.graph
    }

    /// 验证并构建
    pub fn build_validated(self) -> Result<WorkflowGraph, Vec<String>> {
        self.graph.validate()?;
        Ok(self.graph)
    }
}

/// 条件构建器
pub struct ConditionBuilder {
    parent: WorkflowBuilder,
    condition_node: String,
    true_branch: Option<String>,
    false_branch: Option<String>,
}

impl ConditionBuilder {
    /// 设置为真时的分支
    pub fn on_true<F, Fut>(mut self, id: &str, name: &str, executor: F) -> Self
    where
        F: Fn(super::state::WorkflowContext, WorkflowValue) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<WorkflowValue, String>> + Send + 'static,
    {
        let node = WorkflowNode::task(id, name, executor);
        self.parent.graph.add_node(node);
        self.parent
            .graph
            .connect_conditional(&self.condition_node, id, "true");
        self.true_branch = Some(id.to_string());
        self
    }

    /// 设置为假时的分支
    pub fn on_false<F, Fut>(mut self, id: &str, name: &str, executor: F) -> Self
    where
        F: Fn(super::state::WorkflowContext, WorkflowValue) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<WorkflowValue, String>> + Send + 'static,
    {
        let node = WorkflowNode::task(id, name, executor);
        self.parent.graph.add_node(node);
        self.parent
            .graph
            .connect_conditional(&self.condition_node, id, "false");
        self.false_branch = Some(id.to_string());
        self
    }

    /// 汇聚两个分支
    pub fn merge(mut self, id: &str, name: &str) -> WorkflowBuilder {
        let node = WorkflowNode::join(
            id,
            name,
            vec![
                self.true_branch.as_deref().unwrap_or(""),
                self.false_branch.as_deref().unwrap_or(""),
            ]
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect(),
        );
        self.parent.graph.add_node(node);

        if let Some(ref true_branch) = self.true_branch {
            self.parent.graph.connect(true_branch, id);
        }
        if let Some(ref false_branch) = self.false_branch {
            self.parent.graph.connect(false_branch, id);
        }

        self.parent.current_node = Some(id.to_string());
        self.parent
    }

    /// 不汇聚，返回构建器
    pub fn end_condition(mut self) -> WorkflowBuilder {
        // 设置当前节点为最后添加的分支
        self.parent.current_node = self.true_branch.or(self.false_branch);
        self.parent
    }
}

/// 并行构建器
pub struct ParallelBuilder {
    parent: WorkflowBuilder,
    parallel_node: String,
    branches: Vec<String>,
}

impl ParallelBuilder {
    /// 添加分支任务
    pub fn branch<F, Fut>(mut self, id: &str, name: &str, executor: F) -> Self
    where
        F: Fn(super::state::WorkflowContext, WorkflowValue) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<WorkflowValue, String>> + Send + 'static,
    {
        let node = WorkflowNode::task(id, name, executor);
        self.parent.graph.add_node(node);
        self.parent.graph.connect(&self.parallel_node, id);
        self.branches.push(id.to_string());
        self
    }

    /// 添加分支智能体
    pub fn branch_agent<F, Fut>(mut self, id: &str, name: &str, agent_fn: F) -> Self
    where
        F: Fn(super::state::WorkflowContext, WorkflowValue) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<WorkflowValue, String>> + Send + 'static,
    {
        let node = WorkflowNode::agent(id, name, agent_fn);
        self.parent.graph.add_node(node);
        self.parent.graph.connect(&self.parallel_node, id);
        self.branches.push(id.to_string());
        self
    }

    /// 汇聚所有分支
    pub fn join(mut self, id: &str, name: &str) -> WorkflowBuilder {
        let node = WorkflowNode::join(id, name, self.branches.iter().map(|s| s.as_str()).collect());
        self.parent.graph.add_node(node);

        for branch in &self.branches {
            self.parent.graph.connect(branch, id);
        }

        self.parent.current_node = Some(id.to_string());
        self.parent
    }

    /// 汇聚并转换
    pub fn join_with_transform<F, Fut>(
        mut self,
        id: &str,
        name: &str,
        transform: F,
    ) -> WorkflowBuilder
    where
        F: Fn(HashMap<String, WorkflowValue>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = WorkflowValue> + Send + 'static,
    {
        let node = WorkflowNode::join_with_transform(
            id,
            name,
            self.branches.iter().map(|s| s.as_str()).collect(),
            transform,
        );
        self.parent.graph.add_node(node);

        for branch in &self.branches {
            self.parent.graph.connect(branch, id);
        }

        self.parent.current_node = Some(id.to_string());
        self.parent
    }
}

/// 简化的工作流构建宏
#[macro_export]
macro_rules! workflow {
    ($id:expr, $name:expr => {
        $($body:tt)*
    }) => {
        WorkflowBuilder::new($id, $name)
            $($body)*
            .build()
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workflow_builder() {
        let graph = WorkflowBuilder::new("test", "Test Workflow")
            .start()
            .task("task1", "Task 1", |_ctx, input| async move { Ok(input) })
            .task("task2", "Task 2", |_ctx, input| async move { Ok(input) })
            .end()
            .build();

        assert_eq!(graph.node_count(), 4);
        assert_eq!(graph.edge_count(), 3);
    }

    #[test]
    fn test_condition_builder() {
        let graph = WorkflowBuilder::new("test", "Conditional Workflow")
            .start()
            .condition("check", "Check", |_ctx, input| async move {
                input.as_i64().unwrap_or(0) > 10
            })
            .on_true("high", "High", |_ctx, _input| async move {
                Ok(WorkflowValue::String("high".to_string()))
            })
            .on_false("low", "Low", |_ctx, _input| async move {
                Ok(WorkflowValue::String("low".to_string()))
            })
            .merge("merge", "Merge")
            .end()
            .build();

        assert_eq!(graph.node_count(), 6);
    }

    #[test]
    fn test_parallel_builder() {
        let graph = WorkflowBuilder::new("test", "Parallel Workflow")
            .start()
            .parallel("fork", "Fork")
            .branch("a", "Branch A", |_ctx, _input| async move {
                Ok(WorkflowValue::String("a".to_string()))
            })
            .branch("b", "Branch B", |_ctx, _input| async move {
                Ok(WorkflowValue::String("b".to_string()))
            })
            .branch("c", "Branch C", |_ctx, _input| async move {
                Ok(WorkflowValue::String("c".to_string()))
            })
            .join("join", "Join")
            .end()
            .build();

        assert_eq!(graph.node_count(), 7);
    }
}
