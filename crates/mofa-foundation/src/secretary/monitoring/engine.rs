//! Event handling engine
//!
//! This module provides the main event handling engine that manages plugins
//! and dispatches events to the appropriate handlers.

use super::event::{Event, EventStatus};
use super::plugin::EventResponsePlugin;
use super::rule_manager::{RuleAdjustmentStrategy, RuleManager};
use mofa_kernel::plugin::{AgentPlugin, PluginContext, PluginResult};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::{RwLock, Semaphore};

/// Event handling engine
pub struct EventHandlingEngine {
    /// Rule manager for runtime rule adjustment
    rule_manager: Arc<RuleManager>,
    /// Event queue
    event_queue: Arc<RwLock<VecDeque<Event>>>,
    /// Plugins registered with the engine
    plugins: Arc<RwLock<HashMap<String, Box<dyn EventResponsePlugin + Send + Sync>>>>,
    /// Plugin context
    plugin_context: PluginContext,
    /// Maximum concurrent event handlers
    max_concurrent_handlers: usize,
    /// Semaphore to control concurrent handlers
    semaphore: Arc<Semaphore>,
}

impl EventHandlingEngine {
    /// Create a new event handling engine
    pub fn new() -> Self {
        Self {
            rule_manager: Arc::new(RuleManager::new()),
            event_queue: Arc::new(RwLock::new(VecDeque::new())),
            plugins: Arc::new(RwLock::new(HashMap::new())),
            plugin_context: PluginContext::new("event-handling-engine"),
            max_concurrent_handlers: 10,
            semaphore: Arc::new(Semaphore::new(10)),
        }
    }

    /// Create a new event handling engine with custom concurrent handlers limit
    pub fn with_max_concurrent_handlers(mut self, limit: usize) -> Self {
        self.max_concurrent_handlers = limit;
        self.semaphore = Arc::new(Semaphore::new(limit));
        self
    }

    /// Set rule adjustment strategy
    pub async fn set_rule_strategy(&self, strategy: RuleAdjustmentStrategy) {
        self.rule_manager.set_strategy(strategy).await;
    }

    /// Register an event response plugin
    pub async fn register_plugin(&self, plugin: Box<dyn EventResponsePlugin + Send + Sync>) {
        // Register with the engine
        let plugin_id = plugin.metadata().id.to_string();
        let mut plugins = self.plugins.write().await;

        // Ownership of the plugin is transferred to the engine
        plugins.insert(plugin_id.clone(), plugin);

        // Note: Removed adding to rule manager due to trait object cloning restrictions
        // We can modify the rule manager to use shared ownership if needed
    }

    /// Register multiple plugins at once
    pub async fn register_plugins(&self, plugins: Vec<Box<dyn EventResponsePlugin + Send + Sync>>) {
        for plugin in plugins {
            self.register_plugin(plugin).await;
        }
    }

    /// Submit an event to be handled
    pub async fn submit_event(&self, event: Event) {
        println!(
            "Submitted new event: [{}] {} - {}",
            event.priority, event.source, event.description
        );

        // Lock the queue and add the event
        let mut queue = self.event_queue.write().await;
        queue.push_back(event);
    }

    /// Process the next event in the queue
    pub async fn process_next_event(&self) -> PluginResult<Option<Event>> {
        // Acquire a semaphore permit
        let semaphore = self.semaphore.clone();
        let _permit = semaphore.acquire().await.unwrap();

        // Get the next event from the queue
        let mut queue = self.event_queue.write().await;
        let mut event = match queue.pop_front() {
            Some(event) => event,
            None => return Ok(None),
        };

        // Update event status
        event.update_status(EventStatus::Processing);

        // Adjust rules based on the event
        self.rule_manager.adjust_rules(&event).await?;

        // Lock plugins for reading
        let mut plugins = self.plugins.write().await;

        // Find the first plugin that can handle the event
        for (_plugin_id, plugin) in plugins.iter_mut() {
            if plugin.can_handle(&event) {
                println!(
                    "Processing event {} with plugin: {}",
                    event.id,
                    plugin.metadata().name
                );

                // Process the event
                // Downcast to mutable EventResponsePlugin
                let plugin_mut = plugin
                    .as_any_mut()
                    .downcast_mut::<Box<dyn EventResponsePlugin + Send + Sync>>()
                    .unwrap();
                let processed_event = plugin_mut.handle_event(event).await?;

                println!(
                    "Event {} processed successfully by plugin {}",
                    processed_event.id,
                    plugin.metadata().name
                );

                return Ok(Some(processed_event));
            }
        }

        // No plugin found to handle the event
        println!("No plugin found to handle event: {}", event.id);
        event.update_status(EventStatus::ManualInterventionNeeded);

        Ok(Some(event))
    }

    /// Start the engine and process events continuously
    pub async fn start(&self) -> PluginResult<()> {
        println!(
            "Starting event handling engine with {} concurrent handlers...",
            self.max_concurrent_handlers
        );

        // Keep processing events forever
        loop {
            match self.process_next_event().await {
                Ok(Some(event)) => {
                    // Event processed, do any post-processing if needed
                    if event.status == EventStatus::Resolved {
                        println!("Event resolved: {}", event.id);
                    } else {
                        println!("Event {} status: {:?}", event.id, event.status);
                    }
                }
                Ok(None) => {
                    // No events in queue, wait a bit before checking again
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                }
                Err(err) => {
                    // Handle error
                    println!("Error processing event: {}", err);
                    // Continue processing
                    tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secretary::monitoring::event::*;
    use crate::secretary::monitoring::plugins::{
        NetworkAttackResponsePlugin, ServerFaultResponsePlugin,
    };

    #[tokio::test]
    async fn test_event_handling() {
        // Create event engine
        let engine = EventHandlingEngine::new();

        // Create plugins
        let server_fault_plugin = ServerFaultResponsePlugin::new();
        let network_attack_plugin = NetworkAttackResponsePlugin::new();

        // Register plugins with explicit boxing
        engine
            .register_plugins(vec![
                Box::new(server_fault_plugin) as Box<dyn EventResponsePlugin + Send + Sync>,
                Box::new(network_attack_plugin) as Box<dyn EventResponsePlugin + Send + Sync>,
            ])
            .await;

        // Create server fault event
        let server_fault_event = Event::new(
            EventType::ServerFault,
            EventPriority::High,
            ImpactScope::Instance("web-server-01".to_string()),
            "monitoring-agent".to_string(),
            "Server unresponsive".to_string(),
            serde_json::json!({ "server": "web-server-01" }),
        );

        // Create network attack event
        let network_attack_event = Event::new(
            EventType::NetworkAttack,
            EventPriority::Emergency,
            ImpactScope::Service("api-gateway".to_string()),
            "ids".to_string(),
            "DDoS attack".to_string(),
            serde_json::json!({ "source_ip": "10.0.0.1" }),
        );

        // Submit both events
        engine.submit_event(server_fault_event).await;
        engine.submit_event(network_attack_event).await;

        // Process the first event
        let result = engine.process_next_event().await;
        assert!(result.is_ok());

        // Check that event was processed
        if let Ok(Some(event)) = result {
            assert!(matches!(event.status, EventStatus::Resolved));
        }
    }
}
