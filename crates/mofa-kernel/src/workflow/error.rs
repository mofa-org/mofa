use thiserror::Error;

/// Workflow error types
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum WorkflowError {
    /// A cyclic dependency was detected in the workflow graph
    #[error("Cyclic dependency detected in workflow nodes: {agents:?}")]
    CyclicDependency {
        /// Nodes involved in the cycle
        agents: Vec<String>,
    },
    
    /// Other workflow errors that don't fit into the above categories
    #[error("{0}")]
    Other(String),
}
