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
            nodes: Arc::new(self.nodes),
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
    nodes: Arc<HashMap<NodeId, Box<dyn NodeFunc<S>>>>,
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
                }
            }
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
                current_nodes = self.get_next_nodes(&node_id, &command);

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

    async fn stream(
        &self,
        input: S,
        config: Option<RuntimeContext>,
    ) -> AgentResult<Pin<Box<dyn Stream<Item = AgentResult<StreamEvent<S>>> + Send>>> {
        let ctx =
            config.unwrap_or_else(|| RuntimeContext::with_config(&self.id, self.config.clone()));

        let nodes = self.nodes.clone();
        let edges = self.edges.clone();
        let reducers = self.reducers.clone();
        let entry_point = self.entry_point.clone();

        // Create a channel for streaming events
        let (tx, rx) = tokio::sync::mpsc::channel(100);

        // Spawn execution task
        tokio::spawn(async move {
            let mut state = input;
            let mut current_nodes = vec![entry_point];

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
                let mut next_nodes_collector = Vec::new();

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
                    let _ = tx
                        .send(Ok(StreamEvent::NodeEnd {
                            node_id: node_id.clone(),
                            state: state.clone(),
                            command: command.clone(),
                        }))
                        .await;

                    // Follow edges to determine next nodes (mirrors invoke() logic)
                    let next = match &command.control {
                        ControlFlow::Goto(target) => vec![target.clone()],
                        ControlFlow::Return => vec![],
                        ControlFlow::Send(sends) => {
                            sends.iter().map(|s| s.target.clone()).collect()
                        }
                        ControlFlow::Continue => {
                            match edges.get(&node_id) {
                                Some(EdgeTarget::Single(target)) => vec![target.clone()],
                                Some(EdgeTarget::Parallel(targets)) => targets.clone(),
                                Some(EdgeTarget::Conditional(routes)) => {
                                    let mut matched = None;
                                    for update in &command.updates {
                                        if let Some(target) = routes.get(&update.key) {
                                            matched = Some(vec![target.clone()]);
                                            break;
                                        }
                                    }
                                    matched.unwrap_or_else(|| {
                                        routes.values().next()
                                            .map(|t| vec![t.clone()])
                                            .unwrap_or_default()
                                    })
                                }
                                None => vec![],
                            }
                        }
                    };
                    next_nodes_collector.extend(next);
                }

                // Deduplicate and filter END nodes
                let next_set: HashSet<String> = next_nodes_collector.into_iter()
                    .filter(|n| n != END)
                    .collect();
                current_nodes = next_set.into_iter().collect();
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
    use mofa_kernel::workflow::{JsonState, StateGraph};
    use serde_json::json;

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
}
