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
use tokio::sync::RwLock;
use tokio::task::JoinSet;
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

        // Create compiled graph
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
}

impl<S: GraphState> CompiledGraphImpl<S> {
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

    async fn execute_parallel_nodes(
        nodes: &HashMap<NodeId, Arc<dyn NodeFunc<S>>>,
        node_ids: &[String],
        base_state: &S,
        base_ctx: &RuntimeContext,
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

            join_set.spawn(async move {
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

    /// Get the next node(s) based on the current node and command
    fn get_next_nodes(&self, current_node: &str, command: &Command) -> Vec<String> {
        match &command.control {
            ControlFlow::Goto(target) => {
                vec![target.clone()]
            }
            ControlFlow::Return => {
                vec![] // End execution
            }
            ControlFlow::Send(sends) => {
                // MapReduce: create branches for each send target
                sends.iter().map(|s| s.target.clone()).collect()
            }
            ControlFlow::Continue => {
                // Follow graph edges
                match self.edges.get(current_node) {
                    Some(EdgeTarget::Single(target)) => vec![target.clone()],
                    Some(EdgeTarget::Parallel(targets)) => targets.clone(),
                    Some(EdgeTarget::Conditional(routes)) => {
                        // Find matching route based on state updates
                        for update in &command.updates {
                            if let Some(target) = routes.get(&update.key) {
                                return vec![target.clone()];
                            }
                        }
                        // Default to first route if no match
                        routes
                            .values()
                            .next()
                            .map(|t: &String| vec![t.clone()])
                            .unwrap_or_default()
                    }
                    None => vec![],
                    _ => vec![],
                }
            }
            _ => vec![],
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
                current_nodes = self.get_next_nodes(&node_id, &command);

                debug!(
                    "Node '{}' completed, next nodes: {:?}",
                    node_id, current_nodes
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
                )
                .await?;

                for (node_id, command) in parallel_results {
                    debug!("Applying updates from parallel node '{}'", node_id);

                    // Apply updates only after all parallel nodes have completed.
                    self.apply_updates(&mut state, &command.updates).await?;

                    // Collect next nodes
                    let next = self.get_next_nodes(&node_id, &command);
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

        // Create a channel for streaming events
        let (tx, rx) = tokio::sync::mpsc::channel(100);

        // Spawn execution task
        tokio::spawn(async move {
            let mut state = input;
            let mut current_nodes = vec![entry_point];
            let mut iteration_count = 0;
            const MAX_ITERATIONS: usize = 20;

            // Helper function to get next nodes based on command and edges
            let get_next_nodes = |current_node: &str, command: &Command| -> Vec<String> {
                match &command.control {
                    ControlFlow::Goto(target) => vec![target.clone()],
                    ControlFlow::Return => vec![], // End execution
                    ControlFlow::Send(sends) => {
                        // MapReduce: create branches for each send target
                        sends.iter().map(|s| s.target.clone()).collect()
                    }
                    ControlFlow::Continue => {
                        // Follow graph edges
                        match edges.get(current_node) {
                            Some(EdgeTarget::Single(target)) => vec![target.clone()],
                            Some(EdgeTarget::Parallel(targets)) => targets.clone(),
                            Some(EdgeTarget::Conditional(routes)) => {
                                // Find matching route based on state updates
                                for update in &command.updates {
                                    if let Some(target) = routes.get(&update.key) {
                                        return vec![target.clone()];
                                    }
                                }
                                // Default to first route if no match
                                routes
                                    .values()
                                    .next()
                                    .map(|t: &String| vec![t.clone()])
                                    .unwrap_or_default()
                            }
                            None => vec![],
                            _ => vec![],
                        }
                    }
                    _ => vec![],
                }
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

                if nodes_to_execute.len() == 1 {
                    let node_id = nodes_to_execute[0].clone();
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

                    // Execute node
                    let command = match node.call(&mut state, &ctx).await {
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

                    next_nodes.extend(get_next_nodes(&node_id, &command));
                } else {
                    for node_id in &nodes_to_execute {
                        let _ = tx
                            .send(Ok(StreamEvent::NodeStart {
                                node_id: node_id.clone(),
                                state: state.clone(),
                            }))
                            .await;
                    }

                    let commands = match Self::execute_parallel_nodes(
                        nodes.as_ref(),
                        &nodes_to_execute,
                        &state,
                        &ctx,
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

                        next_nodes.extend(get_next_nodes(&node_id, &command));
                    }
                }

                // Deduplicate current_nodes for parallel execution
                let node_set: HashSet<String> = next_nodes.into_iter().collect();
                current_nodes = node_set.into_iter().collect();
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
    use mofa_kernel::workflow::{JsonState, StateGraph};
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
            let saw_flag = state
                .get_value("flag")
                .and_then(|v: Value| v.as_bool())
                .unwrap_or(false);
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
        while let Some(event) = StreamExt::next(&mut stream).await {
            let ev: StreamEvent<JsonState> = event.unwrap();
            if let StreamEvent::End { final_state } = ev {
        let mut stream_final_state = None;
        while let Some(event) = stream.next().await {
            if let StreamEvent::End { final_state } = event.unwrap() {
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
}
