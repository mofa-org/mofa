use std::sync::Arc;

use crate::agent::{AgentRegistry, ExecutionEngine};

/// Shared state for the control plane and gateway layers.
#[derive(Clone)]
pub struct ControlPlaneState {
    registry: Arc<AgentRegistry>,
    execution_engine: Arc<ExecutionEngine>,
}

trait ControlPlaneStateBounds: Send + Sync + 'static {}
impl ControlPlaneStateBounds for ControlPlaneState {}

impl ControlPlaneState {
    /// Create new shared state from the runtime primitives.
    pub fn new(registry: Arc<AgentRegistry>, execution_engine: Arc<ExecutionEngine>) -> Self {
        Self {
            registry,
            execution_engine,
        }
    }

    /// Access the agent registry.
    pub fn registry(&self) -> Arc<AgentRegistry> {
        self.registry.clone()
    }

    /// Access the execution engine.
    pub fn execution_engine(&self) -> Arc<ExecutionEngine> {
        self.execution_engine.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructs_state_from_shared_components() {
        let registry = Arc::new(AgentRegistry::new());
        let engine = Arc::new(ExecutionEngine::new(registry.clone()));

        let state = ControlPlaneState::new(registry.clone(), engine.clone());

        assert!(Arc::ptr_eq(&registry, &state.registry()));
        assert!(Arc::ptr_eq(&engine, &state.execution_engine()));
    }

    #[test]
    fn clones_preserve_shared_references() {
        let registry = Arc::new(AgentRegistry::new());
        let engine = Arc::new(ExecutionEngine::new(registry.clone()));

        let state = ControlPlaneState::new(registry.clone(), engine.clone());
        let cloned = state.clone();

        assert!(Arc::ptr_eq(&state.registry(), &cloned.registry()));
        assert!(Arc::ptr_eq(
            &state.execution_engine(),
            &cloned.execution_engine()
        ));
    }
}
