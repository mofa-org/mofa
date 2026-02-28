//! 工作流图结构
//! Workflow graph structure
//!
//! 定义工作流的有向图结构和边
//! Defines the directed graph structure and edges of the workflow

use super::node::{NodeType, WorkflowNode};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use tracing::{debug, warn};

/// 边类型
/// Edge type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EdgeType {
    /// 普通边（顺序执行）
    /// Normal edge (sequential execution)
    Normal,
    /// 条件边（条件为真时执行）
    /// Conditional edge (executed when condition is true)
    Conditional(String),
    /// 错误边（发生错误时执行）
    /// Error edge (executed when an error occurs)
    Error,
    /// 默认边（无其他边匹配时执行）
    /// Default edge (executed when no other edges match)
    Default,
}

/// 边配置
/// Edge configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeConfig {
    /// 源节点 ID
    /// Source node ID
    pub from: String,
    /// 目标节点 ID
    /// Target node ID
    pub to: String,
    /// 边类型
    /// Edge type
    pub edge_type: EdgeType,
    /// 边标签（用于显示）
    /// Edge label (for display purposes)
    pub label: Option<String>,
}

impl EdgeConfig {
    pub fn new(from: &str, to: &str) -> Self {
        Self {
            from: from.to_string(),
            to: to.to_string(),
            edge_type: EdgeType::Normal,
            label: None,
        }
    }

    pub fn conditional(from: &str, to: &str, condition: &str) -> Self {
        Self {
            from: from.to_string(),
            to: to.to_string(),
            edge_type: EdgeType::Conditional(condition.to_string()),
            label: Some(condition.to_string()),
        }
    }

    pub fn error(from: &str, to: &str) -> Self {
        Self {
            from: from.to_string(),
            to: to.to_string(),
            edge_type: EdgeType::Error,
            label: Some("error".to_string()),
        }
    }

    pub fn default_edge(from: &str, to: &str) -> Self {
        Self {
            from: from.to_string(),
            to: to.to_string(),
            edge_type: EdgeType::Default,
            label: Some("default".to_string()),
        }
    }

    pub fn with_label(mut self, label: &str) -> Self {
        self.label = Some(label.to_string());
        self
    }
}

/// 工作流图
/// Workflow graph
pub struct WorkflowGraph {
    /// 图 ID
    /// Graph ID
    pub id: String,
    /// 图名称
    /// Graph name
    pub name: String,
    /// 图描述
    /// Graph description
    pub description: String,
    /// 节点映射
    /// Node mapping
    nodes: HashMap<String, WorkflowNode>,
    /// 边列表（邻接表：源节点 ID -> 边列表）
    /// Edge list (adjacency list: source node ID -> edge list)
    edges: HashMap<String, Vec<EdgeConfig>>,
    /// 反向边（用于查找入边）
    /// Reverse edges (used to find incoming edges)
    reverse_edges: HashMap<String, Vec<EdgeConfig>>,
    /// 开始节点 ID
    /// Start node ID
    start_node: Option<String>,
    /// 结束节点 ID 列表（可能有多个）
    /// List of end node IDs (can have multiple)
    end_nodes: Vec<String>,
}

impl WorkflowGraph {
    pub fn new(id: &str, name: &str) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            description: String::new(),
            nodes: HashMap::new(),
            edges: HashMap::new(),
            reverse_edges: HashMap::new(),
            start_node: None,
            end_nodes: Vec::new(),
        }
    }

    pub fn with_description(mut self, desc: &str) -> Self {
        self.description = desc.to_string();
        self
    }

    /// 添加节点
    /// Add node
    pub fn add_node(&mut self, node: WorkflowNode) -> &mut Self {
        let node_id = node.id().to_string();

        // 自动检测开始和结束节点
        // Automatically detect start and end nodes
        match node.node_type() {
            NodeType::Start => {
                self.start_node = Some(node_id.clone());
            }
            NodeType::End => {
                self.end_nodes.push(node_id.clone());
            }
            _ => {}
        }

        self.nodes.insert(node_id.clone(), node);
        self.edges.entry(node_id.clone()).or_default();
        self.reverse_edges.entry(node_id).or_default();
        self
    }

    /// 添加边
    /// Add edge
    pub fn add_edge(&mut self, edge: EdgeConfig) -> &mut Self {
        let from = edge.from.clone();
        let to = edge.to.clone();

        // 添加正向边
        // Add forward edge
        self.edges.entry(from).or_default().push(edge.clone());

        // 添加反向边
        // Add reverse edge
        self.reverse_edges.entry(to).or_default().push(edge);

        self
    }

    /// 添加普通边
    /// Add normal edge
    pub fn connect(&mut self, from: &str, to: &str) -> &mut Self {
        self.add_edge(EdgeConfig::new(from, to))
    }

    /// 添加条件边
    /// Add conditional edge
    pub fn connect_conditional(&mut self, from: &str, to: &str, condition: &str) -> &mut Self {
        self.add_edge(EdgeConfig::conditional(from, to, condition))
    }

    /// 获取节点
    /// Get node
    pub fn get_node(&self, node_id: &str) -> Option<&WorkflowNode> {
        self.nodes.get(node_id)
    }

    /// 获取可变节点
    /// Get mutable node
    pub fn get_node_mut(&mut self, node_id: &str) -> Option<&mut WorkflowNode> {
        self.nodes.get_mut(node_id)
    }

    /// 获取所有节点 ID
    /// Get all node IDs
    pub fn node_ids(&self) -> Vec<&str> {
        self.nodes.keys().map(|s| s.as_str()).collect()
    }

    /// 获取节点数量
    /// Get node count
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// 获取边数量
    /// Get edge count
    pub fn edge_count(&self) -> usize {
        self.edges.values().map(|e| e.len()).sum()
    }

    /// 获取开始节点
    /// Get start node
    pub fn start_node(&self) -> Option<&str> {
        self.start_node.as_deref()
    }

    /// 获取结束节点列表
    /// Get list of end nodes
    pub fn end_nodes(&self) -> &[String] {
        &self.end_nodes
    }

    /// 获取节点的出边
    /// Get outgoing edges of a node
    pub fn get_outgoing_edges(&self, node_id: &str) -> &[EdgeConfig] {
        self.edges.get(node_id).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// 获取节点的入边
    /// Get incoming edges of a node
    pub fn get_incoming_edges(&self, node_id: &str) -> &[EdgeConfig] {
        self.reverse_edges
            .get(node_id)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// 获取节点的后继节点
    /// Get successor nodes of a node
    pub fn get_successors(&self, node_id: &str) -> Vec<&str> {
        self.get_outgoing_edges(node_id)
            .iter()
            .map(|e| e.to.as_str())
            .collect()
    }

    /// 获取节点的前驱节点
    /// Get predecessor nodes of a node
    pub fn get_predecessors(&self, node_id: &str) -> Vec<&str> {
        self.get_incoming_edges(node_id)
            .iter()
            .map(|e| e.from.as_str())
            .collect()
    }

    /// 获取满足条件的下一个节点
    /// Get the next node satisfying the condition
    pub fn get_next_node(&self, node_id: &str, condition: Option<&str>) -> Option<&str> {
        let edges = self.get_outgoing_edges(node_id);

        // 优先匹配条件边
        // Prioritize matching conditional edges
        if let Some(cond) = condition {
            for edge in edges {
                if let EdgeType::Conditional(c) = &edge.edge_type
                    && c == cond
                {
                    return Some(&edge.to);
                }
            }
        }

        // 其次匹配默认边
        // Secondarily match default edges
        for edge in edges {
            if matches!(edge.edge_type, EdgeType::Default) {
                return Some(&edge.to);
            }
        }

        // 最后匹配普通边
        // Finally match normal edges
        for edge in edges {
            if matches!(edge.edge_type, EdgeType::Normal) {
                return Some(&edge.to);
            }
        }

        None
    }

    /// 获取错误处理节点
    /// Get error handling node
    pub fn get_error_handler(&self, node_id: &str) -> Option<&str> {
        let edges = self.get_outgoing_edges(node_id);
        for edge in edges {
            if matches!(edge.edge_type, EdgeType::Error) {
                return Some(&edge.to);
            }
        }
        None
    }

    /// 拓扑排序
    /// Topological sort
    pub fn topological_sort(&self) -> Result<Vec<String>, String> {
        let mut in_degree: HashMap<&str, usize> = HashMap::new();
        let mut queue: VecDeque<&str> = VecDeque::new();
        let mut result: Vec<String> = Vec::new();

        // 计算入度
        // Calculate in-degree
        for node_id in self.nodes.keys() {
            in_degree.insert(node_id, 0);
        }
        for edges in self.edges.values() {
            for edge in edges {
                *in_degree.entry(&edge.to).or_insert(0) += 1;
            }
        }

        // 入度为 0 的节点入队
        // Enqueue nodes with in-degree of 0
        for (node_id, &degree) in &in_degree {
            if degree == 0 {
                queue.push_back(node_id);
            }
        }

        // BFS
        while let Some(node_id) = queue.pop_front() {
            result.push(node_id.to_string());

            for edge in self.get_outgoing_edges(node_id) {
                if let Some(degree) = in_degree.get_mut(edge.to.as_str()) {
                    *degree -= 1;
                    if *degree == 0 {
                        queue.push_back(&edge.to);
                    }
                }
            }
        }

        // 检查是否有环
        // Check for cycles
        if result.len() != self.nodes.len() {
            return Err("Graph contains a cycle".to_string());
        }

        Ok(result)
    }

    /// 检测环
    /// Detect cycle
    pub fn has_cycle(&self) -> bool {
        self.topological_sort().is_err()
    }

    /// 获取可以并行执行的节点组
    /// Get node groups that can be executed in parallel
    pub fn get_parallel_groups(&self) -> Vec<Vec<String>> {
        let mut groups: Vec<Vec<String>> = Vec::new();
        let mut in_degree: HashMap<&str, usize> = HashMap::new();
        let mut remaining: HashSet<&str> = self.nodes.keys().map(|s| s.as_str()).collect();

        // 计算入度
        // Calculate in-degree
        for node_id in self.nodes.keys() {
            in_degree.insert(node_id, 0);
        }
        for edges in self.edges.values() {
            for edge in edges {
                *in_degree.entry(&edge.to).or_insert(0) += 1;
            }
        }

        while !remaining.is_empty() {
            // 找出当前入度为 0 的节点
            // Find nodes currently having an in-degree of 0
            let ready: Vec<String> = remaining
                .iter()
                .filter(|&&node_id| in_degree.get(node_id).copied().unwrap_or(0) == 0)
                .map(|&s| s.to_string())
                .collect();

            if ready.is_empty() {
                warn!("Cycle detected in workflow graph");
                break;
            }

            // 更新入度
            // Update in-degree
            for node_id in &ready {
                remaining.remove(node_id.as_str());
                for edge in self.get_outgoing_edges(node_id) {
                    if let Some(degree) = in_degree.get_mut(edge.to.as_str()) {
                        *degree = degree.saturating_sub(1);
                    }
                }
            }

            groups.push(ready);
        }

        groups
    }

    /// 验证图的完整性
    /// Validate graph integrity
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors: Vec<String> = Vec::new();

        // 检查是否有开始节点
        // Check if start node exists
        if self.start_node.is_none() {
            errors.push("No start node found".to_string());
        }

        // 检查是否有结束节点
        // Check if end nodes exist
        if self.end_nodes.is_empty() {
            errors.push("No end node found".to_string());
        }

        // 检查边引用的节点是否存在
        // Check if nodes referenced by edges exist
        for (from, edges) in &self.edges {
            if !self.nodes.contains_key(from) {
                errors.push(format!("Edge source node '{}' not found", from));
            }
            for edge in edges {
                if !self.nodes.contains_key(&edge.to) {
                    errors.push(format!("Edge target node '{}' not found", edge.to));
                }
            }
        }

        // 检查是否有孤立节点
        // Check for isolated nodes
        for node_id in self.nodes.keys() {
            if node_id != self.start_node.as_ref().unwrap_or(&String::new())
                && self.get_incoming_edges(node_id).is_empty()
            {
                errors.push(format!("Node '{}' is unreachable", node_id));
            }
        }

        // 检查是否有环
        // Check for cycles
        if self.has_cycle() {
            errors.push("Graph contains a cycle".to_string());
        }

        // 检查并行节点是否有对应的聚合节点
        // Check if parallel nodes have corresponding join nodes
        for (node_id, node) in &self.nodes {
            if matches!(node.node_type(), NodeType::Parallel) {
                // 检查每个分支是否最终汇聚
                // Check if each branch eventually converges
                debug!("Checking parallel node: {}", node_id);
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// 获取从源到目标的所有路径
    /// Get all paths from source to target
    pub fn find_all_paths(&self, from: &str, to: &str) -> Vec<Vec<String>> {
        let mut paths: Vec<Vec<String>> = Vec::new();
        let mut current_path: Vec<String> = Vec::new();
        let mut visited: HashSet<String> = HashSet::new();

        self.dfs_paths(from, to, &mut current_path, &mut visited, &mut paths);
        paths
    }

    fn dfs_paths(
        &self,
        current: &str,
        target: &str,
        path: &mut Vec<String>,
        visited: &mut HashSet<String>,
        paths: &mut Vec<Vec<String>>,
    ) {
        path.push(current.to_string());
        visited.insert(current.to_string());

        if current == target {
            paths.push(path.clone());
        } else {
            for edge in self.get_outgoing_edges(current) {
                if !visited.contains(&edge.to) {
                    self.dfs_paths(&edge.to, target, path, visited, paths);
                }
            }
        }

        path.pop();
        visited.remove(current);
    }

    /// 导出为 DOT 格式（用于可视化）
    /// Export to DOT format (for visualization)
    pub fn to_dot(&self) -> String {
        let mut dot = String::new();
        dot.push_str(&format!("digraph \"{}\" {{\n", self.name));
        dot.push_str("  rankdir=TB;\n");
        dot.push_str("  node [shape=box];\n\n");

        // 节点
        // Nodes
        for (node_id, node) in &self.nodes {
            let shape = match node.node_type() {
                NodeType::Start => "ellipse",
                NodeType::End => "ellipse",
                NodeType::Condition => "diamond",
                NodeType::Parallel => "parallelogram",
                NodeType::Join => "parallelogram",
                NodeType::Loop => "hexagon",
                _ => "box",
            };
            let color = match node.node_type() {
                NodeType::Start => "green",
                NodeType::End => "red",
                NodeType::Condition => "yellow",
                NodeType::Parallel | NodeType::Join => "cyan",
                _ => "white",
            };
            dot.push_str(&format!(
                "  \"{}\" [label=\"{}\\n({})\", shape={}, style=filled, fillcolor={}];\n",
                node_id, node.config.name, node_id, shape, color
            ));
        }

        dot.push('\n');

        // 边
        // Edges
        for (from, edges) in &self.edges {
            for edge in edges {
                let label = edge.label.as_deref().unwrap_or("");
                let style = match edge.edge_type {
                    EdgeType::Normal => "solid",
                    EdgeType::Conditional(_) => "dashed",
                    EdgeType::Error => "dotted",
                    EdgeType::Default => "bold",
                };
                dot.push_str(&format!(
                    "  \"{}\" -> \"{}\" [label=\"{}\", style={}];\n",
                    from, edge.to, label, style
                ));
            }
        }

        dot.push_str("}\n");
        dot
    }

    /// Export to JSON format (for web visualization)
    pub fn to_json(&self) -> serde_json::Value {
        let nodes: Vec<serde_json::Value> = self
            .nodes
            .iter()
            .map(|(node_id, node)| {
                let node_type_str = match node.node_type() {
                    NodeType::Start => "start",
                    NodeType::End => "end",
                    NodeType::Task => "task",
                    NodeType::Agent => "agent",
                    NodeType::Condition => "condition",
                    NodeType::Parallel => "parallel",
                    NodeType::Join => "join",
                    NodeType::Loop => "loop",
                    NodeType::Wait => "wait",
                    NodeType::Transform => "transform",
                    NodeType::SubWorkflow => "sub_workflow",
                };
                serde_json::json!({
                    "id": node_id,
                    "name": node.config.name,
                    "type": node_type_str,
                    "description": node.config.description,
                })
            })
            .collect();

        let edges: Vec<serde_json::Value> = self
            .edges
            .values()
            .flatten()
            .map(|edge| {
                let edge_type_str = match &edge.edge_type {
                    EdgeType::Normal => "normal",
                    EdgeType::Conditional(_) => "conditional",
                    EdgeType::Error => "error",
                    EdgeType::Default => "default",
                };
                serde_json::json!({
                    "from": edge.from,
                    "to": edge.to,
                    "edge_type": edge_type_str,
                    "label": edge.label,
                })
            })
            .collect();

        serde_json::json!({
            "id": self.id,
            "name": self.name,
            "description": self.description,
            "nodes": nodes,
            "edges": edges,
            "start_node": self.start_node,
            "end_nodes": self.end_nodes,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_graph() -> WorkflowGraph {
        let mut graph = WorkflowGraph::new("test", "Test Workflow");

        graph.add_node(WorkflowNode::start("start"));
        graph.add_node(WorkflowNode::task(
            "task1",
            "Task 1",
            |_ctx, input| async move { Ok(input) },
        ));
        graph.add_node(WorkflowNode::task(
            "task2",
            "Task 2",
            |_ctx, input| async move { Ok(input) },
        ));
        graph.add_node(WorkflowNode::end("end"));

        graph.connect("start", "task1");
        graph.connect("task1", "task2");
        graph.connect("task2", "end");

        graph
    }

    #[test]
    fn test_topological_sort() {
        let graph = create_test_graph();
        let sorted = graph.topological_sort().unwrap();

        // start 应该在前面
        // start should be at the front
        let start_pos = sorted.iter().position(|x| x == "start").unwrap();
        let task1_pos = sorted.iter().position(|x| x == "task1").unwrap();
        let task2_pos = sorted.iter().position(|x| x == "task2").unwrap();
        let end_pos = sorted.iter().position(|x| x == "end").unwrap();

        assert!(start_pos < task1_pos);
        assert!(task1_pos < task2_pos);
        assert!(task2_pos < end_pos);
    }

    #[test]
    fn test_parallel_groups() {
        let mut graph = WorkflowGraph::new("test", "Test");

        graph.add_node(WorkflowNode::start("start"));
        graph.add_node(WorkflowNode::task("a", "A", |_ctx, input| async move {
            Ok(input)
        }));
        graph.add_node(WorkflowNode::task("b", "B", |_ctx, input| async move {
            Ok(input)
        }));
        graph.add_node(WorkflowNode::task("c", "C", |_ctx, input| async move {
            Ok(input)
        }));
        graph.add_node(WorkflowNode::end("end"));

        graph.connect("start", "a");
        graph.connect("start", "b");
        graph.connect("a", "c");
        graph.connect("b", "c");
        graph.connect("c", "end");

        let groups = graph.get_parallel_groups();

        // 第一组: start
        // Group 1: start
        // 第二组: a, b (可并行)
        // Group 2: a, b (can be parallel)
        // 第三组: c
        // Group 3: c
        // 第四组: end
        // Group 4: end
        assert_eq!(groups.len(), 4);
        assert!(groups[1].contains(&"a".to_string()) && groups[1].contains(&"b".to_string()));
    }

    #[test]
    fn test_cycle_detection() {
        let mut graph = WorkflowGraph::new("test", "Test");

        graph.add_node(WorkflowNode::task("a", "A", |_ctx, input| async move {
            Ok(input)
        }));
        graph.add_node(WorkflowNode::task("b", "B", |_ctx, input| async move {
            Ok(input)
        }));
        graph.add_node(WorkflowNode::task("c", "C", |_ctx, input| async move {
            Ok(input)
        }));

        graph.connect("a", "b");
        graph.connect("b", "c");
        graph.connect("c", "a"); // 形成环
        // forms a cycle

        assert!(graph.has_cycle());
    }

    #[test]
    fn test_find_paths() {
        let graph = create_test_graph();
        let paths = graph.find_all_paths("start", "end");

        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0], vec!["start", "task1", "task2", "end"]);
    }

    #[test]
    fn test_to_dot() {
        let graph = create_test_graph();
        let dot = graph.to_dot();

        assert!(dot.contains("digraph"));
        assert!(dot.contains("start"));
        assert!(dot.contains("end"));
        assert!(dot.contains("->"));
    }

    #[test]
    fn test_to_json() {
        let graph = create_test_graph();
        let json = graph.to_json();

        assert_eq!(json["id"], "test");
        assert_eq!(json["name"], "Test Workflow");
        assert!(json["nodes"].as_array().unwrap().len() == 4);
        assert!(json["edges"].as_array().unwrap().len() == 3);
        assert_eq!(json["start_node"], "start");
        assert!(json["end_nodes"].as_array().unwrap().contains(&serde_json::json!("end")));
    }
}
