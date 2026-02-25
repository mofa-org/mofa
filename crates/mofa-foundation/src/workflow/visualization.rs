//! Workflow Visualization Module
//!
//! Provides JSON export functionality for workflow graphs to be rendered
//! in web-based visualization tools like React Flow or D3.js.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::graph::{EdgeConfig, EdgeType, WorkflowGraph};
use super::node::{NodeType, WorkflowNode};
use super::state::NodeStatus;

/// Visualization node data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VizNode {
    /// Unique node ID
    pub id: String,
    /// Node type
    pub node_type: String,
    /// Node label (for display)
    pub label: String,
    /// Node description
    pub description: Option<String>,
    /// Position (for layout)
    pub position: Option<VizPosition>,
    /// Execution status (for live updates)
    pub status: Option<String>,
    /// Error message if failed
    pub error: Option<String>,
    /// Input schema
    pub input_schema: Option<HashMap<String, serde_json::Value>>,
    /// Output schema
    pub output_schema: Option<HashMap<String, serde_json::Value>>,
    /// Configuration
    pub config: Option<HashMap<String, serde_json::Value>>,
}

/// Node position for layout
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VizPosition {
    pub x: f64,
    pub y: f64,
}

/// Visualization edge data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VizEdge {
    /// Unique edge ID
    pub id: String,
    /// Source node ID
    pub source: String,
    /// Target node ID
    pub target: String,
    /// Edge type
    pub edge_type: String,
    /// Edge label
    pub label: Option<String>,
    /// Condition for conditional edges
    pub condition: Option<String>,
    /// Animated (for execution visualization)
    pub animated: bool,
    /// Edge style
    pub style: Option<VizEdgeStyle>,
}

/// Edge style
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VizEdgeStyle {
    pub stroke: Option<String>,
    pub stroke_width: Option<f64>,
    pub stroke_dasharray: Option<String>,
}

/// Complete visualization graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowVisualization {
    /// Graph metadata
    pub metadata: WorkflowVizMetadata,
    /// Nodes
    pub nodes: Vec<VizNode>,
    /// Edges
    pub edges: Vec<VizEdge>,
    /// Node status map (optional - for live execution)
    pub node_statuses: Option<HashMap<String, String>>,
}

/// Workflow visualization metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowVizMetadata {
    /// Graph ID
    pub id: String,
    /// Graph name
    pub name: String,
    /// Graph description
    pub description: String,
    /// Start node ID
    pub start_node: Option<String>,
    /// End node IDs
    pub end_nodes: Vec<String>,
    /// Total node count
    pub node_count: usize,
    /// Total edge count
    pub edge_count: usize,
}

impl WorkflowGraph {
    /// Export to visualization JSON (without status)
    pub fn to_visualization(&self) -> WorkflowVisualization {
        self.to_visualization_with_status(None)
    }

    /// Export to visualization JSON with execution status
    pub fn to_visualization_with_status(
        &self,
        statuses: Option<HashMap<String, NodeStatus>>,
    ) -> WorkflowVisualization {
        // Generate nodes
        let nodes: Vec<VizNode> = self
            .get_all_nodes()
            .iter()
            .map(|(node_id, node)| {
                let status = statuses.as_ref().and_then(|s| s.get(node_id));
                let (status_str, error) = match status {
                    Some(NodeStatus::Pending) => ("pending".to_string(), None),
                    Some(NodeStatus::Waiting) => ("waiting".to_string(), None),
                    Some(NodeStatus::Running) => ("running".to_string(), None),
                    Some(NodeStatus::Completed) => ("completed".to_string(), None),
                    Some(NodeStatus::Failed(msg)) => ("failed".to_string(), Some(msg.clone())),
                    Some(NodeStatus::Skipped) => ("skipped".to_string(), None),
                    Some(NodeStatus::Cancelled) => ("cancelled".to_string(), None),
                    None => ("unknown".to_string(), None),
                };

                VizNode {
                    id: node_id.clone(),
                    node_type: format!("{:?}", node.node_type()),
                    label: node.config.name.clone(),
                    description: Some(node.config.description.clone()),
                    position: None, // Layout will be calculated by frontend
                    status: Some(status_str),
                    error,
                    input_schema: None,
                    output_schema: None,
                    config: None,
                }
            })
            .collect();

        // Generate edges
        let edges: Vec<VizEdge> = self
            .get_all_edges()
            .iter()
            .flat_map(|(from, edge_list)| {
                edge_list.iter().map(|edge| {
                    let (edge_type_str, condition) = match &edge.edge_type {
                        EdgeType::Normal => ("normal".to_string(), None),
                        EdgeType::Conditional(c) => ("conditional".to_string(), Some(c.clone())),
                        EdgeType::Error => ("error".to_string(), None),
                        EdgeType::Default => ("default".to_string(), None),
                    };

                    let animated = matches!(
                        statuses.as_ref().and_then(|s| s.get(&edge.from)),
                        Some(NodeStatus::Running)
                    );

                    VizEdge {
                        id: format!("{}-{}", edge.from, edge.to),
                        source: edge.from.clone(),
                        target: edge.to.clone(),
                        edge_type: edge_type_str,
                        label: edge.label.clone(),
                        condition,
                        animated,
                        style: None,
                    }
                })
            })
            .collect();

        // Metadata
        let node_statuses = statuses.map(|s| {
            s.into_iter()
                .map(|(k, v)| {
                    let status_str = match v {
                        NodeStatus::Pending => "pending",
                        NodeStatus::Waiting => "waiting",
                        NodeStatus::Running => "running",
                        NodeStatus::Completed => "completed",
                        NodeStatus::Failed(_) => "failed",
                        NodeStatus::Skipped => "skipped",
                        NodeStatus::Cancelled => "cancelled",
                    };
                    (k, status_str.to_string())
                })
                .collect()
        });

        WorkflowVisualization {
            metadata: WorkflowVizMetadata {
                id: self.id.clone(),
                name: self.name.clone(),
                description: self.description.clone(),
                start_node: self.start_node().map(|s| s.to_string()),
                end_nodes: self.end_nodes().to_vec(),
                node_count: self.get_all_nodes().len(),
                edge_count: edges.len(),
            },
            nodes,
            edges,
            node_statuses,
        }
    }

    /// Get node types summary
    pub fn get_node_types_summary(&self) -> HashMap<String, usize> {
        let mut summary: HashMap<String, usize> = HashMap::new();
        for node in self.get_all_nodes().values() {
            let node_type = format!("{:?}", node.node_type());
            *summary.entry(node_type).or_insert(0) += 1;
        }
        summary
    }

    /// Get execution statistics
    pub fn get_execution_stats(&self, statuses: &HashMap<String, NodeStatus>) -> WorkflowExecutionStats {
        let mut stats = WorkflowExecutionStats::default();
        
        for (node_id, status) in statuses {
            match status {
                NodeStatus::Pending => stats.pending += 1,
                NodeStatus::Waiting => stats.waiting += 1,
                NodeStatus::Running => stats.running += 1,
                NodeStatus::Completed => stats.completed += 1,
                NodeStatus::Failed(_) => stats.failed += 1,
                NodeStatus::Skipped => stats.skipped += 1,
                NodeStatus::Cancelled => stats.cancelled += 1,
            }
        }
        
        stats.total = self.get_all_nodes().len();
        stats
    }
}

/// Workflow execution statistics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkflowExecutionStats {
    pub total: usize,
    pub pending: usize,
    pub waiting: usize,
    pub running: usize,
    pub completed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub cancelled: usize,
}

/// Simple auto-layout algorithm for nodes
pub fn auto_layout_nodes(
    nodes: &mut [VizNode],
    edges: &[VizEdge],
    direction: LayoutDirection,
) {
    match direction {
        LayoutDirection::TopToBottom => layout_top_to_bottom(nodes, edges),
        LayoutDirection::LeftToRight => layout_left_to_right(nodes, edges),
    }
}

/// Layout direction
#[derive(Debug, Clone, Copy)]
pub enum LayoutDirection {
    TopToBottom,
    LeftToRight,
}

/// Simple topological layout (top to bottom)
fn layout_top_to_bottom(nodes: &mut [VizNode], edges: &[VizEdge]) {
    // Find start nodes (nodes with no incoming edges)
    let has_incoming: std::collections::HashSet<_> = edges.iter().map(|e| &e.target).collect();
    let start_nodes: Vec<_> = nodes
        .iter()
        .filter(|n| !has_incoming.contains(&n.id))
        .map(|n| n.id.clone())
        .collect();

    if start_nodes.is_empty() {
        // Fallback: simple grid layout
        let cols = (nodes.len() as f64).sqrt() as usize;
        for (i, node) in nodes.iter_mut().enumerate() {
            node.position = Some(VizPosition {
                x: (i % cols) as f64 * 250.0,
                y: (i / cols) as f64 * 150.0,
            });
        }
        return;
    }

    // BFS to assign layers
    let mut layers: HashMap<String, usize> = HashMap::new();
    let mut queue: Vec<(String, usize)> = start_nodes.iter().map(|id| (id.clone(), 0)).collect();
    
    while let Some((node_id, layer)) = queue.pop() {
        if layers.contains_key(&node_id) {
            continue;
        }
        layers.insert(node_id.clone(), layer);
        
        // Find outgoing edges
        for edge in edges.iter().filter(|e| e.source == node_id) {
            queue.push((edge.target.clone(), layer + 1));
        }
    }

    // Assign positions based on layers
    let mut layer_nodes: HashMap<usize, Vec<String>> = HashMap::new();
    for (node_id, layer) in &layers {
        layer_nodes.entry(*layer).or_default().push(node_id.clone());
    }

    for (layer, node_ids) in layer_nodes {
        for (i, node_id) in node_ids.iter().enumerate() {
            if let Some(node) = nodes.iter_mut().find(|n| n.id == *node_id) {
                node.position = Some(VizPosition {
                    x: i as f64 * 300.0,
                    y: layer as f64 * 200.0,
                });
            }
        }
    }
}

/// Simple left-to-right layout
fn layout_left_to_right(nodes: &mut [VizNode], edges: &[VizEdge]) {
    // Same as top-to-bottom but with swapped axes
    let mut reversed_edges: Vec<VizEdge> = edges
        .iter()
        .map(|e| VizEdge {
            id: e.id.clone(),
            source: e.target.clone(),
            target: e.source.clone(),
            edge_type: e.edge_type.clone(),
            label: e.label.clone(),
            condition: e.condition.clone(),
            animated: e.animated,
            style: e.style.clone(),
        })
        .collect();
    
    layout_top_to_bottom(nodes, &mut reversed_edges);
    
    // Swap x and y
    for node in nodes.iter_mut() {
        if let Some(pos) = node.position.take() {
            node.position = Some(VizPosition {
                x: pos.y,
                y: pos.x,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow::builder::WorkflowBuilder;

    #[test]
    fn test_export_simple_workflow() {
        let mut graph = WorkflowBuilder::new("test", "Test Workflow")
            .start()
            .task("process", "Process Data", |_ctx, _input| async { Ok(()) })
            .end()
            .edge("start", "process")
            .edge("process", "end")
            .build();

        let viz = graph.to_visualization();
        
        assert_eq!(viz.metadata.id, "test");
        assert_eq!(viz.nodes.len(), 3);
        assert_eq!(viz.edges.len(), 2);
    }

    #[test]
    fn test_export_with_status() {
        let mut graph = WorkflowBuilder::new("test", "Test Workflow")
            .start()
            .end()
            .edge("start", "end")
            .build();

        let mut statuses = HashMap::new();
        statuses.insert("start".to_string(), NodeStatus::Completed);
        statuses.insert("end".to_string(), NodeStatus::Running);

        let viz = graph.to_visualization_with_status(Some(statuses));
        
        assert_eq!(viz.nodes[0].status, Some("completed".to_string()));
        assert_eq!(viz.nodes[1].status, Some("running".to_string()));
    }

    #[test]
    fn test_auto_layout() {
        let mut nodes = vec![
            VizNode {
                id: "a".to_string(),
                node_type: "Task".to_string(),
                label: "A".to_string(),
                description: None,
                position: None,
                status: None,
                error: None,
                input_schema: None,
                output_schema: None,
                config: None,
            },
            VizNode {
                id: "b".to_string(),
                node_type: "Task".to_string(),
                label: "B".to_string(),
                description: None,
                position: None,
                status: None,
                error: None,
                input_schema: None,
                output_schema: None,
                config: None,
            },
        ];

        let edges = vec![VizEdge {
            id: "a-b".to_string(),
            source: "a".to_string(),
            target: "b".to_string(),
            edge_type: "normal".to_string(),
            label: None,
            condition: None,
            animated: false,
            style: None,
        }];

        auto_layout_nodes(&mut nodes, &edges, LayoutDirection::TopToBottom);
        
        assert!(nodes[0].position.is_some());
        assert!(nodes[1].position.is_some());
    }
}
