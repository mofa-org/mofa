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
use tracing::{debug, info, warn};

/// Type alias for node ID
pub type NodeId = String;

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
    /// Graph configuration
    config: GraphConfig,
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
            if reachable.insert(node_id.clone()) {
                if let Some(edge_target) = self.edges.get(&node_id) {
                    let targets = edge_target.targets();
                    for target in targets {
                        if target != END && !reachable.contains(target) {
                            stack.push(target.to_string());
                        }
                    }
                }
            }
        }

        reachable
    }

    /// Attach a fault-tolerance policy to a specific node.
    ///
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
        }

        self
    }

    fn add_conditional_edges(
        &mut self,
        from: impl Into<String>,
        conditions: Vec<(String, String)>,
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
            if let Some(ref fallback) = policy.fallback_node {
                if !self.nodes.contains_key(fallback) {
                    return Err(AgentError::ValidationFailed(format!(
                        "Fallback node '{}' for node '{}' does not exist in graph",
                        fallback, node_id
                    )));
                }
            }
        }

        let nodes_arc: HashMap<NodeId, Arc<dyn NodeFunc<S>>> = self
            .nodes
            .into_iter()
            .map(|(k, v)| (k, Arc::from(v)))
            .collect();

        let max_parallelism = self.config.max_parallelism.unwrap_or(10);
        let semaphore = Arc::new(tokio::sync::Semaphore::new(max_parallelism));

        // Create compiled graph
        Ok(CompiledGraphImpl {
            id: self.id,
            nodes: Arc::new(nodes_arc),
            edges: Arc::new(self.edges),
            reducers: Arc::new(self.reducers),
            entry_point: self.entry_point.expect("Entry point should be validated"),
            config: self.config,
            policies: Arc::new(self.policies),
            circuit_states: mofa_kernel::workflow::policies::CircuitBreakerRegistry::new(),
            parallelism_semaphore: semaphore,
            telemetry: None,
        })
    }
}

/// Compiled graph ready for execution
pub struct CompiledGraphImpl<S: GraphState> {
    /// Graph ID
    id: String,
    /// Node functions
    nodes: Arc<HashMap<NodeId, Arc<dyn NodeFunc<S>>>>,
    /// Edges
    edges: Arc<HashMap<NodeId, EdgeTarget>>,
    /// Reducers
    reducers: Arc<HashMap<String, Box<dyn Reducer>>>,
    /// Entry point
    entry_point: NodeId,
    /// Configuration
    config: GraphConfig,
    /// Per-node fault-tolerance policies
    policies: Arc<HashMap<NodeId, NodePolicy>>,
    /// Per-node circuit breaker state (shared across invocations)
    circuit_states: CircuitBreakerRegistry,
    /// Concurrency semaphore for parallel branches
    parallelism_semaphore: Arc<Semaphore>,
    /// Optional telemetry emitter for runtime checkpoint events.
    telemetry: Option<Arc<dyn TelemetryEmitter>>,
}

impl<S: GraphState> CompiledGraphImpl<S> {
    /// Attach a telemetry emitter for checkpoint events during invocation.
    pub fn with_telemetry(mut self, telemetry: Arc<dyn TelemetryEmitter>) -> Self {
        self.telemetry = Some(telemetry);
        self
    }

    fn build_node_context(base: &RuntimeContext, node_id: &str) -> RuntimeContext {
        RuntimeContext {
            execution_id: base.execution_id.clone(),
            graph_id: base.graph_id.clone(),
            current_node: Arc::new(RwLock::new(node_id.to_string())),
            remaining_steps: base.remaining_steps.clone(),
            config: base.config.clone(),
            metadata: base.metadata.clone(),
            parent_execution_id: base.parent_execution_id.clone(),
            tags: base.tags.clone(),
        }
    }

    /// Execute multiple nodes in parallel, enforcing max_parallelism via semaphore.
    ///
    /// NOTE: Parallel nodes run without per-node retry/circuit-breaker protection.
    /// Each node executes against an isolated state snapshot. This is by design:
    /// parallel fan-out patterns prioritize throughput over individual node
    /// resilience. If retry is needed, place a single-node step before/after
    /// the parallel fan-out.
    async fn execute_parallel_nodes(
        nodes: &HashMap<NodeId, Arc<dyn NodeFunc<S>>>,
        node_ids: &[String],
        base_state: &S,
        base_ctx: &RuntimeContext,
        semaphore: &Arc<Semaphore>,
    ) -> AgentResult<Vec<(String, Command)>> {
        let mut join_set = JoinSet::new();

        for (index, node_id) in node_ids.iter().enumerate() {
            let node = nodes
                .get(node_id)
                .ok_or_else(|| AgentError::NotFound(format!("Node '{}'", node_id)))?
                .clone();
            // Each parallel node runs against an isolated snapshot. Node-side mutations are
            // intentionally sandboxed; shared-state changes must be expressed via Command updates.
            let mut isolated_state = base_state.clone();
            let node_ctx = Self::build_node_context(base_ctx, node_id);
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

        let mut ordered_results: Vec<Option<(String, Command)>> = vec![None; node_ids.len()];
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

    /// Resolve conditional routing.
    ///
    /// Route selection priority:
    /// 1. `command.route` when non-empty and present in `routes`
    /// 2. Legacy fallback by matching `command.updates[*].key` to route labels
    /// 3. Error if no match (strict routing)
    ///
    /// An empty route string is treated as "no explicit route" and falls back to legacy behavior.
    fn resolve_conditional_next_nodes(
        current_node: &str,
        command: &Command,
        routes: &[(String, String)],
    ) -> AgentResult<Vec<String>> {
        debug!(
            "Routing from '{}': route={:?}, updates={:?}",
            current_node,
            command.route,
            command
                .updates
                .iter()
                .map(|u| u.key.as_str())
                .collect::<Vec<_>>()
        );

        if let Some(route_name) = command.route.as_deref() {
            if route_name.is_empty() {
                debug!(
                    "Empty conditional route from node '{}'; falling back to legacy update-key routing",
                    current_node
                );
            } else if let Some((_, target)) = routes.iter().find(|(route, _)| route == route_name) {
                return Ok(vec![target.clone()]);
            } else {
                warn!(
                    "No conditional edge found for route '{}' from node '{}'",
                    route_name, current_node
                );
            }
        }

        for update in &command.updates {
            if let Some((_, target)) = routes.iter().find(|(route, _)| route == &update.key) {
                debug!(
                    "Legacy conditional routing via update key '{}' from node '{}'",
                    update.key, current_node
                );
                if command
                    .route
                    .as_deref()
                    .map(|route| route.is_empty())
                    .unwrap_or(true)
                {
                    debug!(
                        "Legacy routing used for node '{}'. Consider using Command::route() for explicit routing.",
                        current_node
                    );
                }
                return Ok(vec![target.clone()]);
            }
        }

        // No route matched — report error instead of silent fallback
        let update_keys: Vec<&str> = command.updates.iter().map(|u| u.key.as_str()).collect();
        let route_keys: Vec<&String> = routes.iter().map(|(r, _)| r).collect();
        warn!(
            node_id = current_node,
            ?update_keys,
            ?route_keys,
            "Conditional routing: no route matched for node"
        );
        Err(AgentError::Internal(format!(
            "No conditional route matched for node '{}': update keys {:?}, available routes {:?}",
            current_node, update_keys, route_keys
        )))
    }

    /// Get the next node(s) based on the current node and command
    fn get_next_nodes(&self, current_node: &str, command: &Command) -> AgentResult<Vec<String>> {
        match &command.control {
            ControlFlow::Goto(target) => Ok(vec![target.clone()]),
            ControlFlow::Return => {
                Ok(vec![]) // End execution
            }
            ControlFlow::Send(sends) => {
                // MapReduce: create branches for each send target
                Ok(sends.iter().map(|s| s.target.clone()).collect())
            }
            ControlFlow::Continue => {
                // Follow graph edges
                match self.edges.get(current_node) {
                    Some(EdgeTarget::Single(target)) => Ok(vec![target.clone()]),
                    Some(EdgeTarget::Parallel(targets)) => Ok(targets.clone()),
                    Some(EdgeTarget::Conditional(routes)) => {
                        Self::resolve_conditional_next_nodes(current_node, command, routes)
                    }
                    None => Ok(vec![]),
                    _ => Ok(vec![]),
                }
            }
            _ => Ok(vec![]),
        }
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
impl<S: GraphState + 'static> CompiledGraph<S> for CompiledGraphImpl<S> {
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
        let mut current_nodes = vec![self.entry_point.clone()];

        while !current_nodes.is_empty() {
            // Check recursion limit
            if ctx.is_recursion_limit_reached().await {
                return Err(AgentError::Internal("Recursion limit reached".to_string()));
            }
            ctx.decrement_steps().await;

            // Execute nodes
            if current_nodes.len() == 1 {
                // Single node execution
                let node_id = current_nodes.remove(0);
                let node = self
                    .nodes
                    .get(&node_id)
                    .ok_or_else(|| AgentError::NotFound(format!("Node '{}'", node_id)))?;

                ctx.set_current_node(&node_id).await;
                debug!("Executing node '{}' in graph '{}'", node_id, self.id);

                let command = node.call(&mut state, &ctx).await?;

                // Apply updates
                self.apply_updates(&mut state, &command.updates).await?;

                // Get next nodes
                current_nodes = self.get_next_nodes(&node_id, &command)?;

                debug!(
                    "Node '{}' completed, next nodes: {:?}",
                    node_id, current_nodes
                );
            } else {
                // Parallel execution
                let mut next_nodes = Vec::new();
                let nodes_to_execute = std::mem::take(&mut current_nodes);

                for node_id in nodes_to_execute {
                    let node = self
                        .nodes
                        .get(&node_id)
                        .ok_or_else(|| AgentError::NotFound(format!("Node '{}'", node_id)))?;

                    ctx.set_current_node(&node_id).await;
                    debug!("Executing node '{}' (parallel)", node_id);

                    let command = node.call(&mut state, &ctx).await?;

                    // Apply updates
                    self.apply_updates(&mut state, &command.updates).await?;

                    // Collect next nodes
                    let next = self.get_next_nodes(&node_id, &command)?;
                    next_nodes.extend(next);
                }

                // Deduplicate next nodes
                let next_set: HashSet<String> = next_nodes.into_iter().collect();
                current_nodes = next_set.into_iter().collect();
            }
        }

        info!("Graph '{}' execution completed", self.id);
        Ok(state)
    }

    async fn stream(
        &self,
        input: S,
        config: Option<RuntimeContext>,
    ) -> AgentResult<Pin<Box<dyn Stream<Item = AgentResult<StreamEvent<S>>> + Send>>> {
        let ctx =
            config.unwrap_or_else(|| RuntimeContext::with_config(&self.id, self.config.clone()));

        let nodes = self.nodes.clone();
        let reducers = self.reducers.clone();
        let entry_point = self.entry_point.clone();

        // Create a channel for streaming events
        let (tx, rx) = tokio::sync::mpsc::channel(100);

        // Spawn execution task
        tokio::spawn(async move {
            let mut state = input;
            let mut current_nodes = vec![entry_point];
            let default_policy = NodePolicy::default();

            // Helper function to get next nodes based on command and edges
            let get_next_nodes = |current_node: &str, command: &Command| -> AgentResult<Vec<String>> {
                match &command.control {
                    ControlFlow::Goto(target) => Ok(vec![target.clone()]),
                    ControlFlow::Return => Ok(vec![]), // End execution
                    ControlFlow::Send(sends) => {
                        // MapReduce: create branches for each send target
                        Ok(sends.iter().map(|s| s.target.clone()).collect())
                    }
                    ControlFlow::Continue => {
                        // Follow graph edges
                        match edges.get(current_node) {
                            Some(EdgeTarget::Single(target)) => Ok(vec![target.clone()]),
                            Some(EdgeTarget::Parallel(targets)) => Ok(targets.clone()),
                            Some(EdgeTarget::Conditional(routes)) => {
                                Self::resolve_conditional_next_nodes(current_node, command, routes)
                            }
                            None => Ok(vec![]),
                            _ => Ok(vec![]),
                        }
                    }
                    _ => Ok(vec![]),
                }
            };

            while !current_nodes.is_empty() {
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

                for node_id in nodes_to_execute {
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
                        Err(e) => {
                            let _ = tx
                                .send(Ok(StreamEvent::Error {
                                    node_id: Some(node_id),
                                    error: e.to_string(),
                                }))
                                .await;
                            return;
                        }
                    };

                    // Apply updates
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
                    if tx
                        .send(Ok(StreamEvent::NodeEnd {
                            node_id: node_id.clone(),
                            state: state.clone(),
                            command: command.clone(),
                        }))
                        .await
                        .is_err()
                    {
                        warn!(node_id, "Stream receiver dropped after node end; aborting graph execution");
                        return;
                    }

                    match get_next_nodes(&node_id, &command) {
                        Ok(nodes) => next_nodes.extend(nodes),
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
                    // Send start events for parallel batch — abort if receiver disconnected
                    for node_id in &nodes_to_execute {
                        if tx
                            .send(Ok(StreamEvent::NodeStart {
                                node_id: node_id.clone(),
                                state: state.clone(),
                            }))
                            .await
                            .is_err()
                        {
                            warn!(node_id, "Stream receiver dropped during parallel start; aborting graph execution");
                            return;
                        }
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
                                .send(Err(e))
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

                        if tx
                            .send(Ok(StreamEvent::NodeEnd {
                                node_id: node_id.clone(),
                                state: state.clone(),
                                command: command.clone(),
                            }))
                            .await
                            .is_err()
                        {
                            warn!(node_id, "Stream receiver dropped during parallel end; aborting graph execution");
                            return;
                        }

                        match get_next_nodes(&node_id, &command) {
                            Ok(nodes) => next_nodes.extend(nodes),
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
                    }
                }

                // For simplicity, break after first round
                // TODO: Implement proper edge following
                break;
            }

            // Send final event
            let _ = tx.send(Ok(StreamEvent::End { final_state: state })).await;
        });

        // Convert receiver to stream
        Ok(Box::pin(tokio_stream::wrappers::ReceiverStream::new(rx)))
    }

    async fn step(&self, input: S, config: Option<RuntimeContext>) -> AgentResult<StepResult<S>> {
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
    use mofa_kernel::workflow::telemetry::{DebugEvent, TelemetryEmitter};
    use mofa_kernel::workflow::GraphConfig;
    use mofa_kernel::workflow::{JsonState, StateGraph};
    use serde_json::{json, Value};

    // Simple test node
    struct TestNode {
        name: String,
        updates: Vec<StateUpdate>,
    }

    #[async_trait]
    impl NodeFunc<JsonState, Value> for TestNode {
        async fn call(
            &self,
            _state: &mut JsonState,
            _ctx: &RuntimeContext<Value>,
        ) -> AgentResult<Command<Value>> {
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

    /// Test node that returns a fixed Command (used for routing tests)
    struct StaticCommandNode {
        name: String,
        command: Command,
    }

    #[async_trait]
    impl NodeFunc<JsonState, Value> for StaticCommandNode {
        async fn call(
            &self,
            _state: &mut JsonState,
            _ctx: &RuntimeContext<Value>,
        ) -> AgentResult<Command<Value>> {
            Ok(self.command.clone())
        }

        fn name(&self) -> &str {
            &self.name
        }
    }

    struct ConcurrencyProbeNode {
        name: String,
        active: Arc<AtomicUsize>,
        max_active: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl NodeFunc<JsonState, Value> for ConcurrencyProbeNode {
        async fn call(
            &self,
            _state: &mut JsonState,
            _ctx: &RuntimeContext<Value>,
        ) -> AgentResult<Command<Value>> {
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
    impl NodeFunc<JsonState, Value> for FlagReaderNode {
        async fn call(
            &self,
            state: &mut JsonState,
            _ctx: &RuntimeContext<Value>,
        ) -> AgentResult<Command<Value>> {
            let saw_flag = state.get_value::<bool>("flag").unwrap_or(false);
            Ok(Command::new()
                .update("reader_saw_flag", json!(saw_flag))
                .continue_())
        }

        fn name(&self) -> &str {
            "flag_reader"
        }
    }

    struct LoopNode;

    #[async_trait]
    impl NodeFunc<JsonState, Value> for LoopNode {
        async fn call(
            &self,
            state: &mut JsonState,
            _ctx: &RuntimeContext<Value>,
        ) -> AgentResult<Command<Value>> {
            let count = state
                .get_value::<serde_json::Value>("count")
                .and_then(|v| v.as_u64())
                .unwrap_or(0)
                + 1;
            let command = Command::new().update("count", json!(count));
            if count >= 12 {
                Ok(command.return_())
            } else {
                Ok(command.goto("loop"))
            }
        }

        fn name(&self) -> &str {
            "loop"
        }
    }

    #[derive(Default)]
    struct CollectingEmitter {
        events: Arc<Mutex<Vec<DebugEvent>>>,
    }

    #[async_trait]
    impl TelemetryEmitter for CollectingEmitter {
        async fn emit(&self, event: DebugEvent) {
            self.events.lock().await.push(event);
        }
    }

    struct DisabledEmitter;

    #[async_trait]
    impl TelemetryEmitter for DisabledEmitter {
        async fn emit(&self, _event: DebugEvent) {}
        fn is_enabled(&self) -> bool {
            false
        }
    }

    struct RouteNode {
        name: String,
        updates: Vec<StateUpdate>,
        route: String,
    }

    #[async_trait]
    impl NodeFunc<JsonState, Value> for RouteNode {
        async fn call(
            &self,
            _state: &mut JsonState,
            _ctx: &RuntimeContext<Value>,
        ) -> AgentResult<Command<Value>> {
            let mut cmd = Command::new();
            for update in &self.updates {
                cmd = cmd.update(update.key.clone(), update.value.clone());
            }
            Ok(cmd.route(self.route.clone()).continue_())
        }

        fn name(&self) -> &str {
            &self.name
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

    // ── Conditional Routing Tests ──

    #[tokio::test]
    async fn test_conditional_routing_uses_route_value_not_update_key() {
        let mut graph = StateGraphImpl::<JsonState>::new("route_graph");

        graph
            .add_node(
                "decide",
                Box::new(RouteNode {
                    name: "decide".to_string(),
                    // This update key collides with a route label and should NOT control routing.
                    updates: vec![StateUpdate::new("approve", json!(true))],
                    route: "reject".to_string(),
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
            .add_edge(START, "decide")
            .add_conditional_edges(
                "decide",
                vec![
                    ("approve".to_string(), "approved".to_string()),
                    ("reject".to_string(), "rejected".to_string()),
                ],
            )
            .add_edge("approved", END)
            .add_edge("rejected", END);

        let compiled = graph.compile().unwrap();
        let final_state = compiled.invoke(JsonState::new(), None).await.unwrap();

        assert_eq!(final_state.get_value("decision"), Some(json!("rejected")));
    }

    #[tokio::test]
    async fn test_conditional_routing_legacy_update_key_fallback() {
        let mut graph = StateGraphImpl::<JsonState>::new("legacy_route_graph");

        graph
            .add_node(
                "decide",
                Box::new(TestNode {
                    name: "decide".to_string(),
                    // No explicit command.route; legacy update key fallback should select this route.
                    updates: vec![StateUpdate::new("approve", json!(true))],
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
            .add_edge(START, "decide")
            .add_conditional_edges(
                "decide",
                vec![
                    ("approve".to_string(), "approved".to_string()),
                    ("reject".to_string(), "rejected".to_string()),
                ],
            )
            .add_edge("approved", END)
            .add_edge("rejected", END);

        let compiled = graph.compile().unwrap();
        let final_state = compiled.invoke(JsonState::new(), None).await.unwrap();

        assert_eq!(final_state.get_value("decision"), Some(json!("approved")));

        let mut stream = compiled.stream(JsonState::new(), None);
        let mut stream_final_state: Option<JsonState> = None;
        while let Some(event) = stream.next().await {
            if let Ok(StreamEvent::End { final_state }) = event {
                stream_final_state = Some(final_state);
            }
        }

        let stream_final_state: JsonState =
            stream_final_state.expect("stream should emit a final end event with state");
        assert_eq!(
            stream_final_state.get_value("decision"),
            Some(json!("approved"))
        );
    }

    #[tokio::test]
    async fn test_conditional_routing_empty_route_falls_back_to_legacy_update_key() {
        let mut graph = StateGraphImpl::<JsonState>::new("empty_route_graph");

        graph
            .add_node(
                "decide",
                Box::new(RouteNode {
                    name: "decide".to_string(),
                    // Empty route should not match any conditional edge; fallback should still work.
                    updates: vec![StateUpdate::new("approve", json!(true))],
                    route: "".to_string(),
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
            .add_edge(START, "decide")
            .add_conditional_edges(
                "decide",
                vec![
                    ("approve".to_string(), "approved".to_string()),
                    ("reject".to_string(), "rejected".to_string()),
                ],
            )
            .add_edge("approved", END)
            .add_edge("rejected", END);

        let compiled = graph.compile().unwrap();
        let final_state = compiled.invoke(JsonState::new(), None).await.unwrap();

        assert_eq!(final_state.get_value("decision"), Some(json!("approved")));
    }

    #[tokio::test]
    async fn test_conditional_routing_prefers_route_value_in_invoke() {
        let mut graph = StateGraphImpl::<JsonState>::new("route_invoke");

        let routes = vec![
            ("approve".to_string(), "approved".to_string()),
            ("reject".to_string(), "rejected".to_string()),
        ];

        graph
            .add_node(
                "router",
                Box::new(StaticCommandNode {
                    name: "router".to_string(),
                    // route says "approve", but a key-name update matches "reject".
                    // The explicit route must win.
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

        assert_eq!(
            final_state.get_value::<serde_json::Value>("decision"),
            Some(json!("approved"))
        );
    }

    #[tokio::test]
    async fn test_conditional_routing_legacy_fallback_when_route_absent() {
        let mut graph = StateGraphImpl::<JsonState>::new("route_fallback");

        let routes = vec![
            ("approve".to_string(), "approved".to_string()),
            ("reject".to_string(), "rejected".to_string()),
        ];

        graph
            .add_node(
                "router",
                Box::new(StaticCommandNode {
                    name: "router".to_string(),
                    // No explicit route — should fall back to legacy key matching.
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

        assert_eq!(
            final_state.get_value::<serde_json::Value>("decision"),
            Some(json!("rejected"))
        );
    }

    #[tokio::test]
    async fn test_conditional_routing_stream_respects_route_value() {
        let mut graph = StateGraphImpl::<JsonState>::new("route_stream");

        let routes = vec![
            ("approve".to_string(), "approved".to_string()),
            ("reject".to_string(), "rejected".to_string()),
        ];

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
        let mut stream = compiled.stream(JsonState::new(), None);

        let mut final_state = None;
        while let Some(event) = stream.next().await {
            if let Ok(StreamEvent::End { final_state: state }) = event {
                final_state = Some(state);
            }
        }

        let final_state = final_state.expect("stream should produce final state");
        assert_eq!(
            final_state.get_value::<serde_json::Value>("decision"),
            Some(json!("approved"))
        );
    }

    #[tokio::test]
    async fn test_conditional_routing_no_match_returns_error_invoke() {
        let mut graph = StateGraphImpl::<JsonState>::new("route_no_match");

        let routes = vec![
            ("approve".to_string(), "approved".to_string()),
            ("reject".to_string(), "rejected".to_string()),
        ];

        graph
            .add_node(
                "router",
                Box::new(StaticCommandNode {
                    name: "router".to_string(),
                    // No route value set, and update key "unknown" matches no route
                    command: Command::new().update("unknown", json!(true)).continue_(),
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
        let result = compiled.invoke(JsonState::new(), None).await;

        assert!(
            result.is_err(),
            "invoke should return Err when no conditional route matches"
        );
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("No conditional route matched"),
            "error should mention no route matched, got: {}",
            err_msg
        );
    }

    #[tokio::test]
    async fn test_conditional_routing_no_match_returns_error_stream() {
        let mut graph = StateGraphImpl::<JsonState>::new("route_no_match_stream");

        let routes = vec![
            ("approve".to_string(), "approved".to_string()),
            ("reject".to_string(), "rejected".to_string()),
        ];

        graph
            .add_node(
                "router",
                Box::new(StaticCommandNode {
                    name: "router".to_string(),
                    command: Command::new().update("unknown", json!(true)).continue_(),
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
        let mut stream = compiled.stream(JsonState::new(), None);

        let mut got_error = false;
        while let Some(event) = stream.next().await {
            if let Ok(StreamEvent::Error { error, .. }) = event {
                assert!(
                    error.contains("No conditional route matched"),
                    "stream error should mention no route matched, got: {}",
                    error
                );
                got_error = true;
            }
        }

        assert!(
            got_error,
            "stream should emit an error event when no conditional route matches"
        );
    }
}
