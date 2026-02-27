//! Unit tests for `mofa-kernel` plugin types
//!
//! Covers:
//! - [`PluginState`] equality and error variant behaviour
//! - [`PluginType`] variants including `Custom`
//! - [`PluginPriority`] ordering guarantees
//! - [`PluginMetadata`] builder methods
//! - [`PluginConfig`] typed getters/setters
//! - [`HotReloadConfig`] builder and defaults
//! - [`ReloadStrategy`] default value
#![allow(clippy::module_inception)]

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::plugin::{
        HotReloadConfig, PluginConfig, PluginMetadata, PluginPriority, PluginState, PluginType,
        ReloadStrategy,
    };

    // =========================================================================
    // PluginState
    // =========================================================================

    /// The canonical happy-path lifecycle:
    /// Unloaded → Loading → Loaded → Running → Paused → Running → Loaded → Unloaded
    ///
    /// Note: `PluginState` is a plain data enum — transition *enforcement* is the
    /// responsibility of the executor (AgentForge work-in-progress). These tests
    /// document the expected sequence and assert equality so that future transition
    /// guards can reference them.
    #[test]
    fn test_plugin_state_happy_path_sequence() {
        let sequence = vec![
            PluginState::Unloaded,
            PluginState::Loading,
            PluginState::Loaded,
            PluginState::Running,
            PluginState::Paused,
            PluginState::Running,  // resume
            PluginState::Loaded,   // stop → back to Loaded
            PluginState::Unloaded, // unload
        ];

        // Each state must compare equal to itself.
        for state in &sequence {
            assert_eq!(state, state, "PluginState must satisfy reflexive equality");
        }

        // Adjacent states in the lifecycle must be distinct.
        // (This catches accidental `PartialEq` implementations that collapse variants.)
        let distinct_pairs = [
            (&PluginState::Unloaded, &PluginState::Loading),
            (&PluginState::Loading, &PluginState::Loaded),
            (&PluginState::Loaded, &PluginState::Running),
            (&PluginState::Running, &PluginState::Paused),
        ];
        for (a, b) in &distinct_pairs {
            assert_ne!(a, b, "{a:?} and {b:?} must be distinct states");
        }
    }

    #[test]
    fn test_plugin_state_error_carries_message() {
        let msg = "connection timeout after 30 s".to_string();
        let state = PluginState::Error(msg.clone());

        // Error variant must equal another Error with the same message.
        assert_eq!(state, PluginState::Error(msg.clone()));

        // Error variant must NOT equal an Error with a different message.
        assert_ne!(state, PluginState::Error("different error".to_string()));

        // Error variant must NOT equal any non-error state.
        assert_ne!(state, PluginState::Running);
        assert_ne!(state, PluginState::Unloaded);
    }

    #[test]
    fn test_plugin_state_clone_round_trip() {
        let original = PluginState::Error("disk full".to_string());
        let cloned = original.clone();
        assert_eq!(original, cloned);
    }

    // =========================================================================
    // PluginType
    // =========================================================================

    #[test]
    fn test_plugin_type_wellknown_variants_are_distinct() {
        let types = vec![
            PluginType::LLM,
            PluginType::Tool,
            PluginType::Storage,
            PluginType::Memory,
            PluginType::VectorDB,
            PluginType::Communication,
            PluginType::Monitor,
            PluginType::Skill,
        ];

        // Each variant must equal itself.
        for t in &types {
            assert_eq!(t, t);
        }

        // No two different well-known variants may compare equal.
        for (i, a) in types.iter().enumerate() {
            for (j, b) in types.iter().enumerate() {
                if i != j {
                    assert_ne!(a, b, "{a:?} must differ from {b:?}");
                }
            }
        }
    }

    #[test]
    fn test_plugin_type_custom_equality() {
        let scraper = PluginType::Custom("scraper".to_string());
        let scraper2 = PluginType::Custom("scraper".to_string());
        let summariser = PluginType::Custom("summariser".to_string());

        assert_eq!(scraper, scraper2, "Same Custom label must be equal");
        assert_ne!(scraper, summariser, "Different Custom labels must differ");
        assert_ne!(
            scraper,
            PluginType::LLM,
            "Custom must not equal a well-known variant"
        );
    }

    #[test]
    fn test_plugin_type_custom_clone() {
        let original = PluginType::Custom("notify".to_string());
        assert_eq!(original.clone(), original);
    }

    // =========================================================================
    // PluginPriority
    // =========================================================================

    #[test]
    fn test_plugin_priority_ordering() {
        // Critical > High > Normal > Low
        assert!(PluginPriority::Critical > PluginPriority::High);
        assert!(PluginPriority::High > PluginPriority::Normal);
        assert!(PluginPriority::Normal > PluginPriority::Low);
    }

    #[test]
    fn test_plugin_priority_default_is_normal() {
        assert_eq!(PluginPriority::default(), PluginPriority::Normal);
    }

    #[test]
    fn test_plugin_priority_copy() {
        let p = PluginPriority::High;
        let q = p; // Copy — must not move
        assert_eq!(p, q);
    }

    // =========================================================================
    // PluginMetadata
    // =========================================================================

    #[test]
    fn test_plugin_metadata_new_sets_defaults() {
        let meta = PluginMetadata::new("agent-001", "My Agent", PluginType::Tool);

        assert_eq!(meta.id, "agent-001");
        assert_eq!(meta.name, "My Agent");
        assert_eq!(meta.plugin_type, PluginType::Tool);
        assert_eq!(meta.version, "1.0.0", "default version must be 1.0.0");
        assert!(meta.description.is_empty(), "default description must be empty");
        assert_eq!(meta.priority, PluginPriority::Normal);
        assert!(meta.dependencies.is_empty());
        assert!(meta.capabilities.is_empty());
        assert!(meta.author.is_none());
    }

    #[test]
    fn test_plugin_metadata_builder_chain() {
        let meta = PluginMetadata::new("rag-001", "RAG Plugin", PluginType::VectorDB)
            .with_version("2.1.0")
            .with_description("Retrieval-Augmented Generation plugin")
            .with_priority(PluginPriority::High)
            .with_dependency("embed-001")
            .with_dependency("store-001")
            .with_capability("semantic-search")
            .with_capability("chunk-retrieval");

        assert_eq!(meta.version, "2.1.0");
        assert_eq!(meta.description, "Retrieval-Augmented Generation plugin");
        assert_eq!(meta.priority, PluginPriority::High);
        assert_eq!(meta.dependencies, vec!["embed-001", "store-001"]);
        assert_eq!(meta.capabilities, vec!["semantic-search", "chunk-retrieval"]);
    }

    #[test]
    fn test_plugin_metadata_multiple_dependencies() {
        let meta = PluginMetadata::new("orchestrator", "Orchestrator", PluginType::Skill)
            .with_dependency("dep-a")
            .with_dependency("dep-b")
            .with_dependency("dep-c");

        assert_eq!(meta.dependencies.len(), 3);
        assert!(meta.dependencies.contains(&"dep-a".to_string()));
        assert!(meta.dependencies.contains(&"dep-c".to_string()));
    }

    // =========================================================================
    // PluginConfig
    // =========================================================================

    #[test]
    fn test_plugin_config_new_defaults() {
        let cfg = PluginConfig::new();
        assert!(cfg.enabled, "new config must be enabled by default");
        assert!(cfg.auto_start, "new config must auto-start by default");
        assert!(cfg.settings.is_empty());
    }

    #[test]
    fn test_plugin_config_set_and_get_typed_values() {
        let mut cfg = PluginConfig::new();

        cfg.set("endpoint", "http://localhost:8080");
        cfg.set("timeout_ms", 5000_i64);
        cfg.set("debug", true);

        assert_eq!(
            cfg.get_string("endpoint"),
            Some("http://localhost:8080".to_string())
        );
        assert_eq!(cfg.get_i64("timeout_ms"), Some(5000));
        assert_eq!(cfg.get_bool("debug"), Some(true));
    }

    #[test]
    fn test_plugin_config_missing_key_returns_none() {
        let cfg = PluginConfig::new();
        assert!(cfg.get_string("nonexistent").is_none());
        assert!(cfg.get_bool("nonexistent").is_none());
        assert!(cfg.get_i64("nonexistent").is_none());
    }

    #[test]
    fn test_plugin_config_overwrite_existing_key() {
        let mut cfg = PluginConfig::new();
        cfg.set("model", "gpt-3.5-turbo");
        cfg.set("model", "gpt-4o"); // overwrite
        assert_eq!(cfg.get_string("model"), Some("gpt-4o".to_string()));
    }

    // =========================================================================
    // HotReloadConfig
    // =========================================================================

    #[test]
    fn test_hot_reload_config_defaults() {
        let cfg = HotReloadConfig::default();
        assert_eq!(
            cfg.strategy,
            ReloadStrategy::Debounced(Duration::from_secs(1))
        );
        assert!(cfg.preserve_state);
        assert!(cfg.auto_rollback);
        assert_eq!(cfg.max_reload_attempts, 3);
        assert_eq!(cfg.reload_cooldown, Duration::from_secs(5));
    }

    #[test]
    fn test_hot_reload_config_builder() {
        let cfg = HotReloadConfig::new()
            .with_strategy(ReloadStrategy::Immediate)
            .with_preserve_state(false)
            .with_auto_rollback(false)
            .with_max_attempts(5)
            .with_reload_cooldown(Duration::from_secs(10));

        assert_eq!(cfg.strategy, ReloadStrategy::Immediate);
        assert!(!cfg.preserve_state);
        assert!(!cfg.auto_rollback);
        assert_eq!(cfg.max_reload_attempts, 5);
        assert_eq!(cfg.reload_cooldown, Duration::from_secs(10));
    }

    // =========================================================================
    // ReloadStrategy
    // =========================================================================

    #[test]
    fn test_reload_strategy_default_is_debounced_one_second() {
        assert_eq!(
            ReloadStrategy::default(),
            ReloadStrategy::Debounced(Duration::from_secs(1))
        );
    }

    #[test]
    fn test_reload_strategy_variants_are_distinct() {
        let a = ReloadStrategy::Immediate;
        let b = ReloadStrategy::Manual;
        let c = ReloadStrategy::OnIdle;
        let d = ReloadStrategy::Debounced(Duration::from_millis(500));
        let e = ReloadStrategy::Debounced(Duration::from_secs(2));

        assert_ne!(a, b);
        assert_ne!(b, c);
        assert_ne!(a, c);
        assert_ne!(d, e, "Different debounce durations must differ");
        assert_ne!(a, d, "Immediate must differ from Debounced");
    }
}
