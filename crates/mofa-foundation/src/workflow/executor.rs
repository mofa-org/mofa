//! 工作流执行器
//! Workflow Executor
//!
//! 负责工作流的执行调度
//! Responsible for workflow execution scheduling
//!
//! Supports optional telemetry emission for the time-travel debugger.
//! When a `TelemetryEmitter` is attached via `with_telemetry()`, the
//! executor emits `DebugEvent`s at key execution points.

use super::execution_event::ExecutionEvent;
use super::graph::WorkflowGraph;
use super::node::{NodeType, WorkflowNode};
use super::profiler::{ExecutionTimeline, ProfilerMode};
use super::state::{
    ExecutionCheckpoint, ExecutionRecord, NodeExecutionRecord, NodeResult, NodeStatus,
    WorkflowContext, WorkflowStatus, WorkflowValue,
};
use mofa_kernel::workflow::telemetry::{DebugEvent, TelemetryEmitter};
use std::collections::HashMap;
use std::sync::Arc;
use serde_json;
use std::time::Instant;
use tokio::sync::{RwLock, Semaphore, mpsc, oneshot};
use tracing::{error, info, warn};

/// 执行器配置
/// Executor Configuration
#[derive(Debug, Clone)]
pub struct ExecutorConfig {
    /// 最大并行度
    /// Maximum parallelism
    pub max_parallelism: usize,
    /// 是否在失败时停止
    /// Whether to stop on failure
    pub stop_on_failure: bool,
    /// 是否启用检查点
    /// Whether to enable checkpoints
    pub enable_checkpoints: bool,
    /// 检查点间隔（节点数）
    /// Checkpoint interval (number of nodes)
    pub checkpoint_interval: usize,
    /// 执行超时（毫秒）
    /// Execution timeout (milliseconds)
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


/// 工作流执行器
/// Workflow Executor
#[derive(Clone)]
pub struct WorkflowExecutor {
    /// 执行器配置
    /// Executor configuration
    config: ExecutorConfig,
    /// 事件发送器
    /// Event transmitter
    event_tx: Option<mpsc::Sender<ExecutionEvent>>,
    /// Telemetry emitter for the time-travel debugger (optional)
    telemetry: Option<Arc<dyn TelemetryEmitter>>,
    /// 子工作流注册表
    /// Sub-workflow registry
    sub_workflows: Arc<RwLock<HashMap<String, Arc<WorkflowGraph>>>>,
    /// 外部事件等待器
    /// External event waiters
    event_waiters: Arc<RwLock<HashMap<String, Vec<oneshot::Sender<WorkflowValue>>>>>,
    /// 并行执行信号量
    /// Parallel execution semaphore
    semaphore: Arc<Semaphore>,
    /// Profiler for execution timing (optional)
    profiler: ProfilerMode,
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
            profiler: ProfilerMode::Disabled,
        }
    }

    /// 设置事件发送器
    /// Set event transmitter
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

    /// Attach a profiler for execution timing capture.
    ///
    /// When set, the executor will record execution timing spans.
    pub fn with_profiler(mut self, mode: ProfilerMode) -> Self {
        self.profiler = mode;
        self
    }

    /// Get profiler timeline if profiling is enabled.
    pub fn profiler_timeline(&self) -> Option<&ExecutionTimeline> {
        match &self.profiler {
            ProfilerMode::Record(timeline) => Some(timeline.get_timeline()),
            ProfilerMode::Disabled => None,
        }
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
    /// Register sub-workflow
    pub async fn register_sub_workflow(&self, id: &str, graph: WorkflowGraph) {
        let mut workflows = self.sub_workflows.write().await;
        workflows.insert(id.to_string(), Arc::new(graph));
    }

    /// 发送执行事件
    /// Emit execution event
    async fn emit_event(&self, event: ExecutionEvent) {
        if let Some(ref tx) = self.event_tx {
            let _ = tx.send(event).await;
        }
    }

    /// 发送外部事件
    /// Send external event
    pub async fn send_external_event(&self, event_type: &str, data: WorkflowValue) {
        let mut waiters = self.event_waiters.write().await;
        if let Some(senders) = waiters.remove(event_type) {
            for sender in senders {
                let _ = sender.send(data.clone());
            }
        }
    }

    /// 执行工作流
    /// Execute workflow
    pub async fn execute(
        &self,
        graph: &WorkflowGraph,
        input: WorkflowValue,
    ) -> Result<ExecutionRecord, String> {
        let start_time = Instant::now();
        let ctx = WorkflowContext::new(&graph.id);
        ctx.set_input(input.clone()).await;

        // 发送开始事件
        // Emit start event
        self.emit_event(ExecutionEvent::WorkflowStarted {
            workflow_id: graph.id.clone(),
            workflow_name: graph.name.clone(),
            started_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
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
        // Validate graph
        if let Err(errors) = graph.validate() {
            let error_msg = errors.join("; ");
            error!("Workflow validation failed: {}", error_msg);
            return Err(error_msg);
        }

        // 获取开始节点
        // Get start node
        let start_node_id = graph
            .start_node()
            .ok_or_else(|| "No start node".to_string())?;

        // 执行工作流
        // Execute workflow
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
            total_wait_time_ms: 0,
            context: None,
        };

        // 使用基于依赖的执行
        // Use dependency-based execution
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
                if execution_record.status != WorkflowStatus::Paused {
                    execution_record.status = WorkflowStatus::Completed;
                    info!("Workflow {} completed in {:?}", graph.name, duration);
                    "completed".to_string()
                } else {
                    info!("Workflow {} paused after {:?}", graph.name, duration);
                    execution_record.context = Some(ctx.clone());
                    "paused".to_string()
                }
            }
            Err(ref e) => {
                execution_record.status = WorkflowStatus::Failed(e.clone());
                error!("Workflow {} failed: {}", graph.name, e);
                format!("failed: {}", e)
            }
        };
        execution_record.outputs = ctx.get_all_outputs().await;

        // 发送完成事件
        // Emit completion event
        match &execution_record.status {
            WorkflowStatus::Failed(e) => {
                self.emit_event(ExecutionEvent::WorkflowFailed {
                    workflow_id: graph.id.clone(),
                    error: e.clone(),
                    total_duration_ms: duration.as_millis() as u64,
                })
                .await;
            }
            _ => {
                self.emit_event(ExecutionEvent::WorkflowCompleted {
                    workflow_id: graph.id.clone(),
                    final_output: None,
                    total_duration_ms: duration.as_millis() as u64,
                })
                .await;
            }
        }

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

    pub async fn resume_with_human_input(
        &self,
        graph: &WorkflowGraph,
        ctx: WorkflowContext,
        waiting_node_id: &str,
        human_input: WorkflowValue,
    ) -> Result<ExecutionRecord, String> {
        info!(
            "Resuming workflow {} from node {} with human input",
            graph.id, waiting_node_id
        );

        // Calculate wait time
        if let Some(paused_at) = *ctx.paused_at.read().await {
            let duration = chrono::Utc::now().signed_duration_since(paused_at);
            let wait_duration_ms = duration.num_milliseconds().max(0) as u64;
            *ctx.total_wait_time_ms.write().await += wait_duration_ms;  // ← accumulate
        }

        ctx.set_node_output(waiting_node_id, human_input).await;
        ctx.set_node_status(waiting_node_id, NodeStatus::Completed)
            .await;
        *ctx.paused_at.write().await = None;
        *ctx.last_waiting_node.write().await = None;

        let start_time = Instant::now();

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
            // total_wait_time_ms: wait_duration_ms,
            total_wait_time_ms: *ctx.total_wait_time_ms.read().await,
            context: None,
        };

        let start_node_id = graph
            .start_node()
            .ok_or_else(|| "No start node".to_string())?;

        let result = self
            .execute_from_node(
                graph,
                &ctx,
                start_node_id,
                WorkflowValue::Null,
                &mut execution_record,
            )
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
                if execution_record.status != WorkflowStatus::Paused {
                    execution_record.status = WorkflowStatus::Completed;
                    info!("Workflow {} completed in {:?}", graph.name, duration);
                    "completed".to_string()
                } else {
                    "paused".to_string()
                }
            }
            Err(ref e) => {
                execution_record.status = WorkflowStatus::Failed(e.clone());
                error!("Workflow {} failed: {}", graph.name, e);
                format!("failed: {}", e)
            }
        };
        execution_record.outputs = ctx.get_all_outputs().await;

        match &execution_record.status {
            WorkflowStatus::Failed(e) => {
                self.emit_event(ExecutionEvent::WorkflowFailed {
                    workflow_id: graph.id.clone(),
                    error: e.clone(),
                    total_duration_ms: duration.as_millis() as u64,
                })
                .await;
            }
            _ => {
                self.emit_event(ExecutionEvent::WorkflowCompleted {
                    workflow_id: graph.id.clone(),
                    final_output: None,
                    total_duration_ms: duration.as_millis() as u64,
                })
                .await;
            }
        }

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
            workflow_name: graph.name.clone(),
            started_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
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
            total_wait_time_ms: 0,
            context: None,
        };

        let result = self
            .execute_from_node(
                graph,
                &ctx,
                start_node_id,
                WorkflowValue::Null,
                &mut execution_record,
            )
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
                info!(
                    "Workflow {} resumed and completed in {:?}",
                    graph.name, duration
                );
            }
            Err(ref e) => {
                execution_record.status = WorkflowStatus::Failed(e.clone());
                error!("Workflow {} resumed and failed: {}", graph.name, e);
            }
        }

        execution_record.outputs = ctx.get_all_outputs().await;

        match &execution_record.status {
            WorkflowStatus::Failed(e) => {
                self.emit_event(ExecutionEvent::WorkflowFailed {
                    workflow_id: graph.id.clone(),
                    error: e.clone(),
                    total_duration_ms: duration.as_millis() as u64,
                })
                .await;
            }
            _ => {
                self.emit_event(ExecutionEvent::WorkflowCompleted {
                    workflow_id: graph.id.clone(),
                    final_output: None,
                    total_duration_ms: duration.as_millis() as u64,
                })
                .await;
            }
        }

        Ok(execution_record)
    }

    /// Check if a node is ready to be scheduled (all dependencies met)
    /// Particularly useful for Join nodes which must wait for all join_nodes
    async fn can_schedule_node(
        &self,
        _graph: &WorkflowGraph,
        ctx: &WorkflowContext,
        node_id: &str,
    ) -> bool {
        // If it's already running, we shouldn't schedule it again
        let status = ctx.get_node_status(node_id).await;
        if status == Some(NodeStatus::Running) {
            return false;
        }

        // We check if it has join dependencies. If so, they must all be Completed.
        let node = match _graph.get_node(node_id) {
            Some(n) => n,
            None => return false,
        };

        if node.node_type() == &NodeType::Join {
            for pred_id in node.join_nodes() {
                match ctx.get_node_status(pred_id).await {
                    Some(s) if s.is_terminal() => {}
                    _ => return false, // Not ready
                }
            }
        }
        
        true
    }

    /// 尝试跳过已完成节点
    /// Try to skip completed node
    async fn try_skip_completed_node(
        &self,
        graph: &WorkflowGraph,
        ctx: &WorkflowContext,
        node_id: &str,
    ) -> Option<(Vec<String>, WorkflowValue)> {
        if ctx.get_node_status(node_id).await != Some(NodeStatus::Completed) {
            return None;
        }

        info!("Node {} already completed, skipping...", node_id);
        let output = ctx
            .get_node_output(node_id)
            .await
            .unwrap_or(WorkflowValue::Null);

        let node = graph.get_node(node_id)?;
        let next_nodes = self.determine_next_nodes(graph, node, &output).await;

        Some((next_nodes, output))
    }

    /// 从指定节点开始执行（异步事件循环版本）
    /// Execute from specified node (async event loop to support native DAG parallel execution)
    async fn execute_from_node(
        &self,
        graph: &WorkflowGraph,
        ctx: &WorkflowContext,
        start_node_id: &str,
        initial_input: WorkflowValue,
        record: &mut ExecutionRecord,
    ) -> Result<WorkflowValue, String> {
        use std::collections::VecDeque;

        let mut ready_queue = VecDeque::new();
        ready_queue.push_back((start_node_id.to_string(), initial_input));
        
        // This holds spawned node tasks yielding (node_id, node, result, start, end)
        let mut running_tasks = tokio::task::JoinSet::new();

        let mut final_output: Option<WorkflowValue> = None;

        loop {
            // 1. Drain ready_queue into running_tasks up to parallelism limits
            while !ready_queue.is_empty() && running_tasks.len() < self.config.max_parallelism {
                let (current_node_id, current_input) = ready_queue.pop_front().unwrap();

                let node = match graph.get_node(&current_node_id) {
                    Some(n) => n,
                    None => {
                        error!("Node {} not found in graph", current_node_id);
                        if self.config.stop_on_failure {
                            return Err(format!("Node {} not found", current_node_id));
                        }
                        continue;
                    }
                };

                // Try to skip if completed
                if let Some((next_nodes, output)) = self.try_skip_completed_node(graph, ctx, &current_node_id).await {
                    for next_id in next_nodes {
                        if self.can_schedule_node(graph, ctx, &next_id).await {
                            ready_queue.push_back((next_id, output.clone()));
                        }
                    }
                    if graph.end_nodes().contains(&current_node_id) {
                        final_output = Some(output);
                    }
                    continue;
                }

                // Check for HITL "wait_for_human" node PAUSE
                if node.config.node_type == NodeType::Wait {
                    info!("Pausing workflow at node: {}", current_node_id);
                    *ctx.paused_at.write().await = Some(chrono::Utc::now());
                    *ctx.last_waiting_node.write().await = Some(current_node_id.clone());

                    ctx.set_node_status(&current_node_id, NodeStatus::Waiting).await;
                    record.status = WorkflowStatus::Paused;
                    
                    running_tasks.abort_all();
                    return Ok(WorkflowValue::Null);
                }

                // Prepare execution
                ctx.set_node_status(&current_node_id, NodeStatus::Running).await;

                // We emit NodeStart synchronously here so it matches chronological order of spawning
                self.emit_debug(DebugEvent::NodeStart {
                    node_id: current_node_id.clone(),
                    timestamp_ms: DebugEvent::now_ms(),
                    state_snapshot: serde_json::to_value(&current_input).unwrap_or_default(),
                }).await;

                self.emit_event(ExecutionEvent::NodeStarted {
                    node_id: current_node_id.clone(),
                    node_name: node.config.name.clone(),
                    parent_span_id: None,
                }).await;

                let start_time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64;

                // Spawn task
                let executor_clone = self.clone();
                let ctx_clone = ctx.clone();
                let node_clone = node.clone();
                let semaphore = Arc::clone(&self.semaphore);

                running_tasks.spawn(async move {
                    let _permit = semaphore
                        .acquire_owned()
                        .await
                        .map_err(|_| "Execution semaphore closed".to_string());
                    
                    let execution_result = match node_clone.node_type() {
                        NodeType::SubWorkflow => {
                            let res = executor_clone.execute_sub_workflow(&ctx_clone, &node_clone, current_input.clone()).await;
                            match res {
                                Ok(out) => NodeResult::success(&current_node_id, out, 0),
                                Err(e) => NodeResult::failed(&current_node_id, &e, 0)
                            }
                        }
                        NodeType::Wait => {
                            let res = executor_clone.execute_wait(&ctx_clone, &node_clone, current_input.clone()).await;
                            match res {
                                Ok(out) => NodeResult::success(&current_node_id, out, 0),
                                Err(e) => NodeResult::failed(&current_node_id, &e, 0)
                            }
                        }
                        _ => {
                            // Automatically handles Parallel, Join, Task, Condition, Transform, etc.
                            node_clone.execute(&ctx_clone, current_input.clone()).await
                        }
                    };
                    
                    let end_time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64;

                    (current_node_id, node_clone, execution_result, start_time, end_time, current_input)
                });
            }

            // 2. Await next completion if there are any running tasks
            if running_tasks.is_empty() {
                break;
            }

            match running_tasks.join_next().await {
                Some(Ok((node_id, node, mut result, start_time, end_time, input_used))) => {
                    ctx.set_node_output(&node_id, result.output.clone()).await;
                    ctx.set_node_status(&node_id, result.status.clone()).await;

                    self.emit_event(ExecutionEvent::NodeCompleted {
                        node_id: node_id.clone(),
                        output: serde_json::to_value(&result.output).ok(),
                        duration_ms: end_time.saturating_sub(start_time),
                    }).await;

                    self.emit_debug(DebugEvent::NodeEnd {
                        node_id: node_id.clone(),
                        timestamp_ms: end_time,
                        state_snapshot: if result.status.is_success() {
                            serde_json::to_value(&result.output).unwrap_or_default()
                        } else {
                            serde_json::json!({"error": result.error.clone().unwrap_or_default()})
                        },
                        duration_ms: end_time.saturating_sub(start_time),
                    }).await;

                    record.node_records.push(NodeExecutionRecord {
                        node_id: node_id.clone(),
                        started_at: start_time,
                        ended_at: end_time,
                        status: result.status.clone(),
                        retry_count: result.retry_count,
                    });

                    // Checkpoints
                    if self.config.enable_checkpoints && self.config.checkpoint_interval > 0 && record.node_records.len().is_multiple_of(self.config.checkpoint_interval) {
                        let label = format!("auto_checkpoint_{}", record.node_records.len());
                        ctx.create_checkpoint(&label).await;
                        self.emit_event(ExecutionEvent::CheckpointCreated { label }).await;
                    }

                    if result.status.is_success() {
                        let next_nodes = self.determine_next_nodes(graph, &node, &result.output).await;
                        for next_id in next_nodes {
                            if self.can_schedule_node(graph, ctx, &next_id).await {
                                let fwd_input = if node.node_type() == &NodeType::Parallel {
                                    input_used.clone()
                                } else {
                                    result.output.clone()
                                };
                                ready_queue.push_back((next_id, fwd_input));
                            }
                        }
                        if graph.end_nodes().contains(&node_id) {
                            final_output = Some(result.output);
                        }
                    } else {
                        // Error handling
                        let error_msg = result.error.unwrap_or_else(|| "Unknown error".to_string());
                        if let Some(error_handler) = graph.get_error_handler(&node_id) {
                            warn!("Node {} failed, executing error handler: {}", node_id, error_handler);
                            let mut m = HashMap::new();
                            m.insert("error".to_string(), WorkflowValue::String(error_msg));
                            m.insert("node_id".to_string(), WorkflowValue::String(node_id));
                            if self.can_schedule_node(graph, ctx, error_handler).await {
                                ready_queue.push_back((error_handler.to_string(), WorkflowValue::Map(m)));
                            }
                        } else if self.config.stop_on_failure {
                            running_tasks.abort_all();
                            return Err(error_msg);
                        } else {
                            warn!("Node {} failed but continuing: {}", node_id, error_msg);
                            if let Some(next_node_id) = graph.get_next_node(&node_id, None) {
                                if self.can_schedule_node(graph, ctx, next_node_id).await {
                                    ready_queue.push_back((next_node_id.to_string(), WorkflowValue::Null));
                                }
                            }
                        }
                    }
                }
                Some(Err(join_err)) => {
                    error!("Task join error: {}", join_err);
                    if self.config.stop_on_failure {
                        running_tasks.abort_all();
                        return Err(format!("Task panicked or cancelled: {}", join_err));
                    }
                }
                None => break,
            }
        }

        Ok(final_output.unwrap_or(WorkflowValue::Null))
    }

    /// 确定下一个节点 (支持多分支)
    /// Determine the next nodes (supports multiple branches for Parallel nodes)
    async fn determine_next_nodes(
        &self,
        graph: &WorkflowGraph,
        node: &WorkflowNode,
        output: &WorkflowValue,
    ) -> Vec<String> {
        let node_id = node.id();

        match node.node_type() {
            NodeType::Condition => {
                let condition = output.as_str().unwrap_or("false");
                graph
                    .get_next_node(node_id, Some(condition))
                    .map(|s| vec![s.to_string()])
                    .unwrap_or_default()
            }
            NodeType::Parallel => {
                let branches = node.parallel_branches();
                if branches.is_empty() {
                    let edges = graph.get_outgoing_edges(node_id);
                    edges.iter().map(|e| e.to.clone()).collect()
                } else {
                    branches.to_vec()
                }
            }
            NodeType::End => {
                vec![]
            }
            _ => {
                graph.get_next_node(node_id, None).map(|s| vec![s.to_string()]).unwrap_or_default()
            }
        }
    }

    /// 执行子工作流
    /// Execute sub-workflow
    /// 注意：子工作流执行使用独立的执行上下文
    /// Note: Sub-workflow execution uses an independent execution context
    async fn execute_sub_workflow(
        &self,
        ctx: &WorkflowContext,
        node: &WorkflowNode,
        input: WorkflowValue,
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
        // Use execute_parallel_workflow instead of execute to avoid recursion
        // 这样可以避免无限递归的 Future 大小问题
        // This avoids Future size issues caused by infinite recursion
        let sub_record = self.execute_parallel_workflow(&sub_graph, input).await?;

        // 获取子工作流的最终输出
        // Get the final output of the sub-workflow
        let output = if let Some(end_node) = sub_graph.end_nodes().first() {
            sub_record
                .outputs
                .get(end_node)
                .cloned()
                .unwrap_or(WorkflowValue::Null)
        } else {
            WorkflowValue::Null
        };
        ctx.set_node_output(node.id(), output.clone()).await;
        ctx.set_node_status(node.id(), NodeStatus::Completed).await;

        Ok(output)
    }

    /// 执行等待节点
    /// Execute wait node
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
        // Create waiting channel
        let (tx, rx) = oneshot::channel();

        {
            let mut waiters = self.event_waiters.write().await;
            waiters.entry(event_type.to_string()).or_default().push(tx);
        }

        // 等待事件或超时
        // Wait for event or timeout
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
    /// Execute workflow based on topological layers
    /// 同一层的节点并行执行
    /// Nodes in the same layer are executed in parallel
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
        // Get parallel groups (grouped by topological layers)
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
            total_wait_time_ms: 0,
            context: None,
        };

        let ctx_ref = &ctx;
        let semaphore = Arc::clone(&self.semaphore);

        // 按层次执行（同一层次的节点可以并发执行）
        // Execute by layer (nodes in same layer execute concurrently)
        for group in groups {
            tracing::debug!("Spawning {} parallel branches in layer", group.len());
            let mut join_set: tokio::task::JoinSet<(NodeResult, NodeExecutionRecord)> =
                tokio::task::JoinSet::new();
            for node_id in &group {
                let n_id = node_id.clone();
                let ctx_clone = ctx_ref.clone();
                let node_clone = graph.get_node(&n_id).cloned();
                let semaphore = Arc::clone(&semaphore);
                let predecessors: Vec<String> = graph
                    .get_predecessors(&n_id)
                    .into_iter()
                    .map(String::from)
                    .collect();

                join_set.spawn(async move {
                    let node_start_time = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64;
                    let _permit = match semaphore.acquire_owned().await {
                        Ok(permit) => permit,
                        Err(e) => {
                            let result = NodeResult::failed(
                                &n_id,
                                &format!("Parallel semaphore closed: {}", e),
                                0,
                            );
                            let record_entry = NodeExecutionRecord {
                                node_id: n_id,
                                started_at: node_start_time,
                                ended_at: node_start_time,
                                status: result.status.clone(),
                                retry_count: result.retry_count,
                            };
                            return (result, record_entry);
                        }
                    };

                    let result = if let Some(node) = node_clone {
                        if ctx_clone.get_node_status(&n_id).await == Some(NodeStatus::Completed) {
                            info!("Skipping already completed node: {}", n_id);
                            NodeResult::success(
                                &n_id,
                                ctx_clone
                                    .get_node_output(&n_id)
                                    .await
                                    .unwrap_or(WorkflowValue::Null),
                                0,
                            )
                        } else {
                            let node_input = if predecessors.is_empty() {
                                ctx_clone.get_input().await
                            } else if predecessors.len() == 1 {
                                ctx_clone
                                    .get_node_output(&predecessors[0])
                                    .await
                                    .unwrap_or(WorkflowValue::Null)
                            } else {
                                let pred_refs: Vec<&str> =
                                    predecessors.iter().map(|s| s.as_str()).collect();
                                let outputs = ctx_clone.get_node_outputs(&pred_refs).await;
                                WorkflowValue::Map(outputs)
                            };
                            let result = node.execute(&ctx_clone, node_input).await;
                            ctx_clone
                                .set_node_output(&n_id, result.output.clone())
                                .await;
                            ctx_clone
                                .set_node_status(&n_id, result.status.clone())
                                .await;
                            result
                        }
                    } else {
                        NodeResult::failed(&n_id, "Node not found", 0)
                    };

                    let node_end_time = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64;

                    tracing::debug!(
                        "Branch {} completed in {}ms",
                        n_id,
                        node_end_time.saturating_sub(node_start_time)
                    );

                    let record_entry = NodeExecutionRecord {
                        node_id: n_id,
                        started_at: node_start_time,
                        ended_at: node_end_time,
                        status: result.status.clone(),
                        retry_count: result.retry_count,
                    };

                    (result, record_entry)
                });
            }

            // Wait for all nodes in this layer to finish.
            // Node updates are written synchronously to the WorkflowContext as each task finishes.
            // If `stop_on_failure` is enabled, any failure will abort remaining tasks in the layer.
            while let Some(res_join) = join_set.join_next().await {
                let (result, record_entry) = res_join.unwrap_or_else(|e| {
                    (
                        NodeResult::failed("unknown", &format!("Join error or panic: {}", e), 0),
                        NodeExecutionRecord {
                            node_id: "unknown".to_string(),
                            started_at: 0,
                            ended_at: 0,
                            status: NodeStatus::Failed(format!("Join panic: {}", e)),
                            retry_count: 0,
                        },
                    )
                });
                execution_record.node_records.push(record_entry);

                if !result.status.is_success() && self.config.stop_on_failure {
                    join_set.abort_all();
                    execution_record.status = WorkflowStatus::Failed(
                        result.error.unwrap_or_else(|| "Unknown error".to_string()),
                    );
                    execution_record.ended_at = Some(
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as u64,
                    );
                    execution_record.outputs = ctx_ref.get_all_outputs().await;
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
    use tokio::time::{Duration, Instant, sleep};

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
        // Test high path
        let result = executor
            .execute(&graph, WorkflowValue::Int(20))
            .await
            .unwrap();
        assert!(matches!(result.status, WorkflowStatus::Completed));
    }

    #[tokio::test]
    async fn test_checkpoint_resume() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};

        let mut graph = WorkflowGraph::new("test", "Checkpoint Workflow");

        let step1_count = Arc::new(AtomicUsize::new(0));
        let step2_count = Arc::new(AtomicUsize::new(0));

        let step1_count_clone = Arc::clone(&step1_count);
        let step2_count_clone = Arc::clone(&step2_count);

        graph.add_node(WorkflowNode::start("start"));
        graph.add_node(WorkflowNode::task(
            "step1",
            "Step 1",
            move |_ctx, _input| {
                let count = Arc::clone(&step1_count_clone);
                async move {
                    count.fetch_add(1, Ordering::SeqCst);
                    Ok(WorkflowValue::String("step1_done".to_string()))
                }
            },
        ));
        graph.add_node(WorkflowNode::task(
            "step2",
            "Step 2",
            move |_ctx, _input| {
                let count = Arc::clone(&step2_count_clone);
                async move {
                    count.fetch_add(1, Ordering::SeqCst);
                    Ok(WorkflowValue::String("step2_done".to_string()))
                }
            },
        ));
        graph.add_node(WorkflowNode::end("end"));

        graph.connect("start", "step1");
        graph.connect("step1", "step2");
        graph.connect("step2", "end");

        let executor = WorkflowExecutor::new(ExecutorConfig::default());

        //simulate crashing after step1
        let mut node_outputs = HashMap::new();
        node_outputs.insert("start".to_string(), WorkflowValue::Null);
        node_outputs.insert(
            "step1".to_string(),
            WorkflowValue::String("step1_done".to_string()),
        );

        let checkpoint = ExecutionCheckpoint {
            execution_id: "test-exec-id".to_string(),
            workflow_id: "test".to_string(),
            completed_nodes: vec!["start".to_string(), "step1".to_string()],
            node_outputs,
            variables: HashMap::new(),
            timestamp: 0,
        };

        let result2 = executor
            .resume_from_checkpoint(&graph, checkpoint)
            .await
            .unwrap();
        assert!(matches!(result2.status, WorkflowStatus::Completed));

        assert_eq!(
            step1_count.load(Ordering::SeqCst),
            0,
            "Step1 should be skipped"
        );
        assert_eq!(
            step2_count.load(Ordering::SeqCst),
            1,
            "Step2 should be executed"
        );
    }

    #[tokio::test]
    async fn test_sub_workflow_output() {
        let executor = WorkflowExecutor::new(ExecutorConfig::default());

        let mut sub_graph = WorkflowGraph::new("sub_wf", "Sub Workflow");
        sub_graph.add_node(WorkflowNode::start("sub_start"));
        sub_graph.add_node(WorkflowNode::task(
            "sub_task",
            "Sub Task",
            |_ctx, _input| async move { Ok(WorkflowValue::String("hello from sub".to_string())) },
        ));
        sub_graph.add_node(WorkflowNode::end("sub_end"));
        sub_graph.connect("sub_start", "sub_task");
        sub_graph.connect("sub_task", "sub_end");

        executor.register_sub_workflow("sub_wf", sub_graph).await;

        let mut parent_graph = WorkflowGraph::new("parent_wf", "Parent Workflow");
        parent_graph.add_node(WorkflowNode::start("parent_start"));
        parent_graph.add_node(WorkflowNode::sub_workflow(
            "call_sub",
            "Call Sub Workflow",
            "sub_wf",
        ));
        parent_graph.add_node(WorkflowNode::end("parent_end"));
        parent_graph.connect("parent_start", "call_sub");
        parent_graph.connect("call_sub", "parent_end");

        let result = executor
            .execute(&parent_graph, WorkflowValue::Null)
            .await
            .expect("Workflow execution failed");

        assert!(matches!(result.status, WorkflowStatus::Completed));

        let sub_output = result
            .outputs
            .get("call_sub")
            .cloned()
            .unwrap_or(WorkflowValue::Null);

        assert_eq!(
            sub_output.as_str().unwrap_or("Null"),
            "hello from sub",
            "Sub-workflow output was discarded!"
        );
    }

    #[tokio::test]
    async fn test_parallel_output() {
        let executor = WorkflowExecutor::new(ExecutorConfig::default());
        let mut graph = WorkflowGraph::new("parallel_wf", "Parallel Output Workflow");

        graph.add_node(WorkflowNode::start("start"));

        // Add parallel node
        graph.add_node(WorkflowNode::parallel(
            "parallel_split",
            "Split execution",
            vec!["branch_a", "branch_b"],
        ));

        // Add branches
        graph.add_node(WorkflowNode::task(
            "branch_a",
            "Branch A",
            |_ctx, _input| async move { Ok(WorkflowValue::String("result_from_a".to_string())) },
        ));
        graph.add_node(WorkflowNode::task(
            "branch_b",
            "Branch B",
            |_ctx, _input| async move { Ok(WorkflowValue::String("result_from_b".to_string())) },
        ));

        graph.add_node(WorkflowNode::end("end"));

        graph.connect("start", "parallel_split");
        graph.connect("parallel_split", "branch_a");
        graph.connect("parallel_split", "branch_b");
        graph.connect("branch_a", "end");
        graph.connect("branch_b", "end");

        let result = executor
            .execute(&graph, WorkflowValue::Null)
            .await
            .expect("Workflow execution failed");

        assert!(matches!(result.status, WorkflowStatus::Completed));

        let branch_a_output = result
            .outputs
            .get("branch_a")
            .cloned()
            .unwrap_or(WorkflowValue::Null);
            
        let branch_b_output = result
            .outputs
            .get("branch_b")
            .cloned()
            .unwrap_or(WorkflowValue::Null);

        assert_eq!(
            branch_a_output.as_str(),
            Some("result_from_a"),
            "Parallel node missing branch A output"
        );
        assert_eq!(
            branch_b_output.as_str(),
            Some("result_from_b"),
            "Parallel node missing branch B output"
        );
    }

    #[tokio::test]
    async fn test_parallel_branches_execute_concurrently() {
        let executor = WorkflowExecutor::new(ExecutorConfig::default());
        let mut graph = WorkflowGraph::new("parallel_timing_wf", "Parallel Timing Workflow");

        graph.add_node(WorkflowNode::start("start"));
        graph.add_node(WorkflowNode::parallel(
            "parallel_split",
            "Split execution",
            vec!["branch_a", "branch_b"],
        ));
        graph.add_node(WorkflowNode::task(
            "branch_a",
            "Branch A",
            |_ctx, _input| async move {
                sleep(Duration::from_millis(300)).await;
                Ok(WorkflowValue::String("a_done".to_string()))
            },
        ));
        graph.add_node(WorkflowNode::task(
            "branch_b",
            "Branch B",
            |_ctx, _input| async move {
                sleep(Duration::from_millis(300)).await;
                Ok(WorkflowValue::String("b_done".to_string()))
            },
        ));
        graph.add_node(WorkflowNode::end("end"));

        graph.connect("start", "parallel_split");
        graph.connect("parallel_split", "branch_a");
        graph.connect("parallel_split", "branch_b");
        graph.connect("branch_a", "end");
        graph.connect("branch_b", "end");

        let started = Instant::now();
        let result = executor
            .execute(&graph, WorkflowValue::Null)
            .await
            .expect("Workflow execution failed");
        let elapsed = started.elapsed();

        assert!(matches!(result.status, WorkflowStatus::Completed));
        assert!(
            elapsed < Duration::from_millis(500),
            "Expected parallel execution under 500ms, got {:?}",
            elapsed
        );
    }

    #[tokio::test]
    async fn test_parallel_branch_input_isolation() {
        let executor = WorkflowExecutor::new(ExecutorConfig::default());
        let mut graph =
            WorkflowGraph::new("parallel_input_wf", "Parallel Input Isolation Workflow");

        graph.add_node(WorkflowNode::start("start"));
        graph.add_node(WorkflowNode::parallel(
            "parallel_split",
            "Split execution",
            vec!["branch_a", "branch_b"],
        ));

        graph.add_node(WorkflowNode::task(
            "branch_a",
            "Branch A",
            |_ctx, input| async move {
                let mut map = input.as_map().cloned().unwrap_or_default();
                map.insert("branch".to_string(), WorkflowValue::String("a".to_string()));
                Ok(WorkflowValue::Map(map))
            },
        ));
        graph.add_node(WorkflowNode::task(
            "branch_b",
            "Branch B",
            |_ctx, input| async move { Ok(input) },
        ));
        graph.add_node(WorkflowNode::end("end"));

        graph.connect("start", "parallel_split");
        graph.connect("parallel_split", "branch_a");
        graph.connect("parallel_split", "branch_b");
        graph.connect("branch_a", "end");
        graph.connect("branch_b", "end");

        let mut input = HashMap::new();
        input.insert("seed".to_string(), WorkflowValue::Int(7));

        let result = executor
            .execute(&graph, WorkflowValue::Map(input))
            .await
            .expect("Workflow execution failed");

        let branch_a_output = result
            .outputs
            .get("branch_a")
            .cloned()
            .unwrap_or(WorkflowValue::Null);

        let branch_b_output = result
            .outputs
            .get("branch_b")
            .cloned()
            .unwrap_or(WorkflowValue::Null);

        let branch_a = branch_a_output.as_map().expect("branch_a output must be map");
        let branch_b = branch_b_output.as_map().expect("branch_b output must be map");

        assert_eq!(branch_a.get("branch").and_then(|v| v.as_str()), Some("a"));
        assert!(
            !branch_b.contains_key("branch"),
            "branch_b should not observe branch_a input mutation"
        );
        assert_eq!(branch_b.get("seed").and_then(|v| v.as_i64()), Some(7));
    }
}
