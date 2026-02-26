//! Monitoring and event response system
//!
//! This module provides a comprehensive event-driven system for intelligent
//! operation and maintenance, including event definitions, response plugins,
//! and a runtime rule adjustment mechanism.

pub mod engine;
pub mod event;
pub mod plugin;
pub mod plugins;
pub mod rule_manager;

pub use engine::*;
// Re-export key components
pub use event::*;
pub use plugin::*;
pub use plugins::*;
pub use rule_manager::*;

#[cfg(test)]
mod tests {
    #[test]
    fn test_module_structure() {
        // Just verify the module structure compiles
    }
}
