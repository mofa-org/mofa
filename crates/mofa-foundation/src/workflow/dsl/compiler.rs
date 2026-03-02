//! DSL → StateGraph Compiler
//!
//! Bridges the declarative DSL workflow definitions to the StateGraph execution engine.
//! Transforms a parsed `WorkflowDefinition` into a `CompiledGraphImpl<JsonState>` that
//! can be invoked, streamed, or stepped through.
//!
//! # Example
//!
//! ```rust,ignore
//! use mofa_foundation::workflow::dsl::{WorkflowDslParser, DslCompiler};
//!
//! let yaml = std::fs::read_to_string("workflow.yaml")?;
//! let def = WorkflowDslParser::from_yaml(&yaml)?;
//! let compiled = DslCompiler::compile(def)?;
//! let result = compiled.invoke(JsonState::default(), None).await?;
//! ```

use std::collections::HashMap;

use async_trait::async_trait;
use mofa_kernel::agent::error::AgentResult;
use mofa_kernel::workflow::{
    Command, END, GraphState, JsonState, NodeFunc, RuntimeContext, START, StateGraph,
};
use serde_json::Value;

use super::schema::*;
use super::{DslError, DslResult};
use crate::llm::LLMAgent;
use crate::workflow::state_graph::{CompiledGraphImpl, StateGraphImpl};
use std::sync::Arc;

// =============================================================================
// Node Adapters — DSL NodeDefinition → Box<dyn NodeFunc<JsonState>>
// =============================================================================

/// A pass-through node that forwards state unchanged.
///
/// Used for Start, End, and Parallel placeholder nodes.
struct PassthroughNode {
    node_name: String,
}

impl PassthroughNode {
    fn new(name: impl Into<String>) -> Self {
        Self {
            node_name: name.into(),
        }
    }
}

#[async_trait]
impl NodeFunc<JsonState> for PassthroughNode {
    async fn call(&self, _state: &mut JsonState, _ctx: &RuntimeContext) -> AgentResult<Command> {
        Ok(Command::new().continue_())
    }

    fn name(&self) -> &str {
        &self.node_name
    }

    fn description(&self) -> Option<&str> {
        Some("Pass-through node (no-op)")
    }
}

/// A task node that executes a no-op (placeholder for future executor support).
///
/// Currently only supports `TaskExecutorDef::None`. Other executor types
/// (Function, Http, Script) can be added in future PRs.
struct DslTaskNode {
    node_name: String,
    executor: TaskExecutorDef,
}

impl DslTaskNode {
    fn new(name: impl Into<String>, executor: TaskExecutorDef) -> Self {
        Self {
            node_name: name.into(),
            executor,
        }
    }
}

#[async_trait]
impl NodeFunc<JsonState> for DslTaskNode {
    async fn call(&self, _state: &mut JsonState, _ctx: &RuntimeContext) -> AgentResult<Command> {
        match &self.executor {
            TaskExecutorDef::None => {
                // No-op: just continue to next node
                Ok(Command::new().continue_())
            }
            TaskExecutorDef::Function { function } => {
                // Store the function name in state for downstream consumers
                Ok(Command::new()
                    .update(
                        "last_task",
                        serde_json::json!({
                            "type": "function",
                            "function": function,
                            "status": "placeholder"
                        }),
                    )
                    .continue_())
            }
            TaskExecutorDef::Http { url, method } => {
                // Store HTTP config in state for downstream consumers
                Ok(Command::new()
                    .update(
                        "last_task",
                        serde_json::json!({
                            "type": "http",
                            "url": url,
                            "method": method.as_deref().unwrap_or("GET"),
                            "status": "placeholder"
                        }),
                    )
                    .continue_())
            }
            TaskExecutorDef::Script { script } => {
                // Store script info in state
                Ok(Command::new()
                    .update(
                        "last_task",
                        serde_json::json!({
                            "type": "script",
                            "script_length": script.len(),
                            "status": "placeholder"
                        }),
                    )
                    .continue_())
            }
        }
    }

    fn name(&self) -> &str {
        &self.node_name
    }

    fn description(&self) -> Option<&str> {
        Some("DSL task node")
    }
}

/// A condition node that evaluates a condition and sets the route in state.
///
/// The condition result is stored in state under the node's ID key, which can
/// then be used by conditional edges to route to the appropriate next node.
struct DslConditionNode {
    node_name: String,
    condition: ConditionDef,
}

impl DslConditionNode {
    fn new(name: impl Into<String>, condition: ConditionDef) -> Self {
        Self {
            node_name: name.into(),
            condition,
        }
    }
}

#[async_trait]
impl NodeFunc<JsonState> for DslConditionNode {
    async fn call(&self, state: &mut JsonState, _ctx: &RuntimeContext) -> AgentResult<Command> {
        let result = match &self.condition {
            ConditionDef::Expression { expr } => {
                // For expression conditions, evaluate against state
                // Currently returns the expression as the route key for conditional edges.
                // A full expression evaluator (e.g. Rhai) can be integrated in a future PR.
                serde_json::json!(expr)
            }
            ConditionDef::Value {
                field,
                operator,
                value,
            } => {
                // Evaluate a simple field comparison
                let field_value: Option<Value> = state.get_value(field);
                let matches = match field_value {
                    Some(actual) => evaluate_condition(&actual, operator, value),
                    None => false,
                };
                serde_json::json!(if matches { "true" } else { "false" })
            }
        };

        Ok(Command::new().update(&self.node_name, result).continue_())
    }

    fn name(&self) -> &str {
        &self.node_name
    }

    fn description(&self) -> Option<&str> {
        Some("DSL condition node")
    }
}

/// A join node that waits for specified upstream nodes.
///
/// In the StateGraph model, join semantics are handled by the graph's edge
/// topology. This node stores the join metadata in state for observability.
struct DslJoinNode {
    node_name: String,
    wait_for: Vec<String>,
}

impl DslJoinNode {
    fn new(name: impl Into<String>, wait_for: Vec<String>) -> Self {
        Self {
            node_name: name.into(),
            wait_for,
        }
    }
}

#[async_trait]
impl NodeFunc<JsonState> for DslJoinNode {
    async fn call(&self, _state: &mut JsonState, _ctx: &RuntimeContext) -> AgentResult<Command> {
        Ok(Command::new()
            .update(
                &self.node_name,
                serde_json::json!({
                    "type": "join",
                    "waited_for": self.wait_for,
                }),
            )
            .continue_())
    }

    fn name(&self) -> &str {
        &self.node_name
    }

    fn description(&self) -> Option<&str> {
        Some("DSL join node")
    }
}

/// A transform node that records the transform definition in state.
struct DslTransformNode {
    node_name: String,
    transform: TransformDef,
}

impl DslTransformNode {
    fn new(name: impl Into<String>, transform: TransformDef) -> Self {
        Self {
            node_name: name.into(),
            transform,
        }
    }
}

#[async_trait]
impl NodeFunc<JsonState> for DslTransformNode {
    async fn call(&self, _state: &mut JsonState, _ctx: &RuntimeContext) -> AgentResult<Command> {
        let info = match &self.transform {
            TransformDef::Template { template } => serde_json::json!({
                "type": "template",
                "template": template,
            }),
            TransformDef::Expression { expr } => serde_json::json!({
                "type": "expression",
                "expr": expr,
            }),
            TransformDef::MapReduce { map, reduce } => serde_json::json!({
                "type": "map_reduce",
                "map": map,
                "reduce": reduce,
            }),
        };

        Ok(Command::new().update(&self.node_name, info).continue_())
    }

    fn name(&self) -> &str {
        &self.node_name
    }

    fn description(&self) -> Option<&str> {
        Some("DSL transform node")
    }
}

/// An agent node that routes execution to an LLM.
///
/// Sends the current `input` from state to the agent and stores the response.
struct DslAgentNode {
    node_name: String,
    agent: Arc<LLMAgent>,
    _config: AgentRef, // Kept for future extension (e.g. system prompt overrides)
}

impl DslAgentNode {
    fn new(name: impl Into<String>, agent: Arc<LLMAgent>, config: AgentRef) -> Self {
        Self {
            node_name: name.into(),
            agent,
            _config: config,
        }
    }
}

#[async_trait]
impl NodeFunc<JsonState> for DslAgentNode {
    async fn call(&self, state: &mut JsonState, _ctx: &RuntimeContext) -> AgentResult<Command> {
        // Find input argument depending on upstream context
        // Try `input` first, then default to dumping state if empty
        let input_text = if let Some(input) = state.get_value::<Value>("input") {
            if let Some(s) = input.as_str() {
                s.to_string()
            } else {
                input.to_string()
            }
        } else {
            serde_json::to_string(state).unwrap_or_default()
        };

        // Use simple Q&A ask() for stateless workflow processing
        let response = self.agent.ask(input_text).await.map_err(|e| {
            mofa_kernel::agent::error::AgentError::ExecutionFailed(format!(
                "LLM Agent failed: {}",
                e
            ))
        })?;

        // Store result in state using the node's ID as the key
        Ok(Command::new()
            .update(&self.node_name, serde_json::json!(response))
            .continue_())
    }

    fn name(&self) -> &str {
        &self.node_name
    }

    fn description(&self) -> Option<&str> {
        Some("DSL LLM Agent node")
    }
}

// =============================================================================
// Condition Evaluation Helper
// =============================================================================

/// Evaluate a simple condition: `actual <operator> expected`
fn evaluate_condition(actual: &Value, operator: &str, expected: &Value) -> bool {
    match operator {
        "==" | "eq" => actual == expected,
        "!=" | "ne" => actual != expected,
        ">" | "gt" => compare_values(actual, expected) == Some(std::cmp::Ordering::Greater),
        "<" | "lt" => compare_values(actual, expected) == Some(std::cmp::Ordering::Less),
        ">=" | "gte" => {
            matches!(
                compare_values(actual, expected),
                Some(std::cmp::Ordering::Greater | std::cmp::Ordering::Equal)
            )
        }
        "<=" | "lte" => {
            matches!(
                compare_values(actual, expected),
                Some(std::cmp::Ordering::Less | std::cmp::Ordering::Equal)
            )
        }
        "contains" => {
            if let (Some(haystack), Some(needle)) = (actual.as_str(), expected.as_str()) {
                haystack.contains(needle)
            } else {
                false
            }
        }
        _ => false,
    }
}

/// Compare two JSON values numerically
fn compare_values(a: &Value, b: &Value) -> Option<std::cmp::Ordering> {
    match (a.as_f64(), b.as_f64()) {
        (Some(a_num), Some(b_num)) => a_num.partial_cmp(&b_num),
        _ => {
            // Fall back to string comparison
            match (a.as_str(), b.as_str()) {
                (Some(a_str), Some(b_str)) => Some(a_str.cmp(b_str)),
                _ => None,
            }
        }
    }
}

// =============================================================================
// DSL Compiler
// =============================================================================

/// Compiles a parsed `WorkflowDefinition` into an executable `CompiledGraphImpl<JsonState>`.
///
/// This bridges the declarative DSL layer (YAML/TOML) to the StateGraph
/// execution engine, enabling users to define workflows in configuration
/// files and execute them via `invoke()`, `stream()`, or `step()`.
///
/// # Supported Node Types
///
/// | DSL Type       | Adapter            | Behavior                          |
/// |----------------|--------------------|-----------------------------------|
/// | `Start`        | `PassthroughNode`  | No-op entry point                 |
/// | `End`          | `PassthroughNode`  | No-op exit point                  |
/// | `Task`         | `DslTaskNode`      | Executor placeholder              |
/// | `Condition`    | `DslConditionNode` | Evaluates condition, sets route   |
/// | `Parallel`     | `PassthroughNode`  | Marker (parallelism via edges)    |
/// | `Join`         | `DslJoinNode`      | Records join metadata             |
/// | `Transform`    | `DslTransformNode` | Records transform definition      |
///
/// # Unsupported Node Types (Future PRs)
///
/// - `LlmAgent` — requires LLM provider registry
/// - `Loop` — requires runtime loop state machine
/// - `SubWorkflow` — requires recursive workflow loading
/// - `Wait` — requires external event system
pub struct DslCompiler;

impl DslCompiler {
    /// Compile a `WorkflowDefinition` into an executable `CompiledGraphImpl<JsonState>`.
    ///
    /// This performs validation, builds the graph, and compiles it in one step.
    /// Returns an error if any `LlmAgent` nodes are used, since no agent registry is provided.
    ///
    /// # Errors
    ///
    /// Returns `DslError` if:
    /// - The definition is invalid (no start/end node, dangling edges)
    /// - An `LlmAgent`, `Loop`, `SubWorkflow`, or `Wait` node type is used
    /// - Graph compilation fails (unreachable nodes, etc.)
    pub fn compile(def: WorkflowDefinition) -> DslResult<CompiledGraphImpl<JsonState>> {
        Self::compile_with_agents(def, &HashMap::new())
    }

    /// Compile a `WorkflowDefinition` with access to a registry of `LLMAgent` instances.
    ///
    /// This is required if the workflow definition contains `LlmAgent` nodes.
    ///
    /// # Errors
    ///
    /// Extends `compile()` errors, and will also return `DslError::Validation` if
    /// an `LlmAgent` node references an `agent_id` not found in the `agent_registry`.
    pub fn compile_with_agents(
        def: WorkflowDefinition,
        agent_registry: &HashMap<String, Arc<LLMAgent>>,
    ) -> DslResult<CompiledGraphImpl<JsonState>> {
        // 1. Validate the definition
        Self::validate(&def)?;

        // 2. Build the StateGraph
        let mut graph = StateGraphImpl::<JsonState>::build(&def.metadata.id);

        // 3. Add nodes
        for node_def in &def.nodes {
            let node_func = Self::compile_node(node_def, agent_registry)?;
            let node_id = node_def.id();
            graph.add_node(node_id, node_func);
        }

        // 4. Wire edges
        Self::wire_edges(&mut graph, &def)?;

        // 5. Compile
        graph.compile().map_err(|e| DslError::Build(e.to_string()))
    }

    /// Compile a single `NodeDefinition` into a `Box<dyn NodeFunc<JsonState>>`.
    fn compile_node(
        def: &NodeDefinition,
        agent_registry: &HashMap<String, Arc<LLMAgent>>,
    ) -> DslResult<Box<dyn NodeFunc<JsonState>>> {
        match def {
            NodeDefinition::Start { id, .. } => Ok(Box::new(PassthroughNode::new(id))),
            NodeDefinition::End { id, .. } => Ok(Box::new(PassthroughNode::new(id))),
            NodeDefinition::Task { id, executor, .. } => {
                Ok(Box::new(DslTaskNode::new(id, executor.clone())))
            }
            NodeDefinition::Condition { id, condition, .. } => {
                Ok(Box::new(DslConditionNode::new(id, condition.clone())))
            }
            NodeDefinition::Parallel { id, .. } => Ok(Box::new(PassthroughNode::new(id))),
            NodeDefinition::Join { id, wait_for, .. } => {
                Ok(Box::new(DslJoinNode::new(id, wait_for.clone())))
            }
            NodeDefinition::Transform { id, transform, .. } => {
                Ok(Box::new(DslTransformNode::new(id, transform.clone())))
            }
            NodeDefinition::LlmAgent { id, agent, .. } => {
                let agent_id = match agent {
                    AgentRef::Registry { agent_id } => agent_id,
                    AgentRef::Inline(_) => {
                        return Err(DslError::Validation(format!(
                            "Inline agents are not supported in DslCompiler for node '{}'. Please use registry agents.",
                            id
                        )));
                    }
                };

                if let Some(agent_instance) = agent_registry.get(agent_id) {
                    Ok(Box::new(DslAgentNode::new(
                        id,
                        agent_instance.clone(),
                        agent.clone(),
                    )))
                } else {
                    Err(DslError::Validation(format!(
                        "LlmAgent node '{}' requires agent_id '{}' which is not in the registry.",
                        id, agent_id
                    )))
                }
            }

            // Unsupported node types — clear errors
            NodeDefinition::Loop { id, .. } => Err(DslError::InvalidNodeType(format!(
                "Loop node '{}' is not yet supported by the DSL compiler.",
                id
            ))),
            NodeDefinition::SubWorkflow { id, .. } => Err(DslError::InvalidNodeType(format!(
                "SubWorkflow node '{}' is not yet supported by the DSL compiler.",
                id
            ))),
            NodeDefinition::Wait { id, .. } => Err(DslError::InvalidNodeType(format!(
                "Wait node '{}' is not yet supported by the DSL compiler.",
                id
            ))),
        }
    }

    /// Wire edges from the DSL definition into the StateGraph.
    fn wire_edges(
        graph: &mut StateGraphImpl<JsonState>,
        def: &WorkflowDefinition,
    ) -> DslResult<()> {
        // Find the start and end node IDs
        let start_id = def
            .nodes
            .iter()
            .find(|n| matches!(n, NodeDefinition::Start { .. }))
            .map(|n| n.id().to_string())
            .ok_or_else(|| DslError::Validation("Missing start node".into()))?;

        let end_id = def
            .nodes
            .iter()
            .find(|n| matches!(n, NodeDefinition::End { .. }))
            .map(|n| n.id().to_string())
            .ok_or_else(|| DslError::Validation("Missing end node".into()))?;

        // Set the entry point: START → first_node_after_start
        graph.set_entry_point(&start_id);

        // Set the finish point: end_node → END
        graph.set_finish_point(&end_id);

        // Group conditional edges by source: from → [(condition, to)]
        let mut conditional_map: HashMap<String, HashMap<String, String>> = HashMap::new();

        for edge in &def.edges {
            if let Some(ref condition) = edge.condition {
                conditional_map
                    .entry(edge.from.clone())
                    .or_default()
                    .insert(condition.clone(), edge.to.clone());
            } else {
                // Simple edge
                graph.add_edge(&edge.from, &edge.to);
            }
        }

        // Add grouped conditional edges
        for (from, conditions) in conditional_map {
            graph.add_conditional_edges(&from, conditions);
        }

        Ok(())
    }

    /// Validate a workflow definition for compilation.
    ///
    /// This reuses the parser's validation logic and adds compiler-specific checks.
    fn validate(def: &WorkflowDefinition) -> DslResult<()> {
        let node_ids: Vec<&str> = def.nodes.iter().map(|n| n.id()).collect();

        // Must have a start node
        if !def
            .nodes
            .iter()
            .any(|n| matches!(n, NodeDefinition::Start { .. }))
        {
            return Err(DslError::Validation(
                "Workflow must have a Start node".into(),
            ));
        }

        // Must have an end node
        if !def
            .nodes
            .iter()
            .any(|n| matches!(n, NodeDefinition::End { .. }))
        {
            return Err(DslError::Validation(
                "Workflow must have an End node".into(),
            ));
        }

        // All edge references must point to valid nodes
        for edge in &def.edges {
            if !node_ids.contains(&edge.from.as_str()) {
                return Err(DslError::InvalidEdge {
                    from: edge.from.clone(),
                    to: edge.to.clone(),
                });
            }
            if !node_ids.contains(&edge.to.as_str()) {
                return Err(DslError::InvalidEdge {
                    from: edge.from.clone(),
                    to: edge.to.clone(),
                });
            }
        }

        // No duplicate node IDs
        let mut seen = std::collections::HashSet::new();
        for id in &node_ids {
            if !seen.insert(id) {
                return Err(DslError::Validation(format!("Duplicate node ID: '{}'", id)));
            }
        }

        Ok(())
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use mofa_kernel::workflow::CompiledGraph;

    /// Helper: parse YAML into WorkflowDefinition
    fn parse_yaml(yaml: &str) -> WorkflowDefinition {
        serde_yaml::from_str(yaml).expect("Failed to parse YAML")
    }

    // -------------------------------------------------------------------------
    // Compilation Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_compile_minimal_workflow() {
        let yaml = r#"
metadata:
  id: minimal
  name: Minimal Workflow

nodes:
  - type: start
    id: start
  - type: end
    id: end

edges:
  - from: start
    to: end
"#;
        let def = parse_yaml(yaml);
        let compiled = DslCompiler::compile(def);
        assert!(
            compiled.is_ok(),
            "Minimal workflow should compile: {:?}",
            compiled.err()
        );
    }

    #[test]
    fn test_compile_linear_workflow() {
        let yaml = r#"
metadata:
  id: linear
  name: Linear Workflow

nodes:
  - type: start
    id: start
  - type: task
    id: process
    name: Process Data
    executor_type: none
  - type: task
    id: validate
    name: Validate Result
    executor_type: none
  - type: end
    id: end

edges:
  - from: start
    to: process
  - from: process
    to: validate
  - from: validate
    to: end
"#;
        let def = parse_yaml(yaml);
        let compiled = DslCompiler::compile(def);
        assert!(
            compiled.is_ok(),
            "Linear workflow should compile: {:?}",
            compiled.err()
        );
    }

    #[tokio::test]
    async fn test_compile_and_invoke_roundtrip() {
        let yaml = r#"
metadata:
  id: roundtrip
  name: Round Trip Test

nodes:
  - type: start
    id: start
  - type: task
    id: process
    name: Process
    executor_type: none
  - type: end
    id: end

edges:
  - from: start
    to: process
  - from: process
    to: end
"#;
        let def = parse_yaml(yaml);
        let compiled = DslCompiler::compile(def).expect("Should compile");
        let result = compiled.invoke(JsonState::new(), None).await;
        assert!(result.is_ok(), "Invoke should succeed: {:?}", result.err());
    }

    #[test]
    fn test_compile_conditional_edges() {
        let yaml = r#"
metadata:
  id: conditional
  name: Conditional Workflow

nodes:
  - type: start
    id: start
  - type: condition
    id: check
    name: Check Value
    condition:
      condition_type: value
      field: score
      operator: ">="
      value: 80
  - type: task
    id: pass
    name: Pass
    executor_type: none
  - type: task
    id: fail
    name: Fail
    executor_type: none
  - type: end
    id: end

edges:
  - from: start
    to: check
  - from: check
    to: pass
    condition: "true"
  - from: check
    to: fail
    condition: "false"
  - from: pass
    to: end
  - from: fail
    to: end
"#;
        let def = parse_yaml(yaml);
        let compiled = DslCompiler::compile(def);
        assert!(
            compiled.is_ok(),
            "Conditional workflow should compile: {:?}",
            compiled.err()
        );
    }

    #[test]
    fn test_compile_with_transform() {
        let yaml = r#"
metadata:
  id: transform_test
  name: Transform Test

nodes:
  - type: start
    id: start
  - type: transform
    id: transform1
    name: Apply Template
    transform_type: template
    template: "Hello {{ name }}"
  - type: end
    id: end

edges:
  - from: start
    to: transform1
  - from: transform1
    to: end
"#;
        let def = parse_yaml(yaml);
        let compiled = DslCompiler::compile(def);
        assert!(
            compiled.is_ok(),
            "Transform workflow should compile: {:?}",
            compiled.err()
        );
    }

    #[test]
    fn test_compile_with_join() {
        let yaml = r#"
metadata:
  id: join_test
  name: Join Test

nodes:
  - type: start
    id: start
  - type: task
    id: branch_a
    name: Branch A
    executor_type: none
  - type: task
    id: branch_b
    name: Branch B
    executor_type: none
  - type: join
    id: merge
    name: Merge Results
    wait_for:
      - branch_a
      - branch_b
  - type: end
    id: end

edges:
  - from: start
    to: branch_a
  - from: start
    to: branch_b
  - from: branch_a
    to: merge
  - from: branch_b
    to: merge
  - from: merge
    to: end
"#;
        let def = parse_yaml(yaml);
        let compiled = DslCompiler::compile(def);
        assert!(
            compiled.is_ok(),
            "Join workflow should compile: {:?}",
            compiled.err()
        );
    }

    // -------------------------------------------------------------------------
    // Validation Error Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_compile_missing_start_node() {
        let yaml = r#"
metadata:
  id: no_start
  name: No Start

nodes:
  - type: end
    id: end

edges: []
"#;
        let def = parse_yaml(yaml);
        let err = match DslCompiler::compile(def) {
            Err(e) => e,
            Ok(_) => panic!("Expected error, got Ok"),
        };
        assert!(
            matches!(err, DslError::Validation(ref msg) if msg.contains("Start")),
            "Expected validation error about missing start, got: {:?}",
            err
        );
    }

    #[test]
    fn test_compile_missing_end_node() {
        let yaml = r#"
metadata:
  id: no_end
  name: No End

nodes:
  - type: start
    id: start

edges: []
"#;
        let def = parse_yaml(yaml);
        let err = match DslCompiler::compile(def) {
            Err(e) => e,
            Ok(_) => panic!("Expected error, got Ok"),
        };
        assert!(
            matches!(err, DslError::Validation(ref msg) if msg.contains("End")),
            "Expected validation error about missing end, got: {:?}",
            err
        );
    }

    #[test]
    fn test_compile_invalid_edge_reference() {
        let yaml = r#"
metadata:
  id: bad_edge
  name: Bad Edge

nodes:
  - type: start
    id: start
  - type: end
    id: end

edges:
  - from: start
    to: nonexistent
"#;
        let def = parse_yaml(yaml);
        let err = match DslCompiler::compile(def) {
            Err(e) => e,
            Ok(_) => panic!("Expected error, got Ok"),
        };
        assert!(
            matches!(err, DslError::InvalidEdge { .. }),
            "Expected InvalidEdge error, got: {:?}",
            err
        );
    }

    #[test]
    fn test_compile_duplicate_node_ids() {
        let yaml = r#"
metadata:
  id: duplicates
  name: Duplicates

nodes:
  - type: start
    id: start
  - type: task
    id: start
    name: Duplicate
    executor_type: none
  - type: end
    id: end

edges:
  - from: start
    to: end
"#;
        let def = parse_yaml(yaml);
        let err = match DslCompiler::compile(def) {
            Err(e) => e,
            Ok(_) => panic!("Expected error, got Ok"),
        };
        assert!(
            matches!(err, DslError::Validation(ref msg) if msg.contains("Duplicate")),
            "Expected duplicate node ID error, got: {:?}",
            err
        );
    }

    #[test]
    fn test_compile_unsupported_llm_agent_missing_registry() {
        let yaml = r#"
metadata:
  id: llm_test_missing_registry
  name: LLM Test Missing Registry

nodes:
  - type: start
    id: start
  - type: llm_agent
    id: agent1
    name: My Agent
    agent:
      agent_id: test_missing
  - type: end
    id: end

edges:
  - from: start
    to: agent1
  - from: agent1
    to: end
"#;
        let def = parse_yaml(yaml);
        let err = match DslCompiler::compile(def) {
            Err(e) => e,
            Ok(_) => panic!("Expected error, got Ok"),
        };
        assert!(
            matches!(err, DslError::Validation(ref msg) if msg.contains("not in the registry")),
            "Expected missing registry validation error, got: {:?}",
            err
        );
    }

    // -------------------------------------------------------------------------
    // Condition Evaluation Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_evaluate_condition_equality() {
        assert!(evaluate_condition(
            &serde_json::json!("hello"),
            "==",
            &serde_json::json!("hello")
        ));
        assert!(!evaluate_condition(
            &serde_json::json!("hello"),
            "==",
            &serde_json::json!("world")
        ));
    }

    #[test]
    fn test_evaluate_condition_numeric() {
        assert!(evaluate_condition(
            &serde_json::json!(10),
            ">",
            &serde_json::json!(5)
        ));
        assert!(evaluate_condition(
            &serde_json::json!(5),
            "<=",
            &serde_json::json!(5)
        ));
        assert!(!evaluate_condition(
            &serde_json::json!(3),
            ">=",
            &serde_json::json!(5)
        ));
    }

    #[test]
    fn test_evaluate_condition_contains() {
        assert!(evaluate_condition(
            &serde_json::json!("hello world"),
            "contains",
            &serde_json::json!("world")
        ));
        assert!(!evaluate_condition(
            &serde_json::json!("hello"),
            "contains",
            &serde_json::json!("world")
        ));
    }

    #[test]
    fn test_evaluate_condition_inequality() {
        assert!(evaluate_condition(
            &serde_json::json!(1),
            "!=",
            &serde_json::json!(2)
        ));
        assert!(!evaluate_condition(
            &serde_json::json!(1),
            "!=",
            &serde_json::json!(1)
        ));
    }

    #[tokio::test]
    async fn test_condition_node_value_match() {
        let condition = ConditionDef::Value {
            field: "score".into(),
            operator: ">=".into(),
            value: serde_json::json!(80),
        };
        let node = DslConditionNode::new("check", condition);

        let mut state = JsonState::new();
        state
            .apply_update("score", serde_json::json!(90))
            .await
            .unwrap();

        let ctx = RuntimeContext::new("test");
        let cmd = node.call(&mut state, &ctx).await.unwrap();

        // Should have set "check" to "true"
        assert!(!cmd.updates.is_empty());
        assert_eq!(cmd.updates[0].key, "check");
        assert_eq!(cmd.updates[0].value, serde_json::json!("true"));
    }

    #[tokio::test]
    async fn test_condition_node_value_no_match() {
        let condition = ConditionDef::Value {
            field: "score".into(),
            operator: ">=".into(),
            value: serde_json::json!(80),
        };
        let node = DslConditionNode::new("check", condition);

        let mut state = JsonState::new();
        state
            .apply_update("score", serde_json::json!(50))
            .await
            .unwrap();

        let ctx = RuntimeContext::new("test");
        let cmd = node.call(&mut state, &ctx).await.unwrap();

        assert_eq!(cmd.updates[0].value, serde_json::json!("false"));
    }

    #[tokio::test]
    async fn test_task_node_function_executor() {
        let node = DslTaskNode::new(
            "task1",
            TaskExecutorDef::Function {
                function: "my_function".into(),
            },
        );

        let mut state = JsonState::new();
        let ctx = RuntimeContext::new("test");
        let cmd = node.call(&mut state, &ctx).await.unwrap();

        assert!(!cmd.updates.is_empty());
        assert_eq!(cmd.updates[0].key, "last_task");
    }
}
