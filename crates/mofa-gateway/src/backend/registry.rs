//! In-memory [`CapabilityRegistry`] implementation.

use mofa_kernel::gateway::{
    BackendHealth, BackendKind, CapabilityDescriptor, CapabilityRegistry, GatewayError,
};
use std::collections::HashMap;

/// [`CapabilityRegistry`] backed by a simple `HashMap`.
///
/// Suitable for single-node deployments.  Distributed/service-mesh
/// implementations belong in separate plugin crates.
#[derive(Default)]
pub struct InMemoryCapabilityRegistry {
    store: HashMap<String, CapabilityDescriptor>,
}

impl InMemoryCapabilityRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self::default()
    }
}

impl CapabilityRegistry for InMemoryCapabilityRegistry {
    fn register(&mut self, descriptor: CapabilityDescriptor) -> Result<(), GatewayError> {
        if self.store.contains_key(&descriptor.id) {
            return Err(GatewayError::DuplicateBackend(descriptor.id));
        }
        self.store.insert(descriptor.id.clone(), descriptor);
        Ok(())
    }

    fn lookup(&self, id: &str) -> Option<&CapabilityDescriptor> {
        self.store.get(id)
    }

    fn list_by_kind(&self, kind: &BackendKind) -> Vec<&CapabilityDescriptor> {
        self.store.values().filter(|d| &d.kind == kind).collect()
    }

    fn list_all(&self) -> Vec<&CapabilityDescriptor> {
        self.store.values().collect()
    }

    fn deregister(&mut self, id: &str) -> Result<(), GatewayError> {
        self.store
            .remove(id)
            .map(|_| ())
            .ok_or_else(|| GatewayError::BackendNotFound(id.to_string()))
    }

    fn update_health(&mut self, id: &str, health: BackendHealth) -> Result<(), GatewayError> {
        self.store
            .get_mut(id)
            .map(|d| d.health = health)
            .ok_or_else(|| GatewayError::BackendNotFound(id.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn openai() -> CapabilityDescriptor {
        CapabilityDescriptor::new("openai", BackendKind::LlmOpenAI, "https://api.openai.com")
    }

    #[test]
    fn register_and_lookup() {
        let mut reg = InMemoryCapabilityRegistry::new();
        reg.register(openai()).unwrap();
        assert!(reg.lookup("openai").is_some());
        assert!(reg.lookup("unknown").is_none());
    }

    #[test]
    fn duplicate_register_returns_error() {
        let mut reg = InMemoryCapabilityRegistry::new();
        reg.register(openai()).unwrap();
        assert!(matches!(
            reg.register(openai()),
            Err(GatewayError::DuplicateBackend(_))
        ));
    }

    #[test]
    fn list_by_kind_filters_correctly() {
        let mut reg = InMemoryCapabilityRegistry::new();
        reg.register(openai()).unwrap();
        reg.register(CapabilityDescriptor::new(
            "ha-hub",
            BackendKind::IoT,
            "http://homeassistant.local:8123",
        ))
        .unwrap();

        assert_eq!(reg.list_by_kind(&BackendKind::LlmOpenAI).len(), 1);
        assert_eq!(reg.list_by_kind(&BackendKind::IoT).len(), 1);
        assert_eq!(reg.list_by_kind(&BackendKind::McpTool).len(), 0);
    }

    #[test]
    fn deregister_removes_entry() {
        let mut reg = InMemoryCapabilityRegistry::new();
        reg.register(openai()).unwrap();
        reg.deregister("openai").unwrap();
        assert!(reg.lookup("openai").is_none());
    }

    #[test]
    fn update_health_reflects_new_state() {
        let mut reg = InMemoryCapabilityRegistry::new();
        reg.register(openai()).unwrap();
        reg.update_health("openai", BackendHealth::Healthy).unwrap();
        assert_eq!(reg.lookup("openai").unwrap().health, BackendHealth::Healthy);
    }
}
