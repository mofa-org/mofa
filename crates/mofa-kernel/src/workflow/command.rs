//! Command Pattern for Workflow Control
//!
//! Provides a unified way to update state and control workflow execution flow
//! from within node functions. Inspired by LangGraph's Command pattern.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::StateUpdate;

/// Control flow directive for workflow execution
///
/// Determines what happens after a node completes execution.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub enum ControlFlow {
    /// Jump to a specific node by ID
    Goto(String),

    /// Continue to the next node(s) based on graph edges
    #[default]
    Continue,

    /// End workflow execution and return current state
    Return,

    /// Dynamically create parallel execution branches (MapReduce pattern)
    Send(Vec<SendCommand>),
}

/// Command returned by node functions
///
/// A Command encapsulates both state updates and control flow decisions.
/// This allows nodes to update state AND determine where to go next in a
/// single return value.
///
/// # Example
///
/// ```rust,ignore
/// // Update state and continue to next node
/// let cmd = Command::new()
///     .update("result", json!("processed"))
///     .continue_();
///
/// // Update state and jump to specific node
/// let cmd = Command::new()
///     .update("classification", json!("type_a"))
/// .goto("handle_a");
///
/// // End execution with final state
/// let cmd = Command::new()
///     .update("final_result", json!("done"))
///     .return_();
///
/// // Create parallel branches for MapReduce
/// let cmd = Command::send(vec![
///     SendCommand::new("process", json!({"item": 1})),
///     SendCommand::new("process", json!({"item": 2})),
/// ]);
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Command {
    /// State updates to apply
    pub updates: Vec<StateUpdate>,
    /// Control flow directive
    pub control: ControlFlow,
}

impl Command {
    /// Create a new empty command
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a state update
    pub fn update(mut self, key: impl Into<String>, value: Value) -> Self {
        self.updates.push(StateUpdate::new(key, value));
        self
    }

    /// Add multiple state updates
    pub fn updates(mut self, updates: Vec<StateUpdate>) -> Self {
        self.updates.extend(updates);
        self
    }

    /// Set control flow to continue to next node
    pub fn continue_(mut self) -> Self {
        self.control = ControlFlow::Continue;
        self
    }

    /// Set control flow to jump to a specific node
    pub fn goto(mut self, node: impl Into<String>) -> Self {
        self.control = ControlFlow::Goto(node.into());
        self
    }

    /// Set control flow to end execution
    pub fn return_(mut self) -> Self {
        self.control = ControlFlow::Return;
        self
    }

    /// Set control flow to create parallel branches (MapReduce)
    pub fn send(targets: Vec<SendCommand>) -> Self {
        Self {
            updates: Vec::new(),
            control: ControlFlow::Send(targets),
        }
    }

    /// Create a command that just updates state (continues by default)
    pub fn just_update(key: impl Into<String>, value: Value) -> Self {
        Self::new().update(key, value)
    }

    /// Create a command that just controls flow (no state update)
    pub fn just_goto(node: impl Into<String>) -> Self {
        Self::new().goto(node)
    }

    /// Create a command that ends execution
    pub fn just_return() -> Self {
        Self::new().return_()
    }

    /// Check if this command ends execution
    pub fn is_return(&self) -> bool {
        matches!(self.control, ControlFlow::Return)
    }

    /// Check if this command creates parallel branches
    pub fn is_send(&self) -> bool {
        matches!(self.control, ControlFlow::Send(_))
    }

    /// Get the target node if this is a goto command
    pub fn goto_target(&self) -> Option<&str> {
        match &self.control {
            ControlFlow::Goto(target) => Some(target),
            _ => None,
        }
    }
}

/// Send command for MapReduce pattern
///
/// Represents a dynamic edge creation - sending execution to a target node
/// with specific input state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SendCommand {
    /// Target node ID
    pub target: String,
    /// Input state for this branch
    pub input: Value,
    /// Optional branch identifier
    pub branch_id: Option<String>,
}

impl SendCommand {
    /// Create a new send command
    pub fn new(target: impl Into<String>, input: Value) -> Self {
        Self {
            target: target.into(),
            input,
            branch_id: None,
        }
    }

    /// Create a send command with a branch ID
    pub fn with_branch(
        target: impl Into<String>,
        input: Value,
        branch_id: impl Into<String>,
    ) -> Self {
        Self {
            target: target.into(),
            input,
            branch_id: Some(branch_id.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_command_builder() {
        let cmd = Command::new()
            .update("key1", json!("value1"))
            .update("key2", json!(42))
            .goto("next_node");

        assert_eq!(cmd.updates.len(), 2);
        assert_eq!(cmd.updates[0].key, "key1");
        assert_eq!(cmd.goto_target(), Some("next_node"));
    }

    #[test]
    fn test_command_continue() {
        let cmd = Command::new().update("result", json!("done")).continue_();

        assert_eq!(cmd.control, ControlFlow::Continue);
        assert!(!cmd.is_return());
    }

    #[test]
    fn test_command_return() {
        let cmd = Command::new().update("final", json!("result")).return_();

        assert!(cmd.is_return());
    }

    #[test]
    fn test_command_send() {
        let cmd = Command::send(vec![
            SendCommand::new("worker", json!({"task": 1})),
            SendCommand::new("worker", json!({"task": 2})),
        ]);

        assert!(cmd.is_send());
        if let ControlFlow::Send(targets) = &cmd.control {
            assert_eq!(targets.len(), 2);
        } else {
            panic!("Expected Send control flow");
        }
    }

    #[test]
    fn test_send_command() {
        let send = SendCommand::new("process", json!({"data": "test"}));
        assert_eq!(send.target, "process");
        assert!(send.branch_id.is_none());

        let send_with_branch =
            SendCommand::with_branch("process", json!({"data": "test"}), "branch-1");
        assert_eq!(send_with_branch.branch_id, Some("branch-1".to_string()));
    }

    #[test]
    fn test_just_helpers() {
        let cmd = Command::just_update("key", json!("value"));
        assert_eq!(cmd.updates.len(), 1);
        assert_eq!(cmd.control, ControlFlow::Continue);

        let cmd = Command::just_goto("target");
        assert!(cmd.updates.is_empty());
        assert_eq!(cmd.goto_target(), Some("target"));

        let cmd = Command::just_return();
        assert!(cmd.is_return());
    }
}
