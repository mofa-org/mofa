//! Workflow DSL Schema
//!
//! Defines the data structures for declarative workflow configuration.

use crate::workflow::node::NodeType;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Workflow definition from YAML/TOML
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowDefinition {
    /// Workflow metadata
    pub metadata: WorkflowMetadata,

    /// Workflow configuration
    #[serde(default)]
    pub config: WorkflowConfig,

    /// Node definitions
    pub nodes: Vec<NodeDefinition>,

    /// Edge definitions
    #[serde(default)]
    pub edges: Vec<EdgeDefinition>,

    /// Agent definitions (inline or reusable)
    #[serde(default)]
    pub agents: HashMap<String, LlmAgentConfig>,
}

/// Workflow metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowMetadata {
    /// Unique workflow identifier
    pub id: String,

    /// Human-readable name
    pub name: String,

    /// Workflow description
    #[serde(default)]
    pub description: String,

    /// Workflow version
    #[serde(default)]
    pub version: Option<String>,

    /// Author
    #[serde(default)]
    pub author: Option<String>,

    /// Tags
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Workflow-level configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowConfig {
    /// Maximum parallel executions
    #[serde(default = "default_max_parallel")]
    pub max_parallel: usize,

    /// Default timeout (milliseconds)
    #[serde(default = "default_timeout")]
    pub default_timeout_ms: u64,

    /// Enable checkpoints
    #[serde(default)]
    pub enable_checkpoints: bool,

    /// Retry policy for all nodes
    #[serde(default)]
    pub retry_policy: Option<RetryPolicy>,
}

impl Default for WorkflowConfig {
    fn default() -> Self {
        Self {
            max_parallel: default_max_parallel(),
            default_timeout_ms: default_timeout(),
            enable_checkpoints: false,
            retry_policy: None,
        }
    }
}

fn default_max_parallel() -> usize {
    10
}

fn default_timeout() -> u64 {
    60000 // 1 minute
}

/// Node definition (tagged enum for different node types)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NodeDefinition {
    /// Start node
    Start {
        id: String,
        #[serde(default)]
        name: Option<String>,
    },

    /// End node
    End {
        id: String,
        #[serde(default)]
        name: Option<String>,
    },

    /// Task node (custom executor)
    Task {
        id: String,
        name: String,
        #[serde(flatten)]
        executor: TaskExecutorDef,
        #[serde(default)]
        config: NodeConfigDef,
    },

    /// LLM Agent node
    LLM_AGENT {
        id: String,
        name: String,
        /// Agent reference (agent_id for registry agents, inline for embedded)
        agent: AgentRef,
        /// Optional prompt template
        #[serde(default)]
        prompt_template: Option<String>,
        #[serde(default)]
        config: NodeConfigDef,
    },

    /// Condition node
    Condition {
        id: String,
        name: String,
        condition: ConditionDef,
        #[serde(default)]
        config: NodeConfigDef,
    },

    /// Parallel node
    Parallel {
        id: String,
        name: String,
        #[serde(default)]
        config: NodeConfigDef,
    },

    /// Join node
    Join {
        id: String,
        name: String,
        /// List of node IDs to wait for
        #[serde(default)]
        wait_for: Vec<String>,
        #[serde(default)]
        config: NodeConfigDef,
    },

    /// Loop node
    Loop {
        id: String,
        name: String,
        #[serde(flatten)]
        body: TaskExecutorDef,
        condition: LoopConditionDef,
        #[serde(default)]
        max_iterations: u32,
        #[serde(default)]
        config: NodeConfigDef,
    },

    /// Transform node
    Transform {
        id: String,
        name: String,
        #[serde(flatten)]
        transform: TransformDef,
        #[serde(default)]
        config: NodeConfigDef,
    },

    /// Sub-workflow node
    SubWorkflow {
        id: String,
        name: String,
        /// Reference to another workflow
        workflow_id: String,
        #[serde(default)]
        config: NodeConfigDef,
    },

    /// Wait node
    Wait {
        id: String,
        name: String,
        /// Event type to wait for
        event_type: String,
        #[serde(default)]
        config: NodeConfigDef,
    },
}

impl NodeDefinition {
    /// Get the node ID
    pub fn id(&self) -> &str {
        match self {
            NodeDefinition::Start { id, .. } => id,
            NodeDefinition::End { id, .. } => id,
            NodeDefinition::Task { id, .. } => id,
            NodeDefinition::LLM_AGENT { id, .. } => id,
            NodeDefinition::Condition { id, .. } => id,
            NodeDefinition::Parallel { id, .. } => id,
            NodeDefinition::Join { id, .. } => id,
            NodeDefinition::Loop { id, .. } => id,
            NodeDefinition::Transform { id, .. } => id,
            NodeDefinition::SubWorkflow { id, .. } => id,
            NodeDefinition::Wait { id, .. } => id,
        }
    }

    /// Get the node type
    pub fn node_type(&self) -> NodeType {
        match self {
            NodeDefinition::Start { .. } => NodeType::Start,
            NodeDefinition::End { .. } => NodeType::End,
            NodeDefinition::Task { .. } => NodeType::Task,
            NodeDefinition::LLM_AGENT { .. } => NodeType::Agent,
            NodeDefinition::Condition { .. } => NodeType::Condition,
            NodeDefinition::Parallel { .. } => NodeType::Parallel,
            NodeDefinition::Join { .. } => NodeType::Join,
            NodeDefinition::Loop { .. } => NodeType::Loop,
            NodeDefinition::Transform { .. } => NodeType::Transform,
            NodeDefinition::SubWorkflow { .. } => NodeType::SubWorkflow,
            NodeDefinition::Wait { .. } => NodeType::Wait,
        }
    }
}

/// Edge definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeDefinition {
    /// Source node ID
    pub from: String,

    /// Target node ID
    pub to: String,

    /// Conditional edge (optional)
    #[serde(default)]
    pub condition: Option<String>,

    /// Edge label (optional)
    #[serde(default)]
    pub label: Option<String>,
}

/// LLM Agent configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmAgentConfig {
    /// Model identifier
    pub model: String,

    /// System prompt
    #[serde(default)]
    pub system_prompt: Option<String>,

    /// Temperature
    #[serde(default)]
    pub temperature: Option<f32>,

    /// Max tokens
    #[serde(default)]
    pub max_tokens: Option<u32>,

    /// Context window size (in rounds)
    #[serde(default)]
    pub context_window_size: Option<usize>,

    /// User ID for persistence
    #[serde(default)]
    pub user_id: Option<String>,

    /// Tenant ID for persistence
    #[serde(default)]
    pub tenant_id: Option<String>,
}

/// Agent reference (can be registry or inline)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AgentRef {
    /// Reference to agent by ID (in registry)
    Registry { agent_id: String },

    /// Inline agent configuration
    Inline(Box<LlmAgentConfig>),
}

/// Task executor definition
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "executor_type", rename_all = "snake_case")]
pub enum TaskExecutorDef {
    /// Function executor (for code-defined tasks)
    Function { function: String },

    /// HTTP request executor
    Http { url: String, #[serde(default)] method: Option<String> },

    /// Script executor (Rhai)
    Script { script: String },

    /// No-op executor (for testing)
    None,
}

/// Condition definition
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "condition_type", rename_all = "snake_case")]
pub enum ConditionDef {
    /// Expression-based condition
    Expression { expr: String },

    /// Value-based condition
    Value {
        field: String,
        operator: String,
        value: serde_json::Value,
    },
}

/// Loop condition definition
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "condition_type", rename_all = "snake_case")]
pub enum LoopConditionDef {
    /// While-style loop
    While { expr: String },

    /// Until-style loop
    Until { expr: String },

    /// Count-based loop
    Count { max: u32 },
}

/// Transform definition
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "transform_type", rename_all = "snake_case")]
pub enum TransformDef {
    /// Jinja-style template
    Template { template: String },

    /// JavaScript expression
    Expression { expr: String },

    /// Map/reduce operation
    MapReduce {
        #[serde(default)]
        map: Option<String>,
        #[serde(default)]
        reduce: Option<String>,
    },
}

/// Node-level configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(Default)]
pub struct NodeConfigDef {
    /// Retry policy
    #[serde(default)]
    pub retry_policy: Option<RetryPolicy>,

    /// Timeout (milliseconds)
    #[serde(default)]
    pub timeout_ms: Option<u64>,

    /// Custom metadata
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}


/// Retry policy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    /// Maximum retry attempts
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,

    /// Delay between retries (milliseconds)
    #[serde(default = "default_retry_delay")]
    pub retry_delay_ms: u64,

    /// Enable exponential backoff
    #[serde(default = "default_exponential_backoff")]
    pub exponential_backoff: bool,

    /// Maximum delay (milliseconds)
    #[serde(default = "default_max_delay")]
    pub max_delay_ms: u64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: default_max_retries(),
            retry_delay_ms: default_retry_delay(),
            exponential_backoff: default_exponential_backoff(),
            max_delay_ms: default_max_delay(),
        }
    }
}

fn default_max_retries() -> u32 {
    3
}

fn default_retry_delay() -> u64 {
    1000
}

fn default_exponential_backoff() -> bool {
    true
}

fn default_max_delay() -> u64 {
    30000
}

/// Timeout configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeoutConfig {
    /// Execution timeout (milliseconds)
    pub execution_timeout_ms: u64,

    /// Cancel on timeout
    #[serde(default = "default_cancel_on_timeout")]
    pub cancel_on_timeout: bool,
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self {
            execution_timeout_ms: 60000,
            cancel_on_timeout: true,
        }
    }
}

fn default_cancel_on_timeout() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_workflow_yaml() {
        let yaml = r#"
metadata:
  id: test_workflow
  name: Test Workflow
  description: A test workflow

nodes:
  - type: start
    id: start

  - type: llm_agent
    id: agent1
    name: First Agent
    agent:
      agent_id: my_agent

  - type: end
    id: end

edges:
  - from: start
    to: agent1
  - from: agent1
    to: end
"#;

        let def: WorkflowDefinition = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(def.metadata.id, "test_workflow");
        assert_eq!(def.nodes.len(), 3);
        assert_eq!(def.edges.len(), 2);
    }

    #[test]
    fn test_parse_agent_config() {
        let yaml = r#"
agents:
  my_agent:
    model: gpt-4
    system_prompt: "You are helpful"
    temperature: 0.7
    max_tokens: 2000
"#;

        let def: WorkflowDefinition = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(def.agents.len(), 1);
        let agent = def.agents.get("my_agent").unwrap();
        assert_eq!(agent.model, "gpt-4");
        assert_eq!(agent.temperature, Some(0.7));
    }

    #[test]
    fn test_parse_toml() {
        let toml = r#"
[metadata]
id = "test_workflow"
name = "Test Workflow"

[[nodes]]
type = "start"
id = "start"

[[nodes]]
type = "end"
id = "end"

[[edges]]
from = "start"
to = "end"
"#;

        let def: WorkflowDefinition = toml::from_str(toml).unwrap();
        assert_eq!(def.metadata.id, "test_workflow");
        assert_eq!(def.nodes.len(), 2);
    }
}
