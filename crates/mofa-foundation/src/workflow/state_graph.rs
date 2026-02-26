//! StateGraph Implementation
//!
//! This module provides a LangGraph-inspired StateGraph implementation
//! for building and executing stateful workflow graphs.

use async_trait::async_trait;
use futures::Stream;
use mofa_kernel::agent::error::{AgentError, AgentResult};
use mofa_kernel::workflow::{
    Command, CompiledGraph, ControlFlow, END, EdgeTarget, GraphConfig, GraphState, NodeFunc,
    Reducer, RuntimeContext, START, StateUpdate, StepResult, StreamEvent,
};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::{RwLock, Semaphore};
use tokio::task::JoinSet;
use tracing::{debug, info, warn};

use super::fault_tolerance::{
    CircuitBreakerRegistry, NodeExecutionOutcome, NodePolicy, execute_with_policy,
    new_circuit_registry,
};

/// Type alias for node ID
pub type NodeId = String;
const SEND_BRANCH_ID_META_KEY: &str = "__send_branch_id";
const SEND_SOURCE_NODE_META_KEY: &str = "__send_source_node";

#[derive(Debug, Clone)]
struct ExecutionItem {
    node_id: String,
    send_input: Option<Value>,
    branch_id: Option<String>,
    source_node: Option<String>,
}

impl ExecutionItem {
    fn plain(node_id: impl Into<String>) -> Self {
        Self {
            node_id: node_id.into(),
            send_input: None,
            branch_id: None,
            source_node: None,
        }
    }
}

/// StateGraph implementation - LangGraph-inspired API
///
/// # Example
///
/// ```rust,ignore
/// use mofa_foundation::workflow::{StateGraphImpl, AppendReducer, OverwriteReducer};
/// use mofa_kernel::workflow::{StateGraph, START, END};
///
/// let graph = StateGraphImpl::<MyState>::new("my_workflow")
///     .add_reducer("messages", Box::new(AppendReducer))
///     .add_node("process", Box::new(ProcessNode))
///     .add_edge(START, "process")
///     .add_edge("process", END)
///     .compile()?;
/// ```
pub struct StateGraphImpl<S: GraphState> {
    /// Graph ID
    id: String,
    /// Node functions
    nodes: HashMap<NodeId, Box<dyn NodeFunc<S>>>,
    /// Edges: source -> target(s)
    edges: HashMap<NodeId, EdgeTarget>,
    /// Reducers for state keys
    reducers: HashMap<String, Box<dyn Reducer>>,
    /// Entry point (first node after START)
    entry_point: Option<NodeId>,
    /// Finish points (nodes that connect to END)
    finish_points: Vec<NodeId>,
    /// 图配置
    /// Graph configuration
    config: GraphConfig,
    /// 每节点的容错策略
    /// Per-node fault-tolerance policies
    policies: HashMap<NodeId, NodePolicy>,
}

impl<S: GraphState> StateGraphImpl<S> {
    /// Create a new StateGraph builder with the given ID
    pub fn build(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            nodes: HashMap::new(),
            edges: HashMap::new(),
            reducers: HashMap::new(),
            entry_point: None,
            finish_points: Vec::new(),
            config: GraphConfig::default(),
            policies: HashMap::new(),
        }
    }

    /// Get the number of nodes
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Get the number of edges
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Get all node IDs
    pub fn node_ids(&self) -> Vec<&str> {
        self.nodes.keys().map(|s| s.as_str()).collect()
    }

    /// Validate the graph structure
    pub fn validate(&self) -> AgentResult<()> {
        let mut errors = Vec::new();

        // Check entry point
        if self.entry_point.is_none() {
            errors.push(
                "No entry point set. Use set_entry_point() or add_edge(START, node).".to_string(),
            );
        }

        // Check that all nodes are reachable
        if let Some(entry) = &self.entry_point {
            let reachable = self.find_reachable_nodes(entry);
            for node_id in self.nodes.keys() {
                if !reachable.contains(node_id) && node_id != entry {
                    errors.push(format!(
                        "Node '{}' is not reachable from entry point",
                        node_id
                    ));
                }
            }
        }

        // Check that edges reference valid nodes
        for (from, target) in &self.edges {
            if from != START && !self.nodes.contains_key(from) {
                errors.push(format!("Edge source '{}' does not exist", from));
            }
            let targets = target.targets();
            for target_id in targets {
                if target_id != END && !self.nodes.contains_key(target_id) {
                    errors.push(format!("Edge target '{}' does not exist", target_id));
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(AgentError::ValidationFailed(errors.join("; ")))
        }
    }

    /// Find all nodes reachable from a starting node
    fn find_reachable_nodes(&self, start: &str) -> HashSet<String> {
        let mut reachable = HashSet::new();
        let mut stack = vec![start.to_string()];

        while let Some(node_id) = stack.pop() {
            if reachable.insert(node_id.clone())
                && let Some(edge_target) = self.edges.get(&node_id)
            {
                let targets = edge_target.targets();
                for target in targets {
                    if target != END && !reachable.contains(target) {
                        stack.push(target.to_string());
                    }
                }
            }
        }

        reachable
    }

    /// 为特定节点附加容错策略
    /// Attach a fault-tolerance policy to a specific node.
    ///
    /// 策略是可选的：没有策略的节点以默认行为执行
    /// （无重试，无断路器）。
    /// Policies are optional: nodes without a policy execute with default
    /// behavior (no retry, no circuit breaker).
    pub fn with_policy(&mut self, node_id: impl Into<String>, policy: NodePolicy) -> &mut Self {
        self.policies.insert(node_id.into(), policy);
        self
    }
}

#[async_trait]
impl<S: GraphState + 'static> mofa_kernel::workflow::StateGraph for StateGraphImpl<S> {
    type State = S;
    type Compiled = CompiledGraphImpl<S>;

    fn new(id: impl Into<String>) -> Self {
        Self::build(id)
    }

    fn add_node(&mut self, id: impl Into<String>, node: Box<dyn NodeFunc<S>>) -> &mut Self {
        let node_id = id.into();
        debug!("Adding node '{}' to graph '{}'", node_id, self.id);
        self.nodes.insert(node_id, node);
        self
    }

    fn add_edge(&mut self, from: impl Into<String>, to: impl Into<String>) -> &mut Self {
        let from_id = from.into();
        let to_id = to.into();

        debug!("Adding edge: {} -> {}", from_id, to_id);

        // Handle START edge (entry point)
        if from_id == START {
            self.entry_point = Some(to_id.clone());
            return self;
        }

        // Handle END edge (finish point)
        if to_id == END {
            if !self.finish_points.contains(&from_id) {
                self.finish_points.push(from_id.clone());
            }
            return self;
        }

        // Regular edge
        match self.edges.get_mut(&from_id) {
            Some(EdgeTarget::Parallel(targets)) => {
                targets.push(to_id);
            }
            Some(EdgeTarget::Single(existing)) => {
                let existing = existing.clone();
                self.edges
                    .insert(from_id, EdgeTarget::parallel(vec![existing, to_id]));
            }
            Some(EdgeTarget::Conditional(_)) => {
                warn!(
                    "Overwriting conditional edges with single edge for '{}'",
                    from_id
                );
                self.edges.insert(from_id, EdgeTarget::single(to_id));
            }
            None => {
                self.edges.insert(from_id, EdgeTarget::single(to_id));
            }
            _ => {
                warn!("Unhandled EdgeTarget variant for '{}'", from_id);
            }
        }

        self
    }

    fn add_conditional_edges(
        &mut self,
        from: impl Into<String>,
        conditions: HashMap<String, String>,
    ) -> &mut Self {
        let from_id = from.into();
        debug!(
            "Adding conditional edges from '{}': {:?}",
            from_id, conditions
        );
        self.edges
            .insert(from_id, EdgeTarget::conditional(conditions));
        self
    }

    fn add_parallel_edges(&mut self, from: impl Into<String>, targets: Vec<String>) -> &mut Self {
        let from_id = from.into();
        debug!("Adding parallel edges from '{}': {:?}", from_id, targets);
        self.edges.insert(from_id, EdgeTarget::parallel(targets));
        self
    }

    fn set_entry_point(&mut self, node: impl Into<String>) -> &mut Self {
        let node_id = node.into();
        debug!("Setting entry point to '{}'", node_id);
        self.entry_point = Some(node_id);
        self
    }

    fn set_finish_point(&mut self, node: impl Into<String>) -> &mut Self {
        let node_id = node.into();
        debug!("Setting finish point at '{}'", node_id);
        if !self.finish_points.contains(&node_id) {
            self.finish_points.push(node_id);
        }
        self
    }

    fn add_reducer(&mut self, key: impl Into<String>, reducer: Box<dyn Reducer>) -> &mut Self {
        let key_str = key.into();
        debug!(
            "Adding reducer for key '{}' of type {:?}",
            key_str,
            reducer.reducer_type()
        );
        self.reducers.insert(key_str, reducer);
        self
    }

    fn with_config(&mut self, config: GraphConfig) -> &mut Self {
        self.config = config;
        self
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn compile(self) -> AgentResult<CompiledGraphImpl<S>> {
        info!("Compiling graph '{}'", self.id);

        // Validate
        self.validate()?;

        // Validate fallback node references in policies
        for (node_id, policy) in &self.policies {
            if !self.nodes.contains_key(node_id) {
                return Err(AgentError::ValidationFailed(format!(
                    "Policy references non-existent node '{}'",
                    node_id
                )));
            }
            if let Some(ref fallback) = policy.fallback_node
                && !self.nodes.contains_key(fallback)
            {
                return Err(AgentError::ValidationFailed(format!(
                    "Fallback node '{}' for node '{}' does not exist in graph",
                    fallback, node_id
                )));
            }
        }

        // 创建已编译的图
        // Create compiled graph
        let max_par = self.config.max_parallelism.max(1);
        Ok(CompiledGraphImpl {
            id: self.id,
            nodes: Arc::new(
                self.nodes
                    .into_iter()
                    .map(|(node_id, node)| (node_id, Arc::from(node)))
                    .collect(),
            ),
            edges: Arc::new(self.edges),
            reducers: Arc::new(self.reducers),
            entry_point: self.entry_point.expect("Entry point should be validated"),
            config: self.config,
            policies: Arc::new(self.policies),
            circuit_states: new_circuit_registry(),
            parallelism_semaphore: Arc::new(Semaphore::new(max_par)),
        })
    }
}

/// 已编译的可执行图
/// Compiled graph ready for execution
pub struct CompiledGraphImpl<S: GraphState> {
    /// 图 ID / Graph ID
    id: String,
    /// 节点函数 / Node functions
    nodes: Arc<HashMap<NodeId, Arc<dyn NodeFunc<S>>>>,
    /// 边 / Edges
    edges: Arc<HashMap<NodeId, EdgeTarget>>,
    /// 归约器 / Reducers
    reducers: Arc<HashMap<String, Box<dyn Reducer>>>,
    /// 入口点 / Entry point
    entry_point: NodeId,
    /// 配置 / Configuration
    config: GraphConfig,
    /// 每节点的容错策略 / Per-node fault-tolerance policies
    policies: Arc<HashMap<NodeId, NodePolicy>>,
    /// 每节点的断路器状态（跨调用共享）
    /// Per-node circuit breaker state (shared across invocations)
    circuit_states: CircuitBreakerRegistry,
    /// 并行分支的并发信号量
    /// Concurrency semaphore for parallel branches
    parallelism_semaphore: Arc<Semaphore>,
}

impl<S: GraphState> CompiledGraphImpl<S> {
    fn build_node_context(
        base: &RuntimeContext,
        node_id: &str,
        execution_item: &ExecutionItem,
    ) -> RuntimeContext {
        let mut metadata = base.metadata.clone();
        if let Some(branch_id) = &execution_item.branch_id {
            metadata.insert(
                SEND_BRANCH_ID_META_KEY.to_string(),
                Value::String(branch_id.clone()),
            );
        }
        if let Some(source_node) = &execution_item.source_node {
            metadata.insert(
                SEND_SOURCE_NODE_META_KEY.to_string(),
                Value::String(source_node.clone()),
            );
        }

        RuntimeContext {
            execution_id: base.execution_id.clone(),
            graph_id: base.graph_id.clone(),
            current_node: Arc::new(RwLock::new(node_id.to_string())),
            remaining_steps: base.remaining_steps.clone(),
            config: base.config.clone(),
            metadata,
            parent_execution_id: base.parent_execution_id.clone(),
            tags: base.tags.clone(),
        }
    }

    /// 并行执行多个节点，强制执行并发限制
    /// Execute multiple nodes in parallel, enforcing max_parallelism via semaphore.
    ///
    /// NOTE: Parallel nodes run without per-node retry/circuit-breaker protection.
    /// Each node executes against an isolated state snapshot. This is by design:
    /// parallel fan-out patterns prioritize throughput over individual node
    /// resilience. If retry is needed, place a single-node step before/after
    /// the parallel fan-out.
    async fn execute_parallel_nodes(
        nodes: &HashMap<NodeId, Arc<dyn NodeFunc<S>>>,
        execution_items: &[ExecutionItem],
        base_state: &S,
        base_ctx: &RuntimeContext,
        semaphore: &Arc<Semaphore>,
    ) -> AgentResult<Vec<(String, Command)>> {
        let mut join_set = JoinSet::new();

        for (index, item) in execution_items.iter().enumerate() {
            let node_id = &item.node_id;
            if item.send_input.is_some() {
                debug!(
                    target_node = %node_id,
                    branch_id = ?item.branch_id,
                    source_node = ?item.source_node,
                    "Executing Send branch in parallel"
                );
            }
            let node = nodes
                .get(node_id)
                .ok_or_else(|| AgentError::NotFound(format!("Node '{}'", node_id)))?
                .clone();
            // Each parallel node runs against an isolated snapshot. Node-side mutations are
            // intentionally sandboxed; shared-state changes must be expressed via Command updates.
            let mut isolated_state = match &item.send_input {
                Some(send_input) => Self::merge_send_input(base_state, send_input)?,
                None => base_state.clone(),
            };
            let node_ctx = Self::build_node_context(base_ctx, node_id, item);
            let node_id = node_id.clone();
            let sem = semaphore.clone();

            join_set.spawn(async move {
                // Acquire semaphore permit to enforce max_parallelism
                let _permit = sem.acquire().await.map_err(|_| {
                    AgentError::Internal("Parallelism semaphore closed".to_string())
                })?;
                let command = node.call(&mut isolated_state, &node_ctx).await?;
                Ok::<(usize, String, Command), AgentError>((index, node_id, command))
            });
        }

        let mut ordered_results: Vec<Option<(String, Command)>> = vec![None; execution_items.len()];
        while let Some(joined) = join_set.join_next().await {
            let (index, node_id, command) = joined
                .map_err(|e| AgentError::Internal(format!("Parallel node task failed: {}", e)))??;
            ordered_results[index] = Some((node_id, command));
        }

        ordered_results
            .into_iter()
            .map(|entry| {
                entry.ok_or_else(|| {
                    AgentError::Internal(
                        "Parallel node execution returned incomplete results".to_string(),
                    )
                })
            })
            .collect()
    }

    fn merge_send_input(base_state: &S, send_input: &Value) -> AgentResult<S> {
        let mut merged = base_state.to_json()?;
        let merged_map = merged.as_object_mut().ok_or_else(|| {
            AgentError::InvalidInput(
                "Send requires GraphState::to_json() to return a JSON object".to_string(),
            )
        })?;
        let input_map = send_input.as_object().ok_or_else(|| {
            AgentError::InvalidInput(
                "SendCommand::input must be a JSON object for state injection".to_string(),
            )
        })?;
        // Send payloads are merged as a shallow overlay for the branch-local state.
        // Keys in the payload overwrite same-named keys from the base state snapshot.
        for (k, v) in input_map {
            merged_map.insert(k.clone(), v.clone());
        }
        S::from_json(merged)
    }

    fn dedupe_plain_execution_items(items: Vec<ExecutionItem>) -> Vec<ExecutionItem> {
        let mut seen_plain = HashSet::new();
        let mut deduped = Vec::new();
        for item in items {
            if item.send_input.is_none() {
                if seen_plain.insert(item.node_id.clone()) {
                    deduped.push(item);
                }
            } else {
                deduped.push(item);
            }
        }
        deduped
    }

    fn get_next_execution_items_from_edges(
        edges: &HashMap<NodeId, EdgeTarget>,
        current_node: &str,
        command: &Command,
    ) -> Vec<ExecutionItem> {
        match &command.control {
            ControlFlow::Goto(target) => vec![ExecutionItem::plain(target.clone())],
            ControlFlow::Return => vec![], // End execution
            ControlFlow::Send(sends) => {
                // MapReduce: create branches for each send target
                sends
                    .iter()
                    .map(|s| ExecutionItem {
                        node_id: s.target.clone(),
                        send_input: Some(s.input.clone()),
                        branch_id: s.branch_id.clone(),
                        source_node: Some(current_node.to_string()),
                    })
                    .collect()
            }
            ControlFlow::Continue => {
                // Follow graph edges
                match edges.get(current_node) {
                    Some(EdgeTarget::Single(target)) => vec![ExecutionItem::plain(target.clone())],
                    Some(EdgeTarget::Parallel(targets)) => {
                        targets.iter().cloned().map(ExecutionItem::plain).collect()
                    }
                    Some(EdgeTarget::Conditional(routes)) => {
                        // Priority 1: explicit route decision on command
                        if let Some(ref decision) = command.route
                            && let Some(target) = routes.get(decision)
                        {
                            return vec![ExecutionItem::plain(target.clone())];
                        }
                        // Priority 2: legacy key-name matching for backward compatibility
                        // Find matching route based on state updates
                        for update in &command.updates {
                            if let Some(target) = routes.get(&update.key) {
                                return vec![ExecutionItem::plain(target.clone())];
                            }
                        }
                        // Default to first route if no match
                        routes
                            .values()
                            .next()
                            .map(|t: &String| vec![ExecutionItem::plain(t.clone())])
                            .unwrap_or_default()
                    }
                    None => vec![],
                    _ => vec![],
                }
            }
            _ => vec![],
        }
    }

    /// Get the next node(s) based on the current node and command
    fn get_next_nodes(&self, current_node: &str, command: &Command) -> Vec<String> {
        Self::get_next_execution_items_from_edges(self.edges.as_ref(), current_node, command)
            .into_iter()
            .map(|item| item.node_id)
            .collect()
    }

    /// Apply state updates using reducers
    async fn apply_updates(&self, state: &mut S, updates: &[StateUpdate]) -> AgentResult<()> {
        for update in updates {
            let current = state.get_value(&update.key);

            // Get or create reducer
            let new_value = if let Some(reducer) = self.reducers.get(&update.key) {
                reducer.reduce(current.as_ref(), &update.value).await?
            } else {
                // Default: overwrite
                update.value.clone()
            };

            state.apply_update(&update.key, new_value).await?;
        }
        Ok(())
    }
}

#[async_trait]
impl<S: GraphState + 'static> CompiledGraph<S, serde_json::Value> for CompiledGraphImpl<S> {
    fn id(&self) -> &str {
        &self.id
    }

    async fn invoke(&self, input: S, config: Option<RuntimeContext>) -> AgentResult<S> {
        let ctx =
            config.unwrap_or_else(|| RuntimeContext::with_config(&self.id, self.config.clone()));

        info!(
            "Starting graph execution '{}' with execution_id={}",
            self.id, ctx.execution_id
        );

        let mut state = input;
        let mut current_nodes = vec![ExecutionItem::plain(self.entry_point.clone())];
        let default_policy = NodePolicy::default();

        while !current_nodes.is_empty() {
            // Check recursion limit
            if ctx.is_recursion_limit_reached().await {
                return Err(AgentError::Internal("Recursion limit reached".to_string()));
            }
            ctx.decrement_steps().await;

            // Execute nodes
            if current_nodes.len() == 1 && current_nodes[0].send_input.is_none() {
                // Single node execution
                let execution_item = current_nodes.remove(0);
                let node_id = execution_item.node_id;
                let node = self
                    .nodes
                    .get(&node_id)
                    .ok_or_else(|| AgentError::NotFound(format!("Node '{}'", node_id)))?;

                ctx.set_current_node(&node_id).await;
                debug!("Executing node '{}' in graph '{}'", node_id, self.id);

                let policy = self.policies.get(&node_id).unwrap_or(&default_policy);

                let command = match execute_with_policy(
                    node.as_ref(),
                    &mut state,
                    &ctx,
                    policy,
                    &self.circuit_states,
                    &node_id,
                    None, // no event channel in invoke()
                )
                .await
                {
                    Ok(cmd) => cmd,
                    Err(NodeExecutionOutcome::Fallback(fallback_id)) => {
                        debug!("Node '{}' falling back to '{}'", node_id, fallback_id);
                        current_nodes = vec![ExecutionItem::plain(fallback_id)];
                        continue;
                    }
                    Err(NodeExecutionOutcome::Error(e)) => return Err(e),
                };

                // Apply updates
                self.apply_updates(&mut state, &command.updates).await?;

                // Get next nodes
                current_nodes = Self::get_next_execution_items_from_edges(
                    self.edges.as_ref(),
                    &node_id,
                    &command,
                );

                debug!(
                    "Node '{}' completed, next nodes: {:?}",
                    node_id,
                    current_nodes
                        .iter()
                        .map(|item| item.node_id.as_str())
                        .collect::<Vec<_>>()
                );
            } else {
                // Parallel execution
                let mut next_nodes = Vec::new();
                let nodes_to_execute = std::mem::take(&mut current_nodes);
                let parallel_results = Self::execute_parallel_nodes(
                    self.nodes.as_ref(),
                    &nodes_to_execute,
                    &state,
                    &ctx,
                    &self.parallelism_semaphore,
                )
                .await?;

                for (node_id, command) in parallel_results {
                    debug!("Applying updates from parallel node '{}'", node_id);

                    // Apply updates only after all parallel nodes have completed.
                    self.apply_updates(&mut state, &command.updates).await?;

                    // Collect next execution items
                    let next = Self::get_next_execution_items_from_edges(
                        self.edges.as_ref(),
                        &node_id,
                        &command,
                    );
                    next_nodes.extend(next);
                }

                // Deduplicate only plain node targets; keep all Send branches.
                current_nodes = Self::dedupe_plain_execution_items(next_nodes);
            }
        }

        info!("Graph '{}' execution completed", self.id);
        Ok(state)
    }

    fn stream(
        &self,
        input: S,
        config: Option<RuntimeContext<serde_json::Value>>,
    ) -> mofa_kernel::workflow::graph::GraphStream<'_, S, serde_json::Value> {
        let ctx =
            config.unwrap_or_else(|| RuntimeContext::with_config(&self.id, self.config.clone()));

        let nodes = self.nodes.clone();
        let reducers = self.reducers.clone();
        let edges = self.edges.clone();
        let entry_point = self.entry_point.clone();
        let policies = self.policies.clone();
        let circuit_states = self.circuit_states.clone();
        let semaphore = self.parallelism_semaphore.clone();

        // Create a channel for streaming events
        let (tx, rx) = tokio::sync::mpsc::channel(100);

        // Spawn execution task
        tokio::spawn(async move {
            let mut state = input;
            let mut current_nodes = vec![ExecutionItem::plain(entry_point)];
            let mut iteration_count = 0;
            const MAX_ITERATIONS: usize = 20;
            let default_policy = NodePolicy::default();

            // Helper function to get next execution items based on command and edges
            let get_next_execution_items =
                |current_node: &str, command: &Command| -> Vec<ExecutionItem> {
                    Self::get_next_execution_items_from_edges(edges.as_ref(), current_node, command)
                };

            while !current_nodes.is_empty() {
                // Check iteration limit
                iteration_count += 1;
                if iteration_count > MAX_ITERATIONS {
                    let _ = tx
                        .send(Err(AgentError::Internal(format!(
                            "Maximum iterations ({}) reached",
                            MAX_ITERATIONS
                        ))))
                        .await;
                    return;
                }

                // Check recursion limit
                if ctx.remaining_steps.is_exhausted().await {
                    let _ = tx
                        .send(Err(AgentError::Internal(
                            "Recursion limit reached".to_string(),
                        )))
                        .await;
                    return;
                }
                ctx.remaining_steps.decrement().await;

                let nodes_to_execute = std::mem::take(&mut current_nodes);
                let mut next_nodes = Vec::new();

                if nodes_to_execute.len() == 1 && nodes_to_execute[0].send_input.is_none() {
                    let execution_item = nodes_to_execute[0].clone();
                    let node_id = execution_item.node_id;
                    let node = match nodes.get(&node_id) {
                        Some(n) => n,
                        None => {
                            let _ = tx
                                .send(Err(AgentError::NotFound(format!("Node '{}'", node_id))))
                                .await;
                            return;
                        }
                    };

                    ctx.set_current_node(&node_id).await;

                    // Send start event
                    let _ = tx
                        .send(Ok(StreamEvent::NodeStart {
                            node_id: node_id.clone(),
                            state: state.clone(),
                        }))
                        .await;

                    // 使用重试/断路器执行节点
                    // Execute node with retry/circuit-breaker
                    let policy = policies.get(&node_id).unwrap_or(&default_policy);
                    let command = match execute_with_policy(
                        node.as_ref(),
                        &mut state,
                        &ctx,
                        policy,
                        &circuit_states,
                        &node_id,
                        Some(&tx),
                    )
                    .await
                    {
                        Ok(cmd) => cmd,
                        Err(NodeExecutionOutcome::Fallback(fallback_id)) => {
                            // Route to fallback node on next iteration
                            // (execute_with_policy already emitted NodeFallback event)
                            next_nodes.push(ExecutionItem::plain(fallback_id));
                            current_nodes = Self::dedupe_plain_execution_items(next_nodes);
                            continue;
                        }
                        Err(NodeExecutionOutcome::Error(e)) => {
                            let _ = tx
                                .send(Ok(StreamEvent::Error {
                                    node_id: Some(node_id),
                                    error: e.to_string(),
                                }))
                                .await;
                            return;
                        }
                    };

                    for update in &command.updates {
                        let current = state.get_value(&update.key);
                        let new_value = if let Some(reducer) = reducers.get(&update.key) {
                            match reducer.reduce(current.as_ref(), &update.value).await {
                                Ok(v) => v,
                                Err(e) => {
                                    let _ = tx
                                        .send(Ok(StreamEvent::Error {
                                            node_id: Some(node_id.clone()),
                                            error: e.to_string(),
                                        }))
                                        .await;
                                    return;
                                }
                            }
                        } else {
                            update.value.clone()
                        };
                        if let Err(e) = state.apply_update(&update.key, new_value).await {
                            let _ = tx
                                .send(Ok(StreamEvent::Error {
                                    node_id: Some(node_id.clone()),
                                    error: e.to_string(),
                                }))
                                .await;
                            return;
                        }
                    }

                    // Send end event
                    let _ = tx
                        .send(Ok(StreamEvent::NodeEnd {
                            node_id: node_id.clone(),
                            state: state.clone(),
                            command: command.clone(),
                        }))
                        .await;

                    next_nodes.extend(get_next_execution_items(&node_id, &command));
                } else {
                    for execution_item in &nodes_to_execute {
                        let start_state = match &execution_item.send_input {
                            Some(send_input) => match Self::merge_send_input(&state, send_input) {
                                Ok(merged) => merged,
                                Err(e) => {
                                    let _ = tx
                                        .send(Ok(StreamEvent::Error {
                                            node_id: Some(execution_item.node_id.clone()),
                                            error: e.to_string(),
                                        }))
                                        .await;
                                    return;
                                }
                            },
                            None => state.clone(),
                        };
                        let _ = tx
                            .send(Ok(StreamEvent::NodeStart {
                                node_id: execution_item.node_id.clone(),
                                state: start_state,
                            }))
                            .await;
                    }

                    let commands = match Self::execute_parallel_nodes(
                        nodes.as_ref(),
                        &nodes_to_execute,
                        &state,
                        &ctx,
                        &semaphore,
                    )
                    .await
                    {
                        Ok(results) => results,
                        Err(e) => {
                            let _ = tx
                                .send(Ok(StreamEvent::Error {
                                    node_id: None,
                                    error: e.to_string(),
                                }))
                                .await;
                            return;
                        }
                    };

                    for (node_id, command) in commands {
                        for update in &command.updates {
                            let current = state.get_value(&update.key);
                            let new_value = if let Some(reducer) = reducers.get(&update.key) {
                                match reducer.reduce(current.as_ref(), &update.value).await {
                                    Ok(v) => v,
                                    Err(e) => {
                                        let _ = tx
                                            .send(Ok(StreamEvent::Error {
                                                node_id: Some(node_id.clone()),
                                                error: e.to_string(),
                                            }))
                                            .await;
                                        return;
                                    }
                                }
                            } else {
                                update.value.clone()
                            };
                            if let Err(e) = state.apply_update(&update.key, new_value).await {
                                let _ = tx
                                    .send(Ok(StreamEvent::Error {
                                        node_id: Some(node_id.clone()),
                                        error: e.to_string(),
                                    }))
                                    .await;
                                return;
                            }
                        }

                        let _ = tx
                            .send(Ok(StreamEvent::NodeEnd {
                                node_id: node_id.clone(),
                                state: state.clone(),
                                command: command.clone(),
                            }))
                            .await;

                        next_nodes.extend(get_next_execution_items(&node_id, &command));
                    }
                }

                // Deduplicate only plain node targets; keep all Send branches.
                current_nodes = Self::dedupe_plain_execution_items(next_nodes);
            }

            // Send final event
            let _ = tx.send(Ok(StreamEvent::End { final_state: state })).await;
        });

        // Convert receiver to stream
        Box::pin(tokio_stream::wrappers::ReceiverStream::new(rx))
    }

    async fn step(
        &self,
        input: S,
        config: Option<RuntimeContext<serde_json::Value>>,
    ) -> AgentResult<StepResult<S, serde_json::Value>> {
        let ctx =
            config.unwrap_or_else(|| RuntimeContext::with_config(&self.id, self.config.clone()));

        let mut state = input;

        // Get current node from context or use entry point
        let current_node_id = ctx.current_node().await;
        let node_id = if current_node_id.is_empty() {
            self.entry_point.clone()
        } else {
            current_node_id
        };

        let node = self
            .nodes
            .get(&node_id)
            .ok_or_else(|| AgentError::NotFound(format!("Node '{}'", node_id)))?;

        ctx.set_current_node(&node_id).await;
        let command = node.call(&mut state, &ctx).await?;

        // Apply updates
        self.apply_updates(&mut state, &command.updates).await?;

        // Get next nodes
        let next_nodes = self.get_next_nodes(&node_id, &command);
        let is_complete = next_nodes.is_empty();
        let next_node = next_nodes.into_iter().next();

        Ok(StepResult {
            state,
            node_id,
            command,
            is_complete,
            next_node,
        })
    }

    fn validate_state(&self, _state: &S) -> AgentResult<()> {
        // Default implementation - no validation
        Ok(())
    }

    fn state_schema(&self) -> HashMap<String, String> {
        self.reducers
            .iter()
            .map(|(k, r)| (k.clone(), r.reducer_type().to_string()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;
    use mofa_kernel::workflow::{JsonState, SendCommand, StateGraph};
    use serde_json::json;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::time::{Duration, sleep};

    // Simple test node
    struct TestNode {
        name: String,
        updates: Vec<StateUpdate>,
    }

    #[async_trait]
    impl NodeFunc<JsonState> for TestNode {
        async fn call(
            &self,
            _state: &mut JsonState,
            _ctx: &RuntimeContext,
        ) -> AgentResult<Command> {
            let mut cmd = Command::new();
            for update in &self.updates {
                cmd = cmd.update(update.key.clone(), update.value.clone());
            }
            Ok(cmd.continue_())
        }

        fn name(&self) -> &str {
            &self.name
        }
    }

    struct StaticCommandNode {
        name: String,
        command: Command,
    }

    #[async_trait]
    impl NodeFunc<JsonState> for StaticCommandNode {
        async fn call(
            &self,
            _state: &mut JsonState,
            _ctx: &RuntimeContext,
        ) -> AgentResult<Command> {
            Ok(self.command.clone())
        }

        fn name(&self) -> &str {
            &self.name
        }
    }

    struct SendMapperNode {
        sends: Vec<SendCommand>,
    }

    #[async_trait]
    impl NodeFunc<JsonState> for SendMapperNode {
        async fn call(
            &self,
            _state: &mut JsonState,
            _ctx: &RuntimeContext,
        ) -> AgentResult<Command> {
            Ok(Command::send(self.sends.clone()))
        }

        fn name(&self) -> &str {
            "send_mapper"
        }
    }

    struct SendWorkerNode;

    #[async_trait]
    impl NodeFunc<JsonState> for SendWorkerNode {
        async fn call(&self, state: &mut JsonState, _ctx: &RuntimeContext) -> AgentResult<Command> {
            let task = state.get_value::<i64>("task").ok_or_else(|| {
                AgentError::InvalidInput("task missing in worker state".to_string())
            })?;
            Ok(Command::new().update("tasks_seen", json!(task)).return_())
        }

        fn name(&self) -> &str {
            "send_worker"
        }
    }

    struct SendMetadataWorkerNode;

    #[async_trait]
    impl NodeFunc<JsonState> for SendMetadataWorkerNode {
        async fn call(&self, _state: &mut JsonState, ctx: &RuntimeContext) -> AgentResult<Command> {
            let branch_id = ctx
                .metadata
                .get(SEND_BRANCH_ID_META_KEY)
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    AgentError::InvalidInput(
                        "missing send branch metadata in runtime context".to_string(),
                    )
                })?;

            let source_node = ctx
                .metadata
                .get(SEND_SOURCE_NODE_META_KEY)
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    AgentError::InvalidInput(
                        "missing send source metadata in runtime context".to_string(),
                    )
                })?;

            Ok(Command::new()
                .update("branch_ids", json!(branch_id))
                .update("send_sources", json!(source_node))
                .return_())
        }

        fn name(&self) -> &str {
            "send_metadata_worker"
        }
    }

    struct ConcurrencyProbeNode {
        name: String,
        active: Arc<AtomicUsize>,
        max_active: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl NodeFunc<JsonState> for ConcurrencyProbeNode {
        async fn call(
            &self,
            _state: &mut JsonState,
            _ctx: &RuntimeContext,
        ) -> AgentResult<Command> {
            let concurrent = self.active.fetch_add(1, Ordering::SeqCst) + 1;
            self.max_active.fetch_max(concurrent, Ordering::SeqCst);
            sleep(Duration::from_millis(100)).await;
            self.active.fetch_sub(1, Ordering::SeqCst);

            Ok(Command::new().continue_())
        }

        fn name(&self) -> &str {
            &self.name
        }
    }

    struct FlagReaderNode;

    #[async_trait]
    impl NodeFunc<JsonState> for FlagReaderNode {
        async fn call(&self, state: &mut JsonState, _ctx: &RuntimeContext) -> AgentResult<Command> {
            let saw_flag = state.get_value::<bool>("flag").unwrap_or(false);
            Ok(Command::new()
                .update("reader_saw_flag", json!(saw_flag))
                .continue_())
        }

        fn name(&self) -> &str {
            "flag_reader"
        }
    }

    #[tokio::test]
    async fn test_state_graph_build_and_compile() {
        let mut graph = StateGraphImpl::<JsonState>::new("test_graph");

        graph
            .add_node(
                "start_node",
                Box::new(TestNode {
                    name: "start".to_string(),
                    updates: vec![StateUpdate::new("initialized", json!(true))],
                }),
            )
            .add_node(
                "end_node",
                Box::new(TestNode {
                    name: "end".to_string(),
                    updates: vec![StateUpdate::new("completed", json!(true))],
                }),
            )
            .add_edge(START, "start_node")
            .add_edge("start_node", "end_node")
            .add_edge("end_node", END);

        let compiled = graph.compile();
        assert!(compiled.is_ok());
    }

    #[tokio::test]
    async fn test_state_graph_no_entry_point() {
        let mut graph = StateGraphImpl::<JsonState>::new("test_graph");

        graph.add_node(
            "node1",
            Box::new(TestNode {
                name: "node1".to_string(),
                updates: vec![],
            }),
        );

        let result = graph.compile();
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_compiled_graph_invoke() {
        let mut graph = StateGraphImpl::<JsonState>::new("test_graph");

        graph
            .add_node(
                "process",
                Box::new(TestNode {
                    name: "process".to_string(),
                    updates: vec![
                        StateUpdate::new("processed", json!(true)),
                        StateUpdate::new("count", json!(1)),
                    ],
                }),
            )
            .add_edge(START, "process")
            .add_edge("process", END);

        let compiled = graph.compile().unwrap();

        let initial_state = JsonState::new();
        let result = compiled.invoke(initial_state, None).await;

        assert!(result.is_ok());
        let final_state = result.unwrap();
        assert_eq!(final_state.get_value("processed"), Some(json!(true)));
        assert_eq!(final_state.get_value("count"), Some(json!(1)));
    }

    #[tokio::test]
    async fn test_parallel_nodes_execute_concurrently_in_invoke() {
        let active = Arc::new(AtomicUsize::new(0));
        let max_active = Arc::new(AtomicUsize::new(0));
        let mut graph = StateGraphImpl::<JsonState>::new("parallel_concurrency");

        graph
            .add_node(
                "fan_out",
                Box::new(TestNode {
                    name: "fan_out".to_string(),
                    updates: vec![],
                }),
            )
            .add_node(
                "node_a",
                Box::new(ConcurrencyProbeNode {
                    name: "node_a".to_string(),
                    active: active.clone(),
                    max_active: max_active.clone(),
                }),
            )
            .add_node(
                "node_b",
                Box::new(ConcurrencyProbeNode {
                    name: "node_b".to_string(),
                    active: active.clone(),
                    max_active: max_active.clone(),
                }),
            )
            .add_node(
                "node_c",
                Box::new(ConcurrencyProbeNode {
                    name: "node_c".to_string(),
                    active,
                    max_active: max_active.clone(),
                }),
            )
            .add_edge(START, "fan_out")
            .add_parallel_edges(
                "fan_out",
                vec![
                    "node_a".to_string(),
                    "node_b".to_string(),
                    "node_c".to_string(),
                ],
            );

        let compiled = graph.compile().unwrap();
        compiled.invoke(JsonState::new(), None).await.unwrap();

        assert!(
            max_active.load(Ordering::SeqCst) > 1,
            "parallel nodes should overlap in execution"
        );
    }

    #[tokio::test]
    async fn test_parallel_nodes_use_state_snapshot_in_invoke_and_stream() {
        let mut graph = StateGraphImpl::<JsonState>::new("parallel_state_snapshot");

        graph
            .add_node(
                "fan_out",
                Box::new(TestNode {
                    name: "fan_out".to_string(),
                    updates: vec![],
                }),
            )
            .add_node(
                "writer",
                Box::new(TestNode {
                    name: "writer".to_string(),
                    updates: vec![StateUpdate::new("flag", json!(true))],
                }),
            )
            .add_node("reader", Box::new(FlagReaderNode))
            .add_edge(START, "fan_out")
            .add_parallel_edges("fan_out", vec!["writer".to_string(), "reader".to_string()]);

        let compiled: CompiledGraphImpl<JsonState> = graph.compile().unwrap();

        let final_state = compiled.invoke(JsonState::new(), None).await.unwrap();
        assert_eq!(final_state.get_value("flag"), Some(json!(true)));
        assert_eq!(final_state.get_value("reader_saw_flag"), Some(json!(false)));

        let mut stream = compiled.stream(JsonState::new(), None);
        let mut stream_final_state: Option<JsonState> = None;
        while let Some(event) = stream.next().await {
            if let Ok(StreamEvent::End { final_state }) = event {
                stream_final_state = Some(final_state);
            }
        }

        let stream_final_state: JsonState =
            stream_final_state.expect("stream should emit a final end event with state");
        assert_eq!(stream_final_state.get_value("flag"), Some(json!(true)));
        assert_eq!(
            stream_final_state.get_value("reader_saw_flag"),
            Some(json!(false))
        );
    }

    #[tokio::test]
    async fn test_conditional_routing_uses_route_value() {
        let mut graph = StateGraphImpl::<JsonState>::new("route_value");
        let mut routes = HashMap::new();
        routes.insert("approve".to_string(), "approved".to_string());
        routes.insert("reject".to_string(), "rejected".to_string());

        graph
            .add_node(
                "router",
                Box::new(StaticCommandNode {
                    name: "router".to_string(),
                    command: Command::new().route("approve").continue_(),
                }),
            )
            .add_node(
                "approved",
                Box::new(TestNode {
                    name: "approved".to_string(),
                    updates: vec![StateUpdate::new("decision", json!("approved"))],
                }),
            )
            .add_node(
                "rejected",
                Box::new(TestNode {
                    name: "rejected".to_string(),
                    updates: vec![StateUpdate::new("decision", json!("rejected"))],
                }),
            )
            .add_edge(START, "router")
            .add_conditional_edges("router", routes)
            .add_edge("approved", END)
            .add_edge("rejected", END);

        let compiled = graph.compile().unwrap();
        let final_state = compiled.invoke(JsonState::new(), None).await.unwrap();
        assert_eq!(final_state.get_value("decision"), Some(json!("approved")));
    }

    #[tokio::test]
    async fn test_conditional_routing_legacy_key_matching_when_route_missing() {
        let mut graph = StateGraphImpl::<JsonState>::new("route_legacy");
        let mut routes = HashMap::new();
        routes.insert("approve".to_string(), "approved".to_string());
        routes.insert("reject".to_string(), "rejected".to_string());

        graph
            .add_node(
                "router",
                Box::new(StaticCommandNode {
                    name: "router".to_string(),
                    command: Command::new().update("reject", json!(true)).continue_(),
                }),
            )
            .add_node(
                "approved",
                Box::new(TestNode {
                    name: "approved".to_string(),
                    updates: vec![StateUpdate::new("decision", json!("approved"))],
                }),
            )
            .add_node(
                "rejected",
                Box::new(TestNode {
                    name: "rejected".to_string(),
                    updates: vec![StateUpdate::new("decision", json!("rejected"))],
                }),
            )
            .add_edge(START, "router")
            .add_conditional_edges("router", routes)
            .add_edge("approved", END)
            .add_edge("rejected", END);

        let compiled = graph.compile().unwrap();
        let final_state = compiled.invoke(JsonState::new(), None).await.unwrap();
        assert_eq!(final_state.get_value("decision"), Some(json!("rejected")));
    }

    #[tokio::test]
    async fn test_conditional_routing_route_value_has_priority_over_legacy_key_match() {
        let mut graph = StateGraphImpl::<JsonState>::new("route_priority");
        let mut routes = HashMap::new();
        routes.insert("approve".to_string(), "approved".to_string());
        routes.insert("reject".to_string(), "rejected".to_string());

        graph
            .add_node(
                "router",
                Box::new(StaticCommandNode {
                    name: "router".to_string(),
                    command: Command::new()
                        .route("approve")
                        .update("reject", json!(true))
                        .continue_(),
                }),
            )
            .add_node(
                "approved",
                Box::new(TestNode {
                    name: "approved".to_string(),
                    updates: vec![StateUpdate::new("decision", json!("approved"))],
                }),
            )
            .add_node(
                "rejected",
                Box::new(TestNode {
                    name: "rejected".to_string(),
                    updates: vec![StateUpdate::new("decision", json!("rejected"))],
                }),
            )
            .add_edge(START, "router")
            .add_conditional_edges("router", routes)
            .add_edge("approved", END)
            .add_edge("rejected", END);

        let compiled = graph.compile().unwrap();
        let final_state = compiled.invoke(JsonState::new(), None).await.unwrap();
        assert_eq!(final_state.get_value("decision"), Some(json!("approved")));
    }

    #[tokio::test]
    async fn test_conditional_routing_unknown_route_falls_back_to_legacy_key_match() {
        let mut graph = StateGraphImpl::<JsonState>::new("route_unknown_fallback");
        let mut routes = HashMap::new();
        routes.insert("approve".to_string(), "approved".to_string());
        routes.insert("reject".to_string(), "rejected".to_string());

        graph
            .add_node(
                "router",
                Box::new(StaticCommandNode {
                    name: "router".to_string(),
                    command: Command::new()
                        .route("unknown")
                        .update("reject", json!(true))
                        .continue_(),
                }),
            )
            .add_node(
                "approved",
                Box::new(TestNode {
                    name: "approved".to_string(),
                    updates: vec![StateUpdate::new("decision", json!("approved"))],
                }),
            )
            .add_node(
                "rejected",
                Box::new(TestNode {
                    name: "rejected".to_string(),
                    updates: vec![StateUpdate::new("decision", json!("rejected"))],
                }),
            )
            .add_edge(START, "router")
            .add_conditional_edges("router", routes)
            .add_edge("approved", END)
            .add_edge("rejected", END);

        let compiled = graph.compile().unwrap();
        let final_state = compiled.invoke(JsonState::new(), None).await.unwrap();
        assert_eq!(final_state.get_value("decision"), Some(json!("rejected")));
    }

    #[tokio::test]
    async fn test_send_injects_branch_state_and_preserves_duplicate_targets_in_invoke() {
        let mut graph = StateGraphImpl::<JsonState>::new("send_invoke");

        graph
            .add_reducer("tasks_seen", Box::new(crate::workflow::AppendReducer))
            .add_node(
                "mapper",
                Box::new(SendMapperNode {
                    sends: vec![
                        SendCommand::new("worker", json!({"task": 1})),
                        SendCommand::new("worker", json!({"task": 2})),
                    ],
                }),
            )
            .add_node("worker", Box::new(SendWorkerNode))
            .add_edge(START, "mapper")
            .add_edge("mapper", "worker");

        let compiled = graph.compile().unwrap();
        let final_state = compiled.invoke(JsonState::new(), None).await.unwrap();
        assert_eq!(final_state.get_value("tasks_seen"), Some(json!([1, 2])));
    }

    #[tokio::test]
    async fn test_send_injects_branch_state_and_preserves_duplicate_targets_in_stream() {
        let mut graph = StateGraphImpl::<JsonState>::new("send_stream");

        graph
            .add_reducer("tasks_seen", Box::new(crate::workflow::AppendReducer))
            .add_node(
                "mapper",
                Box::new(SendMapperNode {
                    sends: vec![
                        SendCommand::new("worker", json!({"task": 1})),
                        SendCommand::new("worker", json!({"task": 2})),
                    ],
                }),
            )
            .add_node("worker", Box::new(SendWorkerNode))
            .add_edge(START, "mapper")
            .add_edge("mapper", "worker");

        let compiled = graph.compile().unwrap();
        let mut stream = compiled.stream(JsonState::new(), None);
        let mut final_state: Option<JsonState> = None;

        while let Some(event) = stream.next().await {
            if let Ok(StreamEvent::End {
                final_state: emitted_state,
            }) = event
            {
                final_state = Some(emitted_state);
            }
        }

        let final_state = final_state.expect("stream should emit final state");
        assert_eq!(final_state.get_value("tasks_seen"), Some(json!([1, 2])));
    }

    #[tokio::test]
    async fn test_send_rejects_non_object_payload() {
        let mut graph = StateGraphImpl::<JsonState>::new("send_non_object_payload");

        graph
            .add_node(
                "mapper",
                Box::new(SendMapperNode {
                    sends: vec![SendCommand::new("worker", json!(42))],
                }),
            )
            .add_node("worker", Box::new(SendWorkerNode))
            .add_edge(START, "mapper")
            .add_edge("mapper", "worker");

        let compiled = graph.compile().unwrap();
        let err = compiled.invoke(JsonState::new(), None).await.unwrap_err();
        assert!(
            err.to_string()
                .contains("SendCommand::input must be a JSON object")
        );
    }

    #[tokio::test]
    async fn test_send_with_branch_id_exposes_branch_metadata_to_node_context() {
        let mut graph = StateGraphImpl::<JsonState>::new("send_branch_metadata");

        graph
            .add_reducer("branch_ids", Box::new(crate::workflow::AppendReducer))
            .add_reducer("send_sources", Box::new(crate::workflow::AppendReducer))
            .add_node(
                "mapper",
                Box::new(SendMapperNode {
                    sends: vec![
                        SendCommand::with_branch("worker", json!({"task": 1}), "branch-a"),
                        SendCommand::with_branch("worker", json!({"task": 2}), "branch-b"),
                    ],
                }),
            )
            .add_node("worker", Box::new(SendMetadataWorkerNode))
            .add_edge(START, "mapper")
            .add_edge("mapper", "worker");

        let compiled = graph.compile().unwrap();
        let final_state = compiled.invoke(JsonState::new(), None).await.unwrap();
        assert_eq!(
            final_state.get_value("branch_ids"),
            Some(json!(["branch-a", "branch-b"]))
        );
        assert_eq!(
            final_state.get_value("send_sources"),
            Some(json!(["mapper", "mapper"]))
        );
    }
}
