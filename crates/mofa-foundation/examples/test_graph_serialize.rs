use mofa_foundation::workflow::{
    WorkflowGraph, WorkflowNode, EdgeConfig, 
};

fn main() {
    let mut graph = WorkflowGraph::new("test-graph", "Verification Setup");
    
    graph.add_node(WorkflowNode::start("n1"));
    graph.add_node(WorkflowNode::end("n2"));
    graph.add_node(WorkflowNode::end("n3"));

    graph.add_edge(EdgeConfig::new("n1", "n2"));
    graph.add_edge(EdgeConfig::new("n2", "n3"));
    
    let dto = graph.to_json_dto();
    let json = serde_json::to_string_pretty(&dto).unwrap();
    println!("{}", json);
}
