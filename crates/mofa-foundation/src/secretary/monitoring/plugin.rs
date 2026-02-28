//! Event response plugin system
//!
//! This module defines the event response plugin interface and basic
//! implementation for handling various operational events.

use super::event::{Event, EventStatus, EventType};
use async_trait::async_trait;
use mofa_kernel::plugin::{
    AgentPlugin, PluginContext, PluginMetadata, PluginPriority, PluginResult, PluginState,
    PluginType,
};
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::collections::HashMap;

/// Event response plugin configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventResponseConfig {
    /// Whether the plugin is enabled
    pub enabled: bool,
    /// Plugin priority
    pub priority: PluginPriority,
    /// Event types handled by this plugin
    pub handled_event_types: Vec<EventType>,
    /// Maximum impact scope this plugin can handle
    pub max_impact_scope: String, // Using string for flexibility
    /// Rule configuration
    pub rules: HashMap<String, serde_json::Value>,
}

impl Default for EventResponseConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            priority: PluginPriority::Normal,
            handled_event_types: vec![],
            max_impact_scope: "component".to_string(),
            rules: HashMap::new(),
        }
    }
}

/// Event response plugin trait
#[async_trait]
pub trait EventResponsePlugin: AgentPlugin {
    /// Get the plugin configuration
    fn config(&self) -> &EventResponseConfig;

    /// Update the plugin configuration at runtime
    async fn update_config(&mut self, config: EventResponseConfig) -> PluginResult<()>;

    /// Check if this plugin can handle the given event
    fn can_handle(&self, event: &Event) -> bool;

    /// Handle the event
    async fn handle_event(&mut self, event: Event) -> PluginResult<Event>;

    /// Execute the response workflow for the event
    async fn execute_workflow(&self, event: &Event) -> PluginResult<HashMap<String, String>>;
}

/// Base implementation of EventResponsePlugin
pub struct BaseEventResponsePlugin {
    metadata: PluginMetadata,
    state: PluginState,
    config: EventResponseConfig,
    /// Cached handled event types for synchronous access
    handled_event_types: Vec<EventType>,
    workflow_steps: Vec<String>,
}

impl BaseEventResponsePlugin {
    /// Create a new base event response plugin
    pub fn new(
        id: &str,
        name: &str,
        handled_event_types: Vec<EventType>,
        workflow_steps: Vec<String>,
    ) -> Self {
        let metadata = PluginMetadata::new(id, name, PluginType::Tool)
            .with_priority(PluginPriority::Normal)
            .with_capability("event-response");

        let config = EventResponseConfig {
            handled_event_types: handled_event_types.clone(),
            ..Default::default()
        };

        Self {
            metadata,
            state: PluginState::Unloaded,
            config,
            handled_event_types,
            workflow_steps,
        }
    }

    /// Set the plugin priority
    pub fn with_priority(mut self, priority: PluginPriority) -> Self {
        self.metadata = self.metadata.with_priority(priority);
        self
    }

    /// Set the max impact scope
    pub fn with_max_impact_scope(mut self, scope: &str) -> Self {
        // Note: max_impact_scope is stored here for when decision logic reads it.
        // Plugins that override config independently should also set this field
        // in their own EventResponseConfig.
        self.config.max_impact_scope = scope.to_string();
        self
    }
}

#[async_trait]
impl AgentPlugin for BaseEventResponsePlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    fn state(&self) -> PluginState {
        self.state.clone()
    }

    async fn load(&mut self, _ctx: &PluginContext) -> PluginResult<()> {
        self.state = PluginState::Loading;
        // Perform resource allocation if needed
        self.state = PluginState::Loaded;
        Ok(())
    }

    async fn init_plugin(&mut self) -> PluginResult<()> {
        // Perform initialization logic
        Ok(())
    }

    async fn start(&mut self) -> PluginResult<()> {
        self.state = PluginState::Running;
        Ok(())
    }

    async fn stop(&mut self) -> PluginResult<()> {
        self.state = PluginState::Paused;
        Ok(())
    }

    async fn unload(&mut self) -> PluginResult<()> {
        self.state = PluginState::Unloaded;
        // Release resources if needed
        Ok(())
    }

    async fn execute(&mut self, input: String) -> PluginResult<String> {
        // Parse input as Event
        let mut event: Event = serde_json::from_str(&input)?;

        // Check if we can handle this event
        if !self.can_handle(&event) {
            return Err(mofa_kernel::plugin::PluginError::ExecutionFailed("Cannot handle this event type".into()));
        }

        // Handle the event
        event.update_status(EventStatus::Processing);
        let processed_event = self.handle_event(event).await?;

        // Return the result as JSON
        processed_event.to_json().map_err(|e| mofa_kernel::plugin::PluginError::ExecutionFailed(e.to_string()))
    }

    fn stats(&self) -> HashMap<String, serde_json::Value> {
        HashMap::new() // Implement stats collection if needed
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }
}

#[async_trait]
impl EventResponsePlugin for BaseEventResponsePlugin {
    fn config(&self) -> &EventResponseConfig {
        &self.config
    }

    async fn update_config(&mut self, config: EventResponseConfig) -> PluginResult<()> {
        self.config = config;
        Ok(())
    }

    fn can_handle(&self, event: &Event) -> bool {
        // Use cached handled_event_types for synchronous access
        self.handled_event_types.contains(&event.event_type)
    }

    async fn handle_event(&mut self, mut event: Event) -> PluginResult<Event> {
        // Execute the response workflow
        let workflow_result = self.execute_workflow(&event).await?;

        // Update event status based on workflow result
        event.update_status(EventStatus::Resolved);

        // Add workflow result to event data
        event.data["workflow_result"] = serde_json::json!(workflow_result);

        Ok(event)
    }

    async fn execute_workflow(&self, event: &Event) -> PluginResult<HashMap<String, String>> {
        // Default workflow implementation - to be overridden by concrete plugins
        let mut result = HashMap::new();
        result.insert("status".to_string(), "handled".to_string());
        result.insert(
            "message".to_string(),
            format!("Event handled by default workflow: {:?}", event.event_type),
        );
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secretary::monitoring::event::*;

    #[test]
    fn test_base_plugin_creation() {
        let plugin = BaseEventResponsePlugin::new(
            "test-plugin",
            "Test Plugin",
            vec![EventType::ServerFault],
            vec!["step1".to_string(), "step2".to_string()],
        );

        assert_eq!(plugin.metadata().id, "test-plugin");
        assert_eq!(plugin.metadata().name, "Test Plugin");
        assert_eq!(plugin.state(), PluginState::Unloaded);
    }

    #[tokio::test]
    async fn test_can_handle_event() {
        let plugin = BaseEventResponsePlugin::new(
            "test-plugin",
            "Test Plugin",
            vec![EventType::ServerFault, EventType::ServiceException],
            vec![],
        );

        let server_fault_event = Event::new(
            EventType::ServerFault,
            EventPriority::High,
            ImpactScope::Instance("server-01".to_string()),
            "monitoring".to_string(),
            "Server down".to_string(),
            serde_json::Value::Null,
        );

        let network_attack_event = Event::new(
            EventType::NetworkAttack,
            EventPriority::Emergency,
            ImpactScope::System,
            "ids".to_string(),
            "DDoS attack".to_string(),
            serde_json::Value::Null,
        );

        assert!(plugin.can_handle(&server_fault_event));
        assert!(!plugin.can_handle(&network_attack_event));
    }
}
