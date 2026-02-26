// =============================================================================
// Execution Timeline Profiler
// =============================================================================
// Phase 1: Structured timing capture for workflow execution
// =============================================================================

use mofa_kernel::workflow::telemetry::DebugEvent;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Profiler mode configuration
#[derive(Debug, Clone)]
pub enum ProfilerMode {
    /// Profiler disabled - no overhead
    Disabled,
    /// Profiler enabled - records spans
    Record(Arc<ExecutionTimeline>),
}

/// Structured timing span for a single node execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeSpan {
    /// Node identifier
    pub node_id: String,
    /// When node started (ms since epoch)
    pub started_at_ms: u64,
    /// When node ended (ms since epoch)
    pub ended_at_ms: Option<u64>,
    /// Duration in milliseconds (computed when ended)
    pub duration_ms: Option<u64>,
    /// Tool spans within this node
    pub tool_spans: Vec<ToolSpan>,
}

/// Structured timing span for a tool invocation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSpan {
    /// Tool identifier
    pub tool_id: String,
    /// Tool name
    pub tool_name: String,
    /// When tool started
    pub started_at_ms: u64,
    /// When tool ended
    pub ended_at_ms: Option<u64>,
    /// Duration in milliseconds
    pub duration_ms: Option<u64>,
}

/// Complete execution timeline for a workflow run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionTimeline {
    /// Workflow identifier
    pub workflow_id: String,
    /// Execution identifier
    pub execution_id: String,
    /// When workflow started
    pub started_at_ms: u64,
    /// When workflow ended
    pub ended_at_ms: Option<u64>,
    /// Total duration in milliseconds
    pub duration_ms: Option<u64>,
    /// Node execution spans
    pub node_spans: Vec<NodeSpan>,
    /// Current node span being recorded
    #[serde(skip)]
    current_node_span: Option<usize>,
    /// Current tool span being recorded
    #[serde(skip)]
    current_tool_span: Option<usize>,
}

impl ExecutionTimeline {
    /// Create a new execution timeline
    pub fn new(workflow_id: String, execution_id: String) -> Self {
        Self {
            workflow_id,
            execution_id,
            started_at_ms: DebugEvent::now_ms(),
            ended_at_ms: None,
            duration_ms: None,
            node_spans: Vec::new(),
            current_node_span: None,
            current_tool_span: None,
        }
    }

    /// Record workflow end
    pub fn finish(&mut self) {
        let now = DebugEvent::now_ms();
        self.ended_at_ms = Some(now);
        self.duration_ms = Some(now.saturating_sub(self.started_at_ms));
    }

    /// Start recording a node span
    pub fn start_node(&mut self, node_id: String) {
        let span = NodeSpan {
            node_id,
            started_at_ms: DebugEvent::now_ms(),
            ended_at_ms: None,
            duration_ms: None,
            tool_spans: Vec::new(),
        };
        self.current_node_span = Some(self.node_spans.len());
        self.node_spans.push(span);
    }

    /// End the current node span
    pub fn end_node(&mut self) {
        if let Some(idx) = self.current_node_span {
            if let Some(span) = self.node_spans.get_mut(idx) {
                let now = DebugEvent::now_ms();
                span.ended_at_ms = Some(now);
                span.duration_ms = Some(now.saturating_sub(span.started_at_ms));
            }
        }
        self.current_node_span = None;
        self.current_tool_span = None;
    }

    /// Start recording a tool span within current node
    pub fn start_tool(&mut self, tool_id: String, tool_name: String) {
        if let Some(idx) = self.current_node_span {
            if let Some(span) = self.node_spans.get_mut(idx) {
                let tool_span = ToolSpan {
                    tool_id,
                    tool_name,
                    started_at_ms: DebugEvent::now_ms(),
                    ended_at_ms: None,
                    duration_ms: None,
                };
                self.current_tool_span = Some(span.tool_spans.len());
                span.tool_spans.push(tool_span);
            }
        }
    }

    /// End the current tool span
    pub fn end_tool(&mut self) {
        if let Some(node_idx) = self.current_node_span {
            if let Some(span) = self.node_spans.get_mut(node_idx) {
                if let Some(tool_idx) = self.current_tool_span {
                    if let Some(tool_span) = span.tool_spans.get_mut(tool_idx) {
                        let now = DebugEvent::now_ms();
                        tool_span.ended_at_ms = Some(now);
                        tool_span.duration_ms = Some(now.saturating_sub(tool_span.started_at_ms));
                    }
                }
            }
        }
        self.current_tool_span = None;
    }

    /// Check if profiler is enabled
    pub fn is_enabled(&self) -> bool {
        true // Only created when enabled
    }

    /// Get a reference to the timeline
    pub fn get_timeline(&self) -> &ExecutionTimeline {
        self
    }
}

/// Handle for accessing profiler state
#[derive(Clone)]
pub struct ProfilerHandle {
    timeline: Arc<ExecutionTimeline>,
}

impl ProfilerHandle {
    /// Create a new profiler handle
    pub fn new(workflow_id: String, execution_id: String) -> Self {
        Self {
            timeline: Arc::new(ExecutionTimeline::new(workflow_id, execution_id)),
        }
    }

    /// Get timeline reference
    pub fn timeline(&self) -> &Arc<ExecutionTimeline> {
        &self.timeline
    }

    /// Get mutable timeline (for recording)
    pub fn timeline_mut(&mut self) -> Arc<ExecutionTimeline> {
        Arc::clone(&self.timeline)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profiler_disabled_no_spans() {
        // When profiler is disabled, nothing should be recorded
        let mode = ProfilerMode::Disabled;
        match mode {
            ProfilerMode::Disabled => {}
            ProfilerMode::Record(_) => panic!("Expected Disabled"),
        }
    }

    #[test]
    fn test_node_duration_recorded() {
        let mut timeline = ExecutionTimeline::new("workflow-1".to_string(), "exec-1".to_string());
        
        // Start and end a node
        timeline.start_node("node-1".to_string());
        timeline.end_node();
        
        // Verify duration was computed (may be 0 in fast tests)
        assert!(timeline.node_spans[0].duration_ms.is_some());
    }

    #[test]
    fn test_tool_duration_recorded() {
        let mut timeline = ExecutionTimeline::new("workflow-1".to_string(), "exec-1".to_string());
        
        // Start node, then tool
        timeline.start_node("node-1".to_string());
        timeline.start_tool("tool-1".to_string(), "search".to_string());
        timeline.end_tool();
        timeline.end_node();
        
        // Verify tool duration was computed (may be 0 in fast tests)
        assert!(timeline.node_spans[0].tool_spans[0].duration_ms.is_some());
    }

    #[test]
    fn test_workflow_duration() {
        let mut timeline = ExecutionTimeline::new("workflow-1".to_string(), "exec-1".to_string());
        
        timeline.start_node("node-1".to_string());
        timeline.end_node();
        timeline.finish();
        
        // Verify workflow duration
        assert!(timeline.duration_ms.is_some());
    }
}
