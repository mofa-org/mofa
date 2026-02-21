//! 工作流执行器
//!
//! 负责工作流的执行调度
//!
//! Supports optional telemetry emission for the time-travel debugger.
//! When a `TelemetryEmitter` is attached via `with_telemetry()`, the
//! executor emits `DebugEvent`s at key execution points.

use super::graph::WorkflowGraph;
use super::node::{NodeType, WorkflowNode};
use super::state::{
    ExecutionCheckpoint, ExecutionRecord, NodeExecutionRecord, NodeResult, NodeStatus, WorkflowContext, WorkflowStatus,
    WorkflowValue,
};
use mofa_kernel::workflow::telemetry::{DebugEvent, TelemetryEmitter};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{RwLock, Semaphore, mpsc, oneshot};
use tracing::{error, info, warn};

/// 执行器配置
#[derive(Debug, Clone)]
pub struct ExecutorConfig {
    /// 最大并行度
    pub max_parallelism: usize,
    /// 是否在失败时停止
    pub stop_on_failure: bool,
    /// 是否启用检查点
    pub enable_checkpoints: bool,
    /// 检查点间隔（节点数）
    pub checkpoint_interval: usize,
    /// 执行超时（毫秒）
    pub execution_timeout_ms: Option<u64>,
}

impl Default for ExecutorConfig {
    fn default() -> Self {
        Self {
            max_parallelism: 10,
            stop_on_failure: true,
            enable_checkpoints: true,
            checkpoint_interval: 5,
            execution_timeout_ms: None,
        }
    }
}

/// 执行事件
#[derive(Debug, Clone)]
pub enum ExecutionEvent {
    /// 工作流开始
    WorkflowStarted {
        workflow_id: String,
        execution_id: String,
    },
    /// 工作流完成
    WorkflowCompleted {
        workflow_id: String,
        execution_id: String,
        status: WorkflowStatus,
    },
    /// 节点开始
    NodeStarted { node_id: String },
    /// 节点完成
    NodeCompleted { node_id: String, result: NodeResult },
    /// 节点失败
    NodeFailed { node_id: String, error: String },
    /// 检查点创建
    CheckpointCreated { label: String },
    /// 外部事件
    ExternalEvent {
        event_type: String,
        data: WorkflowValue,
    },
}

/// 工作流执行器
pub struct WorkflowExecutor {
    /// 执行器配置
    config: ExecutorConfig,
    /// 事件发送器
    event_tx: Option<mpsc::Sender<ExecutionEvent>>,
    /// Telemetry emitter for the time-travel debugger (optional)
    telemetry: Option<Arc<dyn TelemetryEmitter>>,
    /// 子工作流注册表
    sub_workflows: Arc<RwLock<HashMap<String, Arc<WorkflowGraph>>>>,
    /// 外部事件等待器
    event_waiters: Arc<RwLock<HashMap<String, Vec<oneshot::Sender<WorkflowValue>>>>>,
    /// 并行执行信号量
    semaphore: Arc<Semaphore>,
}

impl WorkflowExecutor {
    pub fn new(config: ExecutorConfig) -> Self {
        let semaphore = Arc::new(Semaphore::new(config.max_parallelism));
        Self {
            config,
            event_tx: None,
            telemetry: None,
            sub_workflows: Arc::new(RwLock::new(HashMap::new())),
            event_waiters: Arc::new(RwLock::new(HashMap::new())),
            semaphore,
        }
    }

    /// 设置事件发送器
    pub fn with_event_sender(mut self, tx: mpsc::Sender<ExecutionEvent>) -> Self {
        self.event_tx = Some(tx);
        self
    }

    /// Attach a telemetry emitter for time-travel debugger support.
    ///
    /// When set, the executor will emit `DebugEvent`s at key execution points
    /// (workflow start/end, node start/end).
    pub fn with_telemetry(mut self, emitter: Arc<dyn TelemetryEmitter>) -> Self {
        self.telemetry = Some(emitter);
        self
    }

    /// Emit a debug telemetry event (no-op if no emitter is set).
    async fn emit_debug(&self, event: DebugEvent) {
        if let Some(ref emitter) = self.telemetry
            && emitter.is_enabled()
        {
            emitter.emit(event).await;
        }
    }

    /// 注册子工作流
    pub async fn register_sub_workflow(&self, id: &str, graph: WorkflowGraph) {
        let mut workflows = self.sub_workflows.write().await;
        workflows.insert(id.to_string(), Arc::new(graph));
    }

    /// 发送执行事件
    async fn emit_event(&self, event: ExecutionEvent) {
        if let Some(ref tx) = self.event_tx {
            let _ = tx.send(event).await;
        }
    }

    /// 发送外部事件
    pub async fn send_external_event(&self, event_type: &str, data: WorkflowValue) {
        let mut waiters = self.event_waiters.write().await;
        if let Some(senders) = waiters.remove(event_type) {
            for sender in senders {
                let _ = sender.send(data.clone());
            }
        }
    }

    /// 执行工作流
    pub async fn execute(
        &self,
        graph: &WorkflowGraph,
        input: WorkflowValue,
    ) -> Result<ExecutionRecord, String> {
        let start_time = Instant::now();
        let ctx = WorkflowContext::new(&graph.id);
        ctx.set_input(input.clone()).await;

        // 发送开始事件
        self.emit_event(ExecutionEvent::WorkflowStarted {
            workflow_id: graph.id.clone(),
            execution_id: ctx.execution_id.clone(),
        })
        .await;

        // Emit debug telemetry: WorkflowStart
        self.emit_debug(DebugEvent::WorkflowStart {
            workflow_id: graph.id.clone(),
            execution_id: ctx.execution_id.clone(),
            timestamp_ms: DebugEvent::now_ms(),
        })
        .await;

        info!(
            "Starting workflow execution: {} ({})",
            graph.name, ctx.execution_id
        );

        // 验证图
        if let Err(errors) = graph.validate() {
            let error_msg = errors.join("; ");
            error!("Workflow validation failed: {}", error_msg);
            return Err(error_msg);
        }

        // 获取开始节点
        let start_node_id = graph
            .start_node()
            .ok_or_else(|| "No start node".to_string())?;

        // 执行工作流
        let mut execution_record = ExecutionRecord {
            execution_id: ctx.execution_id.clone(),
            workflow_id: graph.id.clone(),
            started_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            ended_at: None,
            status: WorkflowStatus::Running,
            node_records: Vec::new(),
            outputs: HashMap::new(),
        };

        // 使用基于依赖的执行
        let result = self
            .execute_from_node(graph, &ctx, start_node_id, input, &mut execution_record)
            .await;

        let duration = start_time.elapsed();
        execution_record.ended_at = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        );

        let final_status = match result {
            Ok(_) => {
                execution_record.status = WorkflowStatus::Completed;
                info!("Workflow {} completed in {:?}", graph.name, duration);
                "completed".to_string()
            }
            Err(ref e) => {
                execution_record.status = WorkflowStatus::Failed(e.clone());
                error!("Workflow {} failed: {}", graph.name, e);
                format!("failed: {}", e)
            }
        };

        // 发送完成事件
        self.emit_event(ExecutionEvent::WorkflowCompleted {
            workflow_id: graph.id.clone(),
            execution_id: ctx.execution_id.clone(),
            status: execution_record.status.clone(),
        })
        .await;

        // Emit debug telemetry: WorkflowEnd
        self.emit_debug(DebugEvent::WorkflowEnd {
            workflow_id: graph.id.clone(),
            execution_id: ctx.execution_id.clone(),
            timestamp_ms: DebugEvent::now_ms(),
            status: final_status,
        })
        .await;

        Ok(execution_record)
    }

    pub async fn resume_from_checkpoint(
        &self,
        graph: &WorkflowGraph,
        checkpoint: ExecutionCheckpoint,
    ) -> Result<ExecutionRecord, String> {
        let start_time = Instant::now();
        let mut ctx = WorkflowContext::new_with_id(&graph.id, checkpoint.execution_id.clone());
        
        self.emit_event(ExecutionEvent::WorkflowStarted {
            workflow_id: graph.id.clone(),
            execution_id: ctx.execution_id.clone(),
        })
        .await;

        info!(
            "Resuming workflow execution: {} ({} from checkpoint {})",
            graph.name, ctx.execution_id, checkpoint.execution_id
        );

        if let Err(errors) = graph.validate() {
            let error_msg = errors.join("; ");
            error!("Workflow validation failed: {}", error_msg);
            return Err(error_msg);
        }

        // Restore checkpoint data
        for (node_id, output) in checkpoint.node_outputs {
            ctx.set_node_output(&node_id, output).await;
        }
        for node_id in checkpoint.completed_nodes {
            ctx.set_node_status(&node_id, NodeStatus::Completed).await;
        }
        for (var_name, value) in checkpoint.variables {
            ctx.set_variable(&var_name, value).await;
        }

        let start_node_id = graph
            .start_node()
            .ok_or_else(|| "No start node".to_string())?;

        let mut execution_record = ExecutionRecord {
            execution_id: ctx.execution_id.clone(),
            workflow_id: graph.id.clone(),
            started_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            ended_at: None,
            status: WorkflowStatus::Running,
            node_records: Vec::new(),
            outputs: HashMap::new(),
        };

        let result = self
            .execute_from_node(graph, &ctx, start_node_id, WorkflowValue::Null, &mut execution_record)
            .await;

        let duration = start_time.elapsed();
        execution_record.ended_at = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        );

        match result {
            Ok(_) => {
                execution_record.status = WorkflowStatus::Completed;
                info!("Workflow {} resumed and completed in {:?}", graph.name, duration);
            }
            Err(ref e) => {
                execution_record.status = WorkflowStatus::Failed(e.clone());
                error!("Workflow {} resumed and failed: {}", graph.name, e);
            }
        }

        execution_record.outputs = ctx.get_all_outputs().await;

        self.emit_event(ExecutionEvent::WorkflowCompleted {
            workflow_id: graph.id.clone(),
            execution_id: ctx.execution_id.clone(),
            status: execution_record.status.clone(),
        })
        .await;

        Ok(execution_record)
    }

    /// 从指定节点开始执行（迭代版本，避免递归异步问题）
    async fn execute_from_node(
        &self,
        graph: &WorkflowGraph,
        ctx: &WorkflowContext,
        start_node_id: &str,
        initial_input: WorkflowValue,
        record: &mut ExecutionRecord,
    ) -> Result<WorkflowValue, String> {
        let mut current_node_id = start_node_id.to_string();
        let mut current_input = initial_input;

        loop {
            let node = graph
                .get_node(&current_node_id)
                .ok_or_else(|| format!("Node {} not found", current_node_id))?;

            let is_completed = ctx.get_node_status(&current_node_id).await == Some(NodeStatus::Completed);

            // Emit debug telemetry: NodeStart
            self.emit_debug(DebugEvent::NodeStart {
                node_id: current_node_id.clone(),
                timestamp_ms: DebugEvent::now_ms(),
                state_snapshot: serde_json::to_value(&current_input).unwrap_or_default(),
            })
            .await;

            let start_time = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;

            let result = if is_completed {
                info!("Skipping already completed node: {}", current_node_id);
                Ok(ctx.get_node_output(&current_node_id).await.unwrap_or(WorkflowValue::Null))
            } else {
                ctx.set_node_status(&current_node_id, NodeStatus::Running)
                    .await;
                self.emit_event(ExecutionEvent::NodeStarted {
                    node_id: current_node_id.clone(),
                })
                .await;

                match node.node_type() {
                    NodeType::Parallel => {
                        self.execute_parallel(graph, ctx, node, current_input.clone(), record)
                            .await
                    }
                    NodeType::Join => self.execute_join(graph, ctx, node, record).await,
                    NodeType::SubWorkflow => {
                        self.execute_sub_workflow(graph, ctx, node, current_input.clone(), record)
                            .await
                    }
                    NodeType::Wait => self.execute_wait(ctx, node, current_input.clone()).await,
                    _ => {
                        let result = node.execute(ctx, current_input.clone()).await;
                        ctx.set_node_output(&current_node_id, result.output.clone())
                            .await;
                        ctx.set_node_status(&current_node_id, result.status.clone())
                            .await;
                        self.emit_event(ExecutionEvent::NodeCompleted {
                            node_id: current_node_id.clone(),
                            result: result.clone(),
                        })
                        .await;
                        if result.status.is_success() {
                            Ok(result.output)
                        } else {
                            Err(result.error.unwrap_or_else(|| "Unknown error".to_string()))
                        }
                    }
                }
            };

            let end_time = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;

            // Emit debug telemetry: NodeEnd
            self.emit_debug(DebugEvent::NodeEnd {
                node_id: current_node_id.clone(),
                timestamp_ms: end_time,
                state_snapshot: match &result {
                    Ok(output) => serde_json::to_value(output).unwrap_or_default(),
                    Err(e) => serde_json::json!({"error": e}),
                },
                duration_ms: end_time.saturating_sub(start_time),
            })
            .await;

            // 记录节点执行
            if !is_completed {
                record.node_records.push(NodeExecutionRecord {
                    node_id: current_node_id.clone(),
                    started_at: start_time,
                    ended_at: end_time,
                    status: ctx
                        .get_node_status(&current_node_id)
                        .await
                        .unwrap_or(NodeStatus::Pending),
                    retry_count: 0,
                });
            }

            // 检查点
            if self.config.enable_checkpoints
                && self.config.checkpoint_interval > 0
                && !record.node_records.is_empty()
                && record.node_records.len().is_multiple_of(self.config.checkpoint_interval)
            {
                let label = format!("auto_checkpoint_{}", record.node_records.len());
                ctx.create_checkpoint(&label).await;
                self.emit_event(ExecutionEvent::CheckpointCreated { label })
                    .await;
            }

            // 处理结果
            match result {
                Ok(output) => {
                    // 确定下一个节点
                    let next = self.determine_next_node(graph, node, &output).await;

                    match next {
                        Some(next_node_id) => {
                            // 继续执行下一个节点
                            current_node_id = next_node_id;
                            current_input = output;
                            // 继续循环
                        }
                        None => {
                            // 没有下一个节点，返回当前输出
                            return Ok(output);
                        }
                    }
                }
                Err(e) => {
                    // 尝试错误处理
                    if let Some(error_handler) = graph.get_error_handler(&current_node_id) {
                        warn!(
                            "Node {} failed, executing error handler: {}",
                            current_node_id, error_handler
                        );
                        let error_input = WorkflowValue::Map({
                            let mut m = HashMap::new();
                            m.insert("error".to_string(), WorkflowValue::String(e.clone()));
                            m.insert(
                                "node_id".to_string(),
                                WorkflowValue::String(current_node_id.clone()),
                            );
                            m
                        });
                        current_node_id = error_handler.to_string();
                        current_input = error_input;
                        // 继续循环执行错误处理器
                    } else if self.config.stop_on_failure {
                        return Err(e);
                    } else {
                        warn!("Node {} failed but continuing: {}", current_node_id, e);
                        // 尝试继续执行下一个节点
                        if let Some(next_node_id) = graph.get_next_node(&current_node_id, None) {
                            current_node_id = next_node_id.to_string();
                            current_input = WorkflowValue::Null;
                            // 继续循环
                        } else {
                            return Err(e);
                        }
                    }
                }
            }
        }
    }

    /// 确定下一个节点
    async fn determine_next_node(
        &self,
        graph: &WorkflowGraph,
        node: &WorkflowNode,
        output: &WorkflowValue,
    ) -> Option<String> {
        let node_id = node.id();

        match node.node_type() {
            NodeType::Condition => {
                // 条件节点根据输出确定分支
                let condition = output.as_str().unwrap_or("false");
                graph
                    .get_next_node(node_id, Some(condition))
                    .map(|s| s.to_string())
            }
            NodeType::End => {
                // 结束节点没有后续
                None
            }
            _ => {
                // 其他节点获取默认下一个
                graph.get_next_node(node_id, None).map(|s| s.to_string())
            }
        }
    }

    /// 执行并行节点
    async fn execute_parallel(
        &self,
        graph: &WorkflowGraph,
        ctx: &WorkflowContext,
        node: &WorkflowNode,
        input: WorkflowValue,
        record: &mut ExecutionRecord,
    ) -> Result<WorkflowValue, String> {
        let branches = node.parallel_branches();

        if branches.is_empty() {
            // 如果没有指定分支，使用出边作为分支
            let edges = graph.get_outgoing_edges(node.id());
            let branch_ids: Vec<String> = edges.iter().map(|e| e.to.clone()).collect();

            if branch_ids.is_empty() {
                return Ok(input);
            }

            return self
                .execute_branches_parallel(graph, ctx, &branch_ids, input, record)
                .await;
        }

        self.execute_branches_parallel(graph, ctx, branches, input, record)
            .await
    }

    /// 并行执行多个分支
    /// 注意：由于节点包含闭包无法跨线程，这里使用顺序执行
    async fn execute_branches_parallel(
        &self,
        graph: &WorkflowGraph,
        ctx: &WorkflowContext,
        branches: &[String],
        input: WorkflowValue,
        _record: &mut ExecutionRecord,
    ) -> Result<WorkflowValue, String> {
        let mut results = HashMap::new();
        let mut errors = Vec::new();

        // 顺序执行各分支（节点包含闭包，无法跨线程共享）
        for branch_id in branches {
            if let Some(node) = graph.get_node(branch_id) {
                if ctx.get_node_status(branch_id).await == Some(NodeStatus::Completed) {
                    if let Some(output) = ctx.get_node_output(branch_id).await {
                        results.insert(branch_id.clone(), output);
                    }
                    continue;
                }

                let result = node.execute(ctx, input.clone()).await;
                ctx.set_node_output(branch_id, result.output.clone()).await;
                ctx.set_node_status(branch_id, result.status.clone()).await;

                if result.status.is_success() {
                    results.insert(branch_id.clone(), result.output);
                } else {
                    errors.push(format!(
                        "{}: {}",
                        branch_id,
                        result.error.unwrap_or_else(|| "Unknown error".to_string())
                    ));
                }
            } else {
                errors.push(format!("Node {} not found", branch_id));
            }
        }

        if !errors.is_empty() && self.config.stop_on_failure {
            return Err(errors.join("; "));
        }

        Ok(WorkflowValue::Map(results))
    }

    /// 执行聚合节点
    async fn execute_join(
        &self,
        _graph: &WorkflowGraph,
        ctx: &WorkflowContext,
        node: &WorkflowNode,
        _record: &mut ExecutionRecord,
    ) -> Result<WorkflowValue, String> {
        let wait_for = node.join_nodes();

        // 等待所有前置节点完成
        let mut all_completed = false;
        let mut attempts = 0;
        const MAX_ATTEMPTS: u32 = 1000;

        while !all_completed && attempts < MAX_ATTEMPTS {
            all_completed = true;
            for node_id in wait_for {
                match ctx.get_node_status(node_id).await {
                    Some(status) if status.is_terminal() => {}
                    _ => {
                        all_completed = false;
                        break;
                    }
                }
            }
            if !all_completed {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                attempts += 1;
            }
        }

        if !all_completed {
            return Err("Join timeout waiting for nodes".to_string());
        }

        // 收集所有前置节点的输出
        let outputs = ctx
            .get_node_outputs(&wait_for.iter().map(|s| s.as_str()).collect::<Vec<_>>())
            .await;

        // 执行节点（可能有转换函数）
        let result = node.execute(ctx, WorkflowValue::Map(outputs)).await;

        ctx.set_node_output(node.id(), result.output.clone()).await;
        ctx.set_node_status(node.id(), result.status.clone()).await;

        if result.status.is_success() {
            Ok(result.output)
        } else {
            Err(result.error.unwrap_or_else(|| "Join failed".to_string()))
        }
    }

    /// 执行子工作流
    /// 注意：子工作流执行使用独立的执行上下文
    async fn execute_sub_workflow(
        &self,
        _graph: &WorkflowGraph,
        ctx: &WorkflowContext,
        node: &WorkflowNode,
        input: WorkflowValue,
        _record: &mut ExecutionRecord,
    ) -> Result<WorkflowValue, String> {
        let sub_workflow_id = node
            .sub_workflow_id()
            .ok_or_else(|| "No sub-workflow specified".to_string())?;

        let workflows = self.sub_workflows.read().await;
        let sub_graph = workflows
            .get(sub_workflow_id)
            .ok_or_else(|| format!("Sub-workflow {} not found", sub_workflow_id))?
            .clone();
        drop(workflows);

        info!("Executing sub-workflow: {}", sub_workflow_id);

        // 使用 execute_parallel_workflow 而不是 execute 来避免递归
        // 这样可以避免无限递归的 Future 大小问题
        let sub_record = self.execute_parallel_workflow(&sub_graph, input).await?;

        // 获取子工作流的最终输出
        let output = if let Some(end_node) = sub_graph.end_nodes().first() {
            sub_record.outputs.get(end_node).cloned().unwrap_or(WorkflowValue::Null)
        } else {
            WorkflowValue::Null
        };

        Ok(output)
    }

    /// 执行等待节点
    async fn execute_wait(
        &self,
        ctx: &WorkflowContext,
        node: &WorkflowNode,
        _input: WorkflowValue,
    ) -> Result<WorkflowValue, String> {
        let event_type = node
            .wait_event_type()
            .ok_or_else(|| "No event type specified".to_string())?;

        info!("Waiting for event: {}", event_type);

        // 创建等待通道
        let (tx, rx) = oneshot::channel();

        {
            let mut waiters = self.event_waiters.write().await;
            waiters.entry(event_type.to_string()).or_default().push(tx);
        }

        // 等待事件或超时
        let timeout = node.config.timeout.execution_timeout_ms;
        let result = if timeout > 0 {
            tokio::time::timeout(std::time::Duration::from_millis(timeout), rx)
                .await
                .map_err(|_| "Wait timeout".to_string())?
                .map_err(|_| "Wait cancelled".to_string())?
        } else {
            rx.await.map_err(|_| "Wait cancelled".to_string())?
        };

        ctx.set_node_output(node.id(), result.clone()).await;
        ctx.set_node_status(node.id(), NodeStatus::Completed).await;

        Ok(result)
    }

    /// 基于拓扑层次执行工作流
    /// 注意：由于节点包含闭包无法跨线程，这里按层次顺序执行
    /// 同一层的节点理论上可以并行，但由于闭包限制，这里顺序执行
    pub async fn execute_parallel_workflow(
        &self,
        graph: &WorkflowGraph,
        input: WorkflowValue,
    ) -> Result<ExecutionRecord, String> {
        let ctx = WorkflowContext::new(&graph.id);
        ctx.set_input(input.clone()).await;

        let start_time = Instant::now();

        info!(
            "Starting layered workflow execution: {} ({})",
            graph.name, ctx.execution_id
        );

        // 获取并行组（按拓扑层次分组）
        let groups = graph.get_parallel_groups();

        let mut execution_record = ExecutionRecord {
            execution_id: ctx.execution_id.clone(),
            workflow_id: graph.id.clone(),
            started_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            ended_at: None,
            status: WorkflowStatus::Running,
            node_records: Vec::new(),
            outputs: HashMap::new(),
        };

        // 按层次执行（同层节点顺序执行，因为闭包无法跨线程共享）
        for group in groups {
            for node_id in group {
                let node_start_time = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;

                let result = if let Some(node) = graph.get_node(&node_id) {
                    if ctx.get_node_status(&node_id).await == Some(NodeStatus::Completed) {
                        info!("Skipping already completed node: {}", node_id);
                        NodeResult::success(
                            &node_id,
                            ctx.get_node_output(&node_id).await.unwrap_or(WorkflowValue::Null),
                            0,
                        )
                    } else {
                        let predecessors = graph.get_predecessors(&node_id);
                        let node_input = if predecessors.is_empty() {
                            ctx.get_input().await
                        } else if predecessors.len() == 1 {
                            ctx.get_node_output(predecessors[0])
                                .await
                                .unwrap_or(WorkflowValue::Null)
                        } else {
                            let outputs = ctx.get_node_outputs(&predecessors).await;
                            WorkflowValue::Map(outputs)
                        };
                        let result = node.execute(&ctx, node_input).await;
                        ctx.set_node_output(&node_id, result.output.clone()).await;
                        ctx.set_node_status(&node_id, result.status.clone()).await;
                        result
                    }
                } else {
                    NodeResult::failed(&node_id, "Node not found", 0)
                };

                let node_end_time = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;

                let record_entry = NodeExecutionRecord {
                    node_id: node_id.clone(),
                    started_at: node_start_time,
                    ended_at: node_end_time,
                    status: result.status.clone(),
                    retry_count: result.retry_count,
                };
                execution_record.node_records.push(record_entry);

                if !result.status.is_success() && self.config.stop_on_failure {
                    execution_record.status = WorkflowStatus::Failed(
                        result.error.unwrap_or_else(|| "Unknown error".to_string()),
                    );
                    execution_record.ended_at = Some(
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as u64,
                    );
                    execution_record.outputs = ctx.get_all_outputs().await;
                    return Ok(execution_record);
                }
            }
        }

        let duration = start_time.elapsed();
        execution_record.status = WorkflowStatus::Completed;
        execution_record.ended_at = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        );
        
        execution_record.outputs = ctx.get_all_outputs().await;

        info!(
            "Layered workflow {} completed in {:?}",
            graph.name, duration
        );

        Ok(execution_record)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_simple_workflow_execution() {
        let mut graph = WorkflowGraph::new("test", "Simple Workflow");

        graph.add_node(WorkflowNode::start("start"));
        graph.add_node(WorkflowNode::task(
            "double",
            "Double",
            |_ctx, input| async move {
                let value = input.as_i64().unwrap_or(0);
                Ok(WorkflowValue::Int(value * 2))
            },
        ));
        graph.add_node(WorkflowNode::task(
            "add_ten",
            "Add Ten",
            |_ctx, input| async move {
                let value = input.as_i64().unwrap_or(0);
                Ok(WorkflowValue::Int(value + 10))
            },
        ));
        graph.add_node(WorkflowNode::end("end"));

        graph.connect("start", "double");
        graph.connect("double", "add_ten");
        graph.connect("add_ten", "end");

        let executor = WorkflowExecutor::new(ExecutorConfig::default());
        let result = executor
            .execute(&graph, WorkflowValue::Int(5))
            .await
            .unwrap();

        assert!(matches!(result.status, WorkflowStatus::Completed));
    }

    #[tokio::test]
    async fn test_conditional_workflow() {
        let mut graph = WorkflowGraph::new("test", "Conditional Workflow");

        graph.add_node(WorkflowNode::start("start"));
        graph.add_node(WorkflowNode::condition(
            "check",
            "Check Value",
            |_ctx, input| async move { input.as_i64().unwrap_or(0) > 10 },
        ));
        graph.add_node(WorkflowNode::task(
            "high",
            "High Path",
            |_ctx, _input| async move { Ok(WorkflowValue::String("high".to_string())) },
        ));
        graph.add_node(WorkflowNode::task(
            "low",
            "Low Path",
            |_ctx, _input| async move { Ok(WorkflowValue::String("low".to_string())) },
        ));
        graph.add_node(WorkflowNode::end("end"));

        graph.connect("start", "check");
        graph.connect_conditional("check", "high", "true");
        graph.connect_conditional("check", "low", "false");
        graph.connect("high", "end");
        graph.connect("low", "end");

        let executor = WorkflowExecutor::new(ExecutorConfig::default());

        // 测试高路径
        let result = executor
            .execute(&graph, WorkflowValue::Int(20))
            .await
            .unwrap();
        assert!(matches!(result.status, WorkflowStatus::Completed));
    }

    #[tokio::test]
    async fn test_checkpoint_resume() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let mut graph = WorkflowGraph::new("test", "Checkpoint Workflow");

        let step1_count = Arc::new(AtomicUsize::new(0));
        let step2_count = Arc::new(AtomicUsize::new(0));

        let step1_count_clone = Arc::clone(&step1_count);
        let step2_count_clone = Arc::clone(&step2_count);

        graph.add_node(WorkflowNode::start("start"));
        graph.add_node(WorkflowNode::task("step1", "Step 1", move |_ctx, _input| {
            let count = Arc::clone(&step1_count_clone);
            async move {
                count.fetch_add(1, Ordering::SeqCst);
                Ok(WorkflowValue::String("step1_done".to_string()))
            }
        }));
        graph.add_node(WorkflowNode::task("step2", "Step 2", move |_ctx, _input| {
            let count = Arc::clone(&step2_count_clone);
            async move {
                count.fetch_add(1, Ordering::SeqCst);
                Ok(WorkflowValue::String("step2_done".to_string()))
            }
        }));
        graph.add_node(WorkflowNode::end("end"));

        graph.connect("start", "step1");
        graph.connect("step1", "step2");
        graph.connect("step2", "end");

        let executor = WorkflowExecutor::new(ExecutorConfig::default());

        //simulate crashing after step1
        let mut node_outputs = HashMap::new();
        node_outputs.insert("start".to_string(), WorkflowValue::Null);
        node_outputs.insert("step1".to_string(), WorkflowValue::String("step1_done".to_string()));

        let checkpoint = ExecutionCheckpoint {
            execution_id: "test-exec-id".to_string(),
            workflow_id: "test".to_string(),
            completed_nodes: vec!["start".to_string(), "step1".to_string()],
            node_outputs,
            variables: HashMap::new(),
            timestamp: 0,
        };

        let result2 = executor.resume_from_checkpoint(&graph, checkpoint).await.unwrap();
        assert!(matches!(result2.status, WorkflowStatus::Completed));

        assert_eq!(step1_count.load(Ordering::SeqCst), 0, "Step1 should be skipped");
        assert_eq!(step2_count.load(Ordering::SeqCst), 1, "Step2 should be executed");
    }
}
