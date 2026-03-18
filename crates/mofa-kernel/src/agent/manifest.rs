//! Agent capability manifest and discovery types.
//!
//! Agents publish an `AgentManifest` so orchestrators can discover and route
//! tasks without holding hardcoded references to specific agent instances.

use crate::agent::capabilities::AgentCapabilities;
use crate::agent::config::schema::CoordinationMode;
use serde::{Deserialize, Serialize};

/// Machine-readable declaration of what an agent is capable of doing.
///
/// Agents register a manifest with the `CapabilityRegistry` on startup so
/// orchestrators can query for the right agent at runtime.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentManifest {
    /// Unique agent identifier.
    pub agent_id: String,
    /// Human-readable agent name.
    pub name: String,
    /// Natural-language description of what this agent does.
    pub description: String,
    /// Structured capability set (input/output types, tags, tool support, etc.).
    pub capabilities: AgentCapabilities,
    /// Names of tools registered with this agent.
    pub tools: Vec<String>,
    /// Memory backend in use (e.g. `"in-memory"`, `"qdrant"`, `"postgres"`).
    pub memory_backend: Option<String>,
    /// Coordination modes this agent participates in.
    pub coordination_modes: Vec<CoordinationMode>,
}

impl AgentManifest {
    /// Returns a builder for constructing an `AgentManifest`.
    pub fn builder(agent_id: impl Into<String>, name: impl Into<String>) -> AgentManifestBuilder {
        AgentManifestBuilder::new(agent_id.into(), name.into())
    }
}

/// Builder for [`AgentManifest`].
#[derive(Debug, Default)]
pub struct AgentManifestBuilder {
    agent_id: String,
    name: String,
    description: String,
    capabilities: AgentCapabilities,
    tools: Vec<String>,
    memory_backend: Option<String>,
    coordination_modes: Vec<CoordinationMode>,
}

impl AgentManifestBuilder {
    fn new(agent_id: String, name: String) -> Self {
        Self {
            agent_id,
            name,
            ..Default::default()
        }
    }

    /// Set the natural-language description.
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    /// Set the structured capability set.
    pub fn capabilities(mut self, caps: AgentCapabilities) -> Self {
        self.capabilities = caps;
        self
    }

    /// Register a tool name.
    pub fn tool(mut self, tool: impl Into<String>) -> Self {
        self.tools.push(tool.into());
        self
    }

    /// Set the memory backend type.
    pub fn memory_backend(mut self, backend: impl Into<String>) -> Self {
        self.memory_backend = Some(backend.into());
        self
    }

    /// Add a supported coordination mode.
    pub fn coordination_mode(mut self, mode: CoordinationMode) -> Self {
        self.coordination_modes.push(mode);
        self
    }

    /// Build the `AgentManifest`.
    pub fn build(self) -> AgentManifest {
        AgentManifest {
            agent_id: self.agent_id,
            name: self.name,
            description: self.description,
            capabilities: self.capabilities,
            tools: self.tools,
            memory_backend: self.memory_backend,
            coordination_modes: self.coordination_modes,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::capabilities::AgentCapabilities;

    #[test]
    fn test_manifest_builder() {
        let manifest = AgentManifest::builder("agent-001", "ResearchAgent")
            .description("searches the web and summarizes documents")
            .capabilities(
                AgentCapabilities::builder()
                    .with_tag("research")
                    .with_tag("summarization")
                    .build(),
            )
            .tool("web_search")
            .tool("document_reader")
            .memory_backend("in-memory")
            .coordination_mode(CoordinationMode::Sequential)
            .build();

        assert_eq!(manifest.agent_id, "agent-001");
        assert_eq!(manifest.name, "ResearchAgent");
        assert!(manifest.capabilities.has_tag("research"));
        assert_eq!(manifest.tools.len(), 2);
        assert_eq!(manifest.memory_backend.as_deref(), Some("in-memory"));
        assert_eq!(manifest.coordination_modes.len(), 1);
    }

    #[test]
    fn test_manifest_serialization() {
        let manifest = AgentManifest::builder("agent-002", "CodeAgent")
            .description("writes and reviews Rust code")
            .build();

        let json = serde_json::to_string(&manifest).unwrap();
        let restored: AgentManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.agent_id, "agent-002");
        assert_eq!(restored.name, "CodeAgent");
    }
}
