//! Deterministic Workflow Trace Engine
//!
//! This module provides the trace capture infrastructure for deterministic workflow replay.
//! It enables capturing execution traces that can later be used for deterministic replay,
//! debugging, and regression testing.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    Trace Capture Pipeline                   │
//! ├─────────────────────────────────────────────────────────────┤
//! │                                                              │
//! │  WorkflowExecutor ──trace──▶ WorkflowTrace                  │
//! │         │                      │                            │
//! │         │                      ▼                            │
//! │         │              TraceEvent (serializable)            │
//! │         │                      │                            │
//! │         │                      ▼                            │
//! │         │              JSON serialization                   │
//! │         │                      │                            │
//! │         │                      ▼                            │
//! │         │              File / Storage                        │
//! │                                                              │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Usage
//!
//! ```rust,ignore
//! use mofa_foundation::workflow::trace::{WorkflowTrace, TraceMode};
//!
//! // Enable trace recording
//! let trace = WorkflowTrace::new("workflow-1", "exec-1");
//!
//! // Configure executor to use trace
//! let executor = WorkflowExecutor::new(config)
//!     .with_trace_mode(TraceMode::Record(trace));
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Trace mode configuration
#[derive(Debug, Clone)]
pub enum TraceMode {
    /// No tracing - execution proceeds normally without any overhead
    Disabled,
    /// Record mode - captures all execution events for later replay
    Record(Arc<WorkflowTraceHandle>),
    /// Replay mode - executes workflow from recorded trace without external side effects
    Replay(Arc<WorkflowTraceHandle>),
}

impl TraceMode {
    /// Check if tracing is enabled
    pub fn is_enabled(&self) -> bool {
        matches!(self, TraceMode::Record(_) | TraceMode::Replay(_))
    }

    /// Check if in replay mode
    pub fn is_replay(&self) -> bool {
        matches!(self, TraceMode::Replay(_))
    }

    /// Get the trace handle if in record or replay mode
    pub fn trace(&self) -> Option<&WorkflowTraceHandle> {
        match self {
            TraceMode::Record(handle) | TraceMode::Replay(handle) => Some(handle),
            TraceMode::Disabled => None,
        }
    }
}

/// Tool invocation record - captures input and output of tool calls
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInvocation {
    /// Unique identifier for this tool call
    pub id: String,
    /// Tool name or identifier
    pub tool_name: String,
    /// Input parameters passed to the tool
    pub input: serde_json::Value,
    /// Output returned by the tool
    pub output: Option<serde_json::Value>,
    /// Error message if tool call failed
    pub error: Option<String>,
    /// Whether the tool call was successful
    pub success: bool,
    /// Execution duration in milliseconds
    pub duration_ms: u64,
}

/// Retry attempt record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryAttempt {
    /// Attempt number (1-indexed)
    pub attempt: u32,
    /// Timestamp when retry was initiated
    pub timestamp_ms: u64,
    /// Reason for retry (error message)
    pub reason: String,
}

/// State update record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceStateUpdate {
    /// Key that was updated
    pub key: String,
    /// Previous value (None if new key)
    pub old_value: Option<serde_json::Value>,
    /// New value
    pub new_value: serde_json::Value,
    /// Timestamp of update
    pub timestamp_ms: u64,
}

/// Execution status transition
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ExecutionStatus {
    /// Workflow/Node has started
    Started,
    /// Running currently
    Running,
    /// Completed successfully
    Completed,
    /// Failed with error
    Failed(String),
    /// Was cancelled
    Cancelled,
}

/// Node execution record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeExecutionRecord {
    /// Node identifier
    pub node_id: String,
    /// Node type
    pub node_type: String,
    /// Order of execution in the workflow
    pub execution_order: usize,
    /// When node started executing
    pub started_at_ms: u64,
    /// When node finished executing
    pub ended_at_ms: Option<u64>,
    /// Duration in milliseconds
    pub duration_ms: Option<u64>,
    /// Current status
    pub status: ExecutionStatus,
    /// Input to the node
    pub input: Option<serde_json::Value>,
    /// Output from the node
    pub output: Option<serde_json::Value>,
    /// Tool invocations made by this node
    #[serde(default)]
    pub tool_invocations: Vec<ToolInvocation>,
    /// Retry attempts for this node
    #[serde(default)]
    pub retry_attempts: Vec<RetryAttempt>,
    /// State updates performed by this node
    #[serde(default)]
    pub state_updates: Vec<TraceStateUpdate>,
}

/// Workflow trace - captures complete execution for deterministic replay
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowTrace {
    /// Unique trace identifier
    pub id: String,
    /// Workflow graph identifier
    pub workflow_id: String,
    /// Execution identifier
    pub execution_id: String,
    /// When trace was started
    pub started_at_ms: u64,
    /// When trace ended (None if still running)
    pub ended_at_ms: Option<u64>,
    /// Final execution status
    pub status: ExecutionStatus,
    /// Node execution records in order
    #[serde(default)]
    pub node_executions: Vec<NodeExecutionRecord>,
    /// Total tool invocations across all nodes
    #[serde(default)]
    pub total_tool_invocations: usize,
    /// Total retry attempts across all nodes
    #[serde(default)]
    pub total_retry_attempts: usize,
}

impl WorkflowTrace {
    /// Create a new workflow trace
    pub fn new(workflow_id: impl Into<String>, execution_id: impl Into<String>) -> Self {
        let exec_id = execution_id.into();
        let id = format!("trace-{}-{}", exec_id, uuid_v4());
        Self {
            id,
            workflow_id: workflow_id.into(),
            execution_id: exec_id,
            started_at_ms: current_time_ms(),
            ended_at_ms: None,
            status: ExecutionStatus::Started,
            node_executions: Vec::new(),
            total_tool_invocations: 0,
            total_retry_attempts: 0,
        }
    }

    /// Record a node start event
    pub fn record_node_start(
        &self,
        node_id: &str,
        node_type: &str,
        input: Option<serde_json::Value>,
    ) -> usize {
        // This is a lightweight operation - actual mutation happens in a managed copy
        0
    }

    /// Create a node execution record for a starting node
    pub fn start_node(
        &mut self,
        node_id: String,
        node_type: String,
        input: Option<serde_json::Value>,
    ) -> usize {
        let execution_order = self.node_executions.len();
        let record = NodeExecutionRecord {
            node_id,
            node_type,
            execution_order,
            started_at_ms: current_time_ms(),
            ended_at_ms: None,
            duration_ms: None,
            status: ExecutionStatus::Running,
            input,
            output: None,
            tool_invocations: Vec::new(),
            retry_attempts: Vec::new(),
            state_updates: Vec::new(),
        };
        self.node_executions.push(record);
        execution_order
    }

    /// Complete a node execution
    pub fn complete_node(
        &mut self,
        execution_order: usize,
        output: Option<serde_json::Value>,
        status: ExecutionStatus,
    ) {
        if let Some(record) = self.node_executions.get_mut(execution_order) {
            let ended_at = current_time_ms();
            record.ended_at_ms = Some(ended_at);
            record.duration_ms = Some(ended_at.saturating_sub(record.started_at_ms));
            record.output = output;
            record.status = status;

            // Update totals
            self.total_tool_invocations += record.tool_invocations.len();
            self.total_retry_attempts += record.retry_attempts.len();
        }
    }

    /// Record a tool invocation for a node
    pub fn record_tool_invocation(
        &mut self,
        execution_order: usize,
        invocation: ToolInvocation,
    ) {
        if let Some(record) = self.node_executions.get_mut(execution_order) {
            record.tool_invocations.push(invocation);
        }
    }

    /// Record a retry attempt for a node
    pub fn record_retry_attempt(
        &mut self,
        execution_order: usize,
        attempt: RetryAttempt,
    ) {
        if let Some(record) = self.node_executions.get_mut(execution_order) {
            record.retry_attempts.push(attempt);
        }
    }

    /// Record a state update for a node
    pub fn record_state_update(
        &mut self,
        execution_order: usize,
        update: TraceStateUpdate,
    ) {
        if let Some(record) = self.node_executions.get_mut(execution_order) {
            record.state_updates.push(update);
        }
    }

    /// Mark the trace as complete
    pub fn finish(&mut self, status: ExecutionStatus) {
        self.ended_at_ms = Some(current_time_ms());
        self.status = status;
    }

    /// Serialize the trace to JSON
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Deserialize a trace from JSON
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

/// Thread-safe wrapper for WorkflowTrace
#[derive(Debug)]
pub struct WorkflowTraceHandle {
    trace: Arc<RwLock<WorkflowTrace>>,
}

impl WorkflowTraceHandle {
    /// Create a new trace handle
    pub fn new(workflow_id: impl Into<String>, execution_id: impl Into<String>) -> Self {
        Self {
            trace: Arc::new(RwLock::new(WorkflowTrace::new(workflow_id, execution_id))),
        }
    }

    /// Get an owned clone of the trace
    pub async fn get_trace(&self) -> WorkflowTrace {
        self.trace.read().await.clone()
    }

    /// Start a node execution and return the execution order
    pub async fn start_node(
        &self,
        node_id: String,
        node_type: String,
        input: Option<serde_json::Value>,
    ) -> usize {
        self.trace.write().await.start_node(node_id, node_type, input)
    }

    /// Complete a node execution
    pub async fn complete_node(
        &self,
        execution_order: usize,
        output: Option<serde_json::Value>,
        status: ExecutionStatus,
    ) {
        self.trace.write().await.complete_node(execution_order, output, status);
    }

    /// Record a tool invocation
    pub async fn record_tool_invocation(&self, execution_order: usize, invocation: ToolInvocation) {
        self.trace.write().await.record_tool_invocation(execution_order, invocation);
    }

    /// Record a retry attempt
    pub async fn record_retry_attempt(&self, execution_order: usize, attempt: RetryAttempt) {
        self.trace.write().await.record_retry_attempt(execution_order, attempt);
    }

    /// Record a state update
    pub async fn record_state_update(&self, execution_order: usize, update: TraceStateUpdate) {
        self.trace.write().await.record_state_update(execution_order, update);
    }

    /// Finish the trace
    pub async fn finish(&self, status: ExecutionStatus) {
        self.trace.write().await.finish(status);
    }

    /// Serialize to JSON
    pub async fn to_json(&self) -> Result<String, serde_json::Error> {
        self.trace.read().await.to_json()
    }
}

// ============================================================================
// Replay Mismatch Error
// ============================================================================

/// Types of mismatch that can occur during replay
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReplayMismatchType {
    /// Node execution order differs
    NodeOrderMismatch {
        expected: String,
        actual: String,
    },
    /// Node input differs from recorded
    InputMismatch {
        node_id: String,
        expected: serde_json::Value,
        actual: serde_json::Value,
    },
    /// Node output differs from recorded
    OutputMismatch {
        node_id: String,
        expected: serde_json::Value,
        actual: serde_json::Value,
    },
    /// State transition differs from recorded
    StateMismatch {
        node_id: String,
        key: String,
        expected: serde_json::Value,
        actual: serde_json::Value,
    },
    /// Execution status differs from recorded
    StatusMismatch {
        node_id: String,
        expected: ExecutionStatus,
        actual: ExecutionStatus,
    },
    /// Attempted to execute more nodes than recorded
    ExcessExecution {
        node_id: String,
    },
    /// Missing expected node execution
    MissingExecution {
        node_id: String,
    },
}

/// Error emitted when replay diverges from recorded trace
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayMismatch {
    /// Type of mismatch
    pub mismatch_type: ReplayMismatchType,
    /// Timestamp when mismatch was detected
    pub timestamp_ms: u64,
    /// Whether this is a fatal mismatch (execution cannot continue)
    pub fatal: bool,
}

impl ReplayMismatch {
    /// Create a new replay mismatch
    pub fn new(mismatch_type: ReplayMismatchType, fatal: bool) -> Self {
        Self {
            mismatch_type,
            timestamp_ms: current_time_ms(),
            fatal,
        }
    }

    /// Create a fatal mismatch
    pub fn fatal(mismatch_type: ReplayMismatchType) -> Self {
        Self::new(mismatch_type, true)
    }

    /// Create a non-fatal mismatch (warning)
    pub fn warning(mismatch_type: ReplayMismatchType) -> Self {
        Self::new(mismatch_type, false)
    }
}

// ============================================================================
// Replay State Machine
// ============================================================================

/// State machine for tracking replay progress
#[derive(Debug)]
pub struct ReplayState {
    /// The trace being replayed
    trace: Arc<WorkflowTrace>,
    /// Current node execution index
    current_index: usize,
    /// Mismatches detected during replay
    mismatches: Vec<ReplayMismatch>,
    /// Whether replay has completed
    completed: bool,
}

impl ReplayState {
    /// Create a new replay state from a trace
    pub fn new(trace: WorkflowTrace) -> Self {
        Self {
            trace: Arc::new(trace),
            current_index: 0,
            mismatches: Vec::new(),
            completed: false,
        }
    }

    /// Get the expected next node execution
    pub fn next_expected_node(&self) -> Option<&NodeExecutionRecord> {
        if self.current_index < self.trace.node_executions.len() {
            self.trace.node_executions.get(self.current_index)
        } else {
            None
        }
    }

    /// Validate and advance to next node
    pub fn validate_node_start(&mut self, node_id: &str, input: &serde_json::Value) -> Result<(), ReplayMismatch> {
        if let Some(expected) = self.next_expected_node() {
            if expected.node_id != node_id {
                let mismatch = ReplayMismatch::fatal(ReplayMismatchType::NodeOrderMismatch {
                    expected: expected.node_id.clone(),
                    actual: node_id.to_string(),
                });
                self.mismatches.push(mismatch.clone());
                return Err(mismatch);
            }

            // Check input matches
            if let Some(ref exp_input) = expected.input {
                if exp_input != input {
                    let mismatch = ReplayMismatch::warning(ReplayMismatchType::InputMismatch {
                        node_id: node_id.to_string(),
                        expected: exp_input.clone(),
                        actual: input.clone(),
                    });
                    self.mismatches.push(mismatch);
                }
            }

            Ok(())
        } else {
            let mismatch = ReplayMismatch::fatal(ReplayMismatchType::ExcessExecution {
                node_id: node_id.to_string(),
            });
            self.mismatches.push(mismatch.clone());
            Err(mismatch)
        }
    }

    /// Validate node completion
    pub fn validate_node_end(
        &mut self,
        output: &serde_json::Value,
        status: &ExecutionStatus,
    ) -> Result<(), ReplayMismatch> {
        if let Some(expected) = self.next_expected_node() {
            // Extract needed data from expected to avoid borrow issues
            let exp_output = expected.output.clone();
            let node_id = expected.node_id.clone();
            let exp_status = expected.status.clone();
            
            // Check output matches
            if let Some(ref exp_out) = exp_output {
                if exp_out != output {
                    let mismatch = ReplayMismatch::warning(ReplayMismatchType::OutputMismatch {
                        node_id: node_id.clone(),
                        expected: exp_out.clone(),
                        actual: output.clone(),
                    });
                    self.mismatches.push(mismatch);
                }
            }

            // Check status matches
            if exp_status != *status {
                let mismatch = ReplayMismatch::warning(ReplayMismatchType::StatusMismatch {
                    node_id,
                    expected: exp_status,
                    actual: status.clone(),
                });
                self.mismatches.push(mismatch);
            }

            // Advance to next node
            self.current_index += 1;
            Ok(())
        } else {
            Ok(()) // Already reported error in validate_node_start
        }
    }

    /// Check if replay completed successfully (no fatal mismatches)
    pub fn finish(&mut self) -> Result<(), Vec<ReplayMismatch>> {
        self.completed = true;
        
        // Check for missing executions
        if self.current_index < self.trace.node_executions.len() {
            for remaining in self.trace.node_executions.iter().skip(self.current_index) {
                self.mismatches.push(ReplayMismatch::warning(ReplayMismatchType::MissingExecution {
                    node_id: remaining.node_id.clone(),
                }));
            }
        }

        // Return fatal errors if any
        let fatal: Vec<_> = self.mismatches.iter()
            .filter(|m| m.fatal)
            .cloned()
            .collect();

        if fatal.is_empty() {
            Ok(())
        } else {
            Err(fatal)
        }
    }

    /// Get all mismatches (for debugging/analysis)
    pub fn mismatches(&self) -> &[ReplayMismatch] {
        &self.mismatches
    }

    /// Get the recorded output for current node (for replay)
    pub fn get_replayed_output(&self) -> Option<&serde_json::Value> {
        self.next_expected_node()
            .and_then(|n| n.output.as_ref())
    }

    /// Check if more nodes are available
    pub fn has_more_nodes(&self) -> bool {
        self.current_index < self.trace.node_executions.len()
    }

    /// Get progress info
    pub fn progress(&self) -> (usize, usize) {
        (self.current_index, self.trace.node_executions.len())
    }
}

// ============================================================================
// Snapshot Consistency Validation
// ============================================================================

/// Result of comparing two traces for consistency
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TraceComparisonResult {
    pub identical: bool,
    pub node_count_match: bool,
    pub order_match: bool,
    pub output_differences: Vec<OutputDifference>,
    pub status_differences: Vec<StatusDifference>,
}

/// Represents a difference in node outputs
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OutputDifference {
    pub node_id: String,
    pub expected_output: serde_json::Value,
    pub actual_output: serde_json::Value,
}

/// Represents a difference in node status
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StatusDifference {
    pub node_id: String,
    pub expected_status: ExecutionStatus,
    pub actual_status: ExecutionStatus,
}

impl WorkflowTrace {
    /// Compare this trace with another for consistency
    /// Used for snapshot testing and regression validation
    pub fn compare(&self, other: &WorkflowTrace) -> TraceComparisonResult {
        let node_count_match = self.node_executions.len() == other.node_executions.len();
        
        // Check order match
        let order_match = self.node_executions.iter()
            .zip(other.node_executions.iter())
            .all(|(a, b)| a.node_id == b.node_id);
        
        // Check output differences
        let output_differences: Vec<OutputDifference> = self.node_executions.iter()
            .zip(other.node_executions.iter())
            .filter_map(|(a, b)| {
                match (&a.output, &b.output) {
                    (Some(exp), Some(act)) if exp != act => Some(OutputDifference {
                        node_id: a.node_id.clone(),
                        expected_output: exp.clone(),
                        actual_output: act.clone(),
                    }),
                    _ => None,
                }
            })
            .collect();
        
        // Check status differences
        let status_differences: Vec<StatusDifference> = self.node_executions.iter()
            .zip(other.node_executions.iter())
            .filter(|(a, b)| a.status != b.status)
            .map(|(a, b)| StatusDifference {
                node_id: a.node_id.clone(),
                expected_status: a.status.clone(),
                actual_status: b.status.clone(),
            })
            .collect();
        
        let identical = node_count_match && order_match 
            && output_differences.is_empty() 
            && status_differences.is_empty();
        
        TraceComparisonResult {
            identical,
            node_count_match,
            order_match,
            output_differences,
            status_differences,
        }
    }
    
    /// Validate that this trace matches expected snapshot
    pub fn validate_snapshot(&self, expected: &WorkflowTrace) -> Result<(), TraceComparisonResult> {
        let result = self.compare(expected);
        if result.identical {
            Ok(())
        } else {
            Err(result)
        }
    }
    
    /// Get trace fingerprint for quick comparison
    pub fn fingerprint(&self) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        self.workflow_id.hash(&mut hasher);
        self.execution_id.hash(&mut hasher);
        self.node_executions.len().hash(&mut hasher);
        
        for node in &self.node_executions {
            node.node_id.hash(&mut hasher);
            if let Some(ref output) = node.output {
                output.hash(&mut hasher);
            }
            node.status.hash(&mut hasher);
        }
        
        format!("{:016x}", hasher.finish())
    }
}

// ============================================================================
// Helper functions
// ============================================================================

/// Get current time in milliseconds
fn current_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Generate a simple UUID v4-like string
fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!(
        "{:08x}-{:04x}-4{:03x}-{:04x}-{:012x}",
        now.as_secs() as u32,
        (now.as_nanos() >> 16) as u16 & 0x0FFF,
        (now.as_nanos() >> 12) as u16 & 0x0FFF,
        (rand_u16() & 0x3FFF) | 0x8000,
        now.as_nanos() as u64 & 0xFFFFFFFFFFFF
    )
}

/// Generate a random u16
fn rand_u16() -> u16 {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    RandomState::new().build_hasher().finish() as u16
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trace_creation() {
        let trace = WorkflowTrace::new("workflow-1", "exec-1");
        assert_eq!(trace.workflow_id, "workflow-1");
        assert_eq!(trace.execution_id, "exec-1");
        assert!(trace.ended_at_ms.is_none());
        assert!(matches!(trace.status, ExecutionStatus::Started));
    }

    #[test]
    fn test_trace_node_lifecycle() {
        let mut trace = WorkflowTrace::new("workflow-1", "exec-1");

        let order = trace.start_node(
            "node-1".to_string(),
            "task".to_string(),
            Some(serde_json::json!({"input": "test"})),
        );
        assert_eq!(order, 0);

        trace.complete_node(
            order,
            Some(serde_json::json!({"result": "success"})),
            ExecutionStatus::Completed,
        );

        assert_eq!(trace.node_executions.len(), 1);
        let node = &trace.node_executions[0];
        assert_eq!(node.node_id, "node-1");
        assert!(node.ended_at_ms.is_some());
        assert!(node.duration_ms.is_some());
    }

    #[test]
    fn test_trace_tool_invocation() {
        let mut trace = WorkflowTrace::new("workflow-1", "exec-1");

        let order = trace.start_node(
            "node-1".to_string(),
            "task".to_string(),
            None,
        );

        trace.record_tool_invocation(
            order,
            ToolInvocation {
                id: "tool-1".to_string(),
                tool_name: "search".to_string(),
                input: serde_json::json!({"query": "test"}),
                output: Some(serde_json::json!({"results": []})),
                error: None,
                success: true,
                duration_ms: 100,
            },
        );

        assert_eq!(trace.node_executions[0].tool_invocations.len(), 1);
    }

    #[test]
    fn test_trace_retry_attempts() {
        let mut trace = WorkflowTrace::new("workflow-1", "exec-1");

        let order = trace.start_node(
            "node-1".to_string(),
            "task".to_string(),
            None,
        );

        trace.record_retry_attempt(
            order,
            RetryAttempt {
                attempt: 1,
                timestamp_ms: 1000,
                reason: "Connection timeout".to_string(),
            },
        );

        assert_eq!(trace.node_executions[0].retry_attempts.len(), 1);
    }

    #[test]
    fn test_trace_serialization() {
        let mut trace = WorkflowTrace::new("workflow-1", "exec-1");
        trace.start_node("node-1".to_string(), "task".to_string(), None);
        trace.complete_node(0, None, ExecutionStatus::Completed);
        trace.finish(ExecutionStatus::Completed);

        let json = trace.to_json().unwrap();
        let round_trip: WorkflowTrace = WorkflowTrace::from_json(&json).unwrap();

        assert_eq!(round_trip.workflow_id, "workflow-1");
        assert_eq!(round_trip.node_executions.len(), 1);
    }

    #[test]
    fn test_trace_mode() {
        let handle = Arc::new(WorkflowTraceHandle::new("wf", "exec"));
        let mode = TraceMode::Record(handle.clone());

        assert!(mode.is_enabled());
        assert!(mode.trace().is_some());

        let disabled = TraceMode::Disabled;
        assert!(!disabled.is_enabled());
        assert!(disabled.trace().is_none());
    }

    #[test]
    fn test_trace_handle() {
        let handle = WorkflowTraceHandle::new("workflow-1", "exec-1");

        let order = futures::executor::block_on(async {
            handle.start_node(
                "node-1".to_string(),
                "task".to_string(),
                Some(serde_json::json!({"key": "value"})),
            ).await
        });

        assert_eq!(order, 0);

        futures::executor::block_on(async {
            handle.complete_node(
                order,
                Some(serde_json::json!({"result": "ok"})),
                ExecutionStatus::Completed,
            ).await;

            handle.finish(ExecutionStatus::Completed).await;

            let trace = handle.get_trace().await;
            assert_eq!(trace.node_executions.len(), 1);
        });
    }

    #[test]
    fn test_replay_state_creation() {
        // Create a trace with node executions
        let mut trace = WorkflowTrace::new("workflow-1", "exec-1");
        trace.start_node("node-1".to_string(), "task".to_string(), Some(serde_json::json!({"input": 1})));
        trace.complete_node(0, Some(serde_json::json!({"result": "output-1"})), ExecutionStatus::Completed);
        trace.start_node("node-2".to_string(), "task".to_string(), Some(serde_json::json!({"input": 2})));
        trace.complete_node(1, Some(serde_json::json!({"result": "output-2"})), ExecutionStatus::Completed);
        trace.finish(ExecutionStatus::Completed);

        // Create replay state
        let handle = Arc::new(WorkflowTraceHandle::new("workflow-1", "exec-1"));
        futures::executor::block_on(async {
            handle.get_trace().await;
        });
        
        // Just test the basic struct exists and compiles
        let _replay_state = ReplayState::new(trace);
    }

    #[test]
    fn test_replay_mismatch_types() {
        // Test output mismatch
        let mismatch = ReplayMismatch::warning(ReplayMismatchType::OutputMismatch {
            node_id: "node-1".to_string(),
            expected: serde_json::json!({"result": "expected"}),
            actual: serde_json::json!({"result": "actual"}),
        });
        assert!(!mismatch.fatal);

        // Test fatal mismatch
        let fatal_mismatch = ReplayMismatch::fatal(ReplayMismatchType::NodeOrderMismatch {
            expected: "node-1".to_string(),
            actual: "node-2".to_string(),
        });
        assert!(fatal_mismatch.fatal);
    }

    #[test]
    fn test_trace_mode_replay() {
        let handle = Arc::new(WorkflowTraceHandle::new("wf", "exec"));
        let mode = TraceMode::Replay(handle.clone());

        assert!(mode.is_enabled());
        assert!(mode.trace().is_some());
    }

    #[test]
    fn test_replay_state_validation() {
        // Create a trace with known executions
        let mut trace = WorkflowTrace::new("workflow-1", "exec-1");
        trace.start_node("node-1".to_string(), "task".to_string(), Some(serde_json::json!({"input": 1})));
        trace.complete_node(0, Some(serde_json::json!({"result": "output-1"})), ExecutionStatus::Completed);
        trace.finish(ExecutionStatus::Completed);
        
        let mut replay_state = ReplayState::new(trace);

        // Validate node start with correct node ID
        let result = replay_state.validate_node_start("node-1", &serde_json::json!({"input": 1}));
        assert!(result.is_ok());

        // Validate node end with correct output
        let result = replay_state.validate_node_end(
            &serde_json::json!({"result": "output-1"}),
            &ExecutionStatus::Completed,
        );
        assert!(result.is_ok());

        // Check progress
        let (current, total) = replay_state.progress();
        assert_eq!(current, 1);
        assert_eq!(total, 1); // One node in trace
    }

    #[test]
    fn test_replay_output_retrieval() {
        // Create a trace with node executions
        let mut trace = WorkflowTrace::new("workflow-1", "exec-1");
        trace.start_node("node-1".to_string(), "task".to_string(), Some(serde_json::json!({"input": 1})));
        trace.complete_node(0, Some(serde_json::json!({"result": "output-1"})), ExecutionStatus::Completed);
        trace.start_node("node-2".to_string(), "task".to_string(), Some(serde_json::json!({"input": 2})));
        trace.complete_node(1, Some(serde_json::json!({"result": "output-2"})), ExecutionStatus::Completed);
        trace.finish(ExecutionStatus::Completed);

        let mut replay_state = ReplayState::new(trace);

        // Get replayed output for first node
        let output = replay_state.get_replayed_output();
        assert!(output.is_some());
        assert_eq!(output.unwrap(), &serde_json::json!({"result": "output-1"}));

        // Advance to next node
        let _ = replay_state.validate_node_start("node-1", &serde_json::json!({}));
        let _ = replay_state.validate_node_end(&serde_json::json!({}), &ExecutionStatus::Completed);

        // Get replayed output for second node
        let output = replay_state.get_replayed_output();
        assert!(output.is_some());
        assert_eq!(output.unwrap(), &serde_json::json!({"result": "output-2"}));
    }

    // ============================================================================
    // Phase 3: Deterministic Validation Tests
    // ============================================================================

    #[test]
    fn test_trace_comparison_identical() {
        // Create two identical traces
        let mut trace1 = WorkflowTrace::new("workflow-1", "exec-1");
        trace1.start_node("node-1".to_string(), "task".to_string(), Some(serde_json::json!({"input": 1})));
        trace1.complete_node(0, Some(serde_json::json!({"result": "output-1"})), ExecutionStatus::Completed);
        trace1.finish(ExecutionStatus::Completed);

        let mut trace2 = WorkflowTrace::new("workflow-1", "exec-1");
        trace2.start_node("node-1".to_string(), "task".to_string(), Some(serde_json::json!({"input": 1})));
        trace2.complete_node(0, Some(serde_json::json!({"result": "output-1"})), ExecutionStatus::Completed);
        trace2.finish(ExecutionStatus::Completed);

        let result = trace1.compare(&trace2);
        assert!(result.identical);
        assert!(result.output_differences.is_empty());
        assert!(result.status_differences.is_empty());
    }

    #[test]
    fn test_trace_comparison_output_diff() {
        // Create two traces with different outputs
        let mut trace1 = WorkflowTrace::new("workflow-1", "exec-1");
        trace1.start_node("node-1".to_string(), "task".to_string(), None);
        trace1.complete_node(0, Some(serde_json::json!({"result": "output-a"})), ExecutionStatus::Completed);
        trace1.finish(ExecutionStatus::Completed);

        let mut trace2 = WorkflowTrace::new("workflow-1", "exec-1");
        trace2.start_node("node-1".to_string(), "task".to_string(), None);
        trace2.complete_node(0, Some(serde_json::json!({"result": "output-b"})), ExecutionStatus::Completed);
        trace2.finish(ExecutionStatus::Completed);

        let result = trace1.compare(&trace2);
        assert!(!result.identical);
        assert_eq!(result.output_differences.len(), 1);
        assert_eq!(result.output_differences[0].node_id, "node-1");
    }

    #[test]
    fn test_trace_comparison_order_diff() {
        // Create two traces with different node order
        let mut trace1 = WorkflowTrace::new("workflow-1", "exec-1");
        trace1.start_node("node-1".to_string(), "task".to_string(), None);
        trace1.complete_node(0, Some(serde_json::json!({})), ExecutionStatus::Completed);
        trace1.start_node("node-2".to_string(), "task".to_string(), None);
        trace1.complete_node(1, Some(serde_json::json!({})), ExecutionStatus::Completed);
        trace1.finish(ExecutionStatus::Completed);

        let mut trace2 = WorkflowTrace::new("workflow-1", "exec-1");
        trace2.start_node("node-2".to_string(), "task".to_string(), None);
        trace2.complete_node(0, Some(serde_json::json!({})), ExecutionStatus::Completed);
        trace2.start_node("node-1".to_string(), "task".to_string(), None);
        trace2.complete_node(1, Some(serde_json::json!({})), ExecutionStatus::Completed);
        trace2.finish(ExecutionStatus::Completed);

        let result = trace1.compare(&trace2);
        assert!(!result.identical);
        assert!(!result.order_match);
    }

    #[test]
    fn test_trace_snapshot_validation() {
        // Create expected snapshot
        let mut expected = WorkflowTrace::new("workflow-1", "exec-1");
        expected.start_node("node-1".to_string(), "task".to_string(), Some(serde_json::json!({"value": 42})));
        expected.complete_node(0, Some(serde_json::json!({"result": "success", "data": [1, 2, 3]})), ExecutionStatus::Completed);
        expected.finish(ExecutionStatus::Completed);

        // Create identical actual trace (e.g., from replay)
        let mut actual = WorkflowTrace::new("workflow-1", "exec-1");
        actual.start_node("node-1".to_string(), "task".to_string(), Some(serde_json::json!({"value": 42})));
        actual.complete_node(0, Some(serde_json::json!({"result": "success", "data": [1, 2, 3]})), ExecutionStatus::Completed);
        actual.finish(ExecutionStatus::Completed);

        // Validate snapshot - should pass
        let validation_result = actual.validate_snapshot(&expected);
        assert!(validation_result.is_ok());
    }

    #[test]
    fn test_trace_fingerprint() {
        let mut trace1 = WorkflowTrace::new("workflow-1", "exec-1");
        trace1.start_node("node-1".to_string(), "task".to_string(), None);
        trace1.complete_node(0, Some(serde_json::json!({})), ExecutionStatus::Completed);
        trace1.finish(ExecutionStatus::Completed);

        let fingerprint1 = trace1.fingerprint();
        
        // Same trace should have same fingerprint
        let mut trace2 = WorkflowTrace::new("workflow-1", "exec-1");
        trace2.start_node("node-1".to_string(), "task".to_string(), None);
        trace2.complete_node(0, Some(serde_json::json!({})), ExecutionStatus::Completed);
        trace2.finish(ExecutionStatus::Completed);

        let fingerprint2 = trace2.fingerprint();
        assert_eq!(fingerprint1, fingerprint2);

        // Different trace should have different fingerprint
        let mut trace3 = WorkflowTrace::new("workflow-1", "exec-1");
        trace3.start_node("node-2".to_string(), "task".to_string(), None);
        trace3.complete_node(0, Some(serde_json::json!({})), ExecutionStatus::Completed);
        trace3.finish(ExecutionStatus::Completed);

        let fingerprint3 = trace3.fingerprint();
        assert_ne!(fingerprint1, fingerprint3);
    }

    #[test]
    fn test_record_replay_identical_result() {
        // Simulate: Record phase - create original trace
        let mut recorded_trace = WorkflowTrace::new("test-workflow", "exec-123");
        recorded_trace.start_node("fetch-data".to_string(), "task".to_string(), Some(serde_json::json!({"source": "api"})));
        recorded_trace.complete_node(0, Some(serde_json::json!({"items": [1, 2, 3], "count": 3})), ExecutionStatus::Completed);
        recorded_trace.start_node("process-data".to_string(), "task".to_string(), Some(serde_json::json!({"input": [1, 2, 3]})));
        recorded_trace.complete_node(1, Some(serde_json::json!({"sum": 6, "avg": 2.0})), ExecutionStatus::Completed);
        recorded_trace.finish(ExecutionStatus::Completed);

        // Simulate: Replay phase - create replay trace (should match)
        let mut replay_trace = WorkflowTrace::new("test-workflow", "exec-456");
        replay_trace.start_node("fetch-data".to_string(), "task".to_string(), Some(serde_json::json!({"source": "api"})));
        replay_trace.complete_node(0, Some(serde_json::json!({"items": [1, 2, 3], "count": 3})), ExecutionStatus::Completed);
        replay_trace.start_node("process-data".to_string(), "task".to_string(), Some(serde_json::json!({"input": [1, 2, 3]})));
        replay_trace.complete_node(1, Some(serde_json::json!({"sum": 6, "avg": 2.0})), ExecutionStatus::Completed);
        replay_trace.finish(ExecutionStatus::Completed);

        // Validate: Both traces should be identical
        let result = recorded_trace.compare(&replay_trace);
        assert!(result.identical, "Record → Replay should produce identical results");
        assert!(result.node_count_match);
        assert!(result.order_match);
        assert!(result.output_differences.is_empty());
        assert!(result.status_differences.is_empty());

        // Snapshot validation should also pass
        assert!(replay_trace.validate_snapshot(&recorded_trace).is_ok());
    }

    #[test]
    fn test_replay_detects_divergence() {
        // Create original trace
        let mut original = WorkflowTrace::new("workflow", "exec-1");
        original.start_node("node-1".to_string(), "task".to_string(), None);
        original.complete_node(0, Some(serde_json::json!({"value": 100})), ExecutionStatus::Completed);
        original.finish(ExecutionStatus::Completed);

        // Create replay trace with different output (simulating divergence)
        let mut replay = WorkflowTrace::new("workflow", "exec-2");
        replay.start_node("node-1".to_string(), "task".to_string(), None);
        replay.complete_node(0, Some(serde_json::json!({"value": 200})), ExecutionStatus::Completed); // Different!
        replay.finish(ExecutionStatus::Completed);

        // Should detect the divergence
        let result = original.compare(&replay);
        assert!(!result.identical);
        assert_eq!(result.output_differences.len(), 1);
        assert_eq!(result.output_differences[0].expected_output, serde_json::json!({"value": 100}));
        assert_eq!(result.output_differences[0].actual_output, serde_json::json!({"value": 200}));
    }
}
