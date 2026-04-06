use mofa_kernel::workflow::{StateGraph, START, END, NodeFunc, Command, RuntimeContext, GraphState, StateUpdate};
use mofa_foundation::workflow::{StateGraphImpl, NodePolicy};
use async_trait::async_trait;
use mofa_kernel::agent::error::{AgentError, AgentResult};

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct DummyState;

#[async_trait]
impl GraphState for DummyState {
    fn keys(&self) -> Vec<&str> { vec![] }
    fn get_value<V: serde::de::DeserializeOwned + Send + Sync + 'static>(&self, _key: &str) -> Option<V> { None }
    async fn apply_update<V: serde::Serialize + Send + Sync + 'static>(&mut self, _key: &str, _value: V) -> AgentResult<()> { Ok(()) }
    fn to_json(&self) -> Result<serde_json::Value, AgentError> { Ok(serde_json::Value::Null) }
    fn from_json(_: serde_json::Value) -> Result<Self, AgentError> { Ok(DummyState) }
}

pub struct DummyNode;
#[async_trait]
impl NodeFunc<DummyState> for DummyNode {
    async fn call(&self, _state: &mut DummyState, _ctx: &RuntimeContext<serde_json::Value>) -> AgentResult<Command> {
        Ok(Command::new())
    }
    fn name(&self) -> &str { "dummy" }
}

#[tokio::test]
async fn test_fallback_reachability() {
    let mut graph = StateGraphImpl::<DummyState>::new("test_graph");
    graph.add_node("A", Box::new(DummyNode));
    graph.add_node("FallbackA", Box::new(DummyNode));
    graph.add_edge(START, "A");
    graph.add_edge("A", END);
    // Notice: NO edge connecting to FallbackA. It is only reachable via policy fallback.
    
    let mut policy = NodePolicy::default();
    policy.fallback_node = Some("FallbackA".to_string());
    
    graph.with_policy("A", policy);
    
    match graph.compile() {
        Ok(_) => println!("Compilation succeeded!"),
        Err(e) => {
            println!("Compilation failed: {}", e);
            panic!("Compilation failed: {}", e);
        }
    }
}
