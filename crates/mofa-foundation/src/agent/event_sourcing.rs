use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentEvent {
    Thought(ThoughtEvent),
    Action(ActionEvent),
    Observation(ObservationEvent),
    Decision(DecisionEvent),
    ToolCall(ToolCallEvent),
    ToolResult(ToolResultEvent),
    Message(MessageEvent),
    StateChange(StateChangeEvent),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThoughtEvent {
    pub event_id: String,
    pub timestamp: u64,
    pub agent_id: String,
    pub thought: String,
    pub reasoning: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionEvent {
    pub event_id: String,
    pub timestamp: u64,
    pub agent_id: String,
    pub action_type: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservationEvent {
    pub event_id: String,
    pub timestamp: u64,
    pub agent_id: String,
    pub observation: String,
    pub source: ObservationSource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ObservationSource {
    Tool,
    Agent,
    User,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionEvent {
    pub event_id: String,
    pub timestamp: u64,
    pub agent_id: String,
    pub decision: String,
    pub confidence: Option<f64>,
    pub alternatives: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallEvent {
    pub event_id: String,
    pub timestamp: u64,
    pub agent_id: String,
    pub tool_name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResultEvent {
    pub event_id: String,
    pub timestamp: u64,
    pub agent_id: String,
    pub tool_name: String,
    pub result: serde_json::Value,
    pub success: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageEvent {
    pub event_id: String,
    pub timestamp: u64,
    pub agent_id: String,
    pub role: MessageRole,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateChangeEvent {
    pub event_id: String,
    pub timestamp: u64,
    pub agent_id: String,
    pub state_key: String,
    pub old_value: Option<serde_json::Value>,
    pub new_value: Option<serde_json::Value>,
}

impl AgentEvent {
    pub fn new_thought(agent_id: String, thought: String, reasoning: Option<String>) -> Self {
        Self::Thought(ThoughtEvent {
            event_id: uuid::Uuid::now_v7().to_string(),
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            agent_id,
            thought,
            reasoning,
        })
    }

    pub fn new_action(agent_id: String, action_type: String, parameters: serde_json::Value) -> Self {
        Self::Action(ActionEvent {
            event_id: uuid::Uuid::now_v7().to_string(),
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            agent_id,
            action_type,
            parameters,
        })
    }

    pub fn timestamp(&self) -> u64 {
        match self {
            AgentEvent::Thought(e) => e.timestamp,
            AgentEvent::Action(e) => e.timestamp,
            AgentEvent::Observation(e) => e.timestamp,
            AgentEvent::Decision(e) => e.timestamp,
            AgentEvent::ToolCall(e) => e.timestamp,
            AgentEvent::ToolResult(e) => e.timestamp,
            AgentEvent::Message(e) => e.timestamp,
            AgentEvent::StateChange(e) => e.timestamp,
        }
    }

    pub fn agent_id(&self) -> &str {
        match self {
            AgentEvent::Thought(e) => &e.agent_id,
            AgentEvent::Action(e) => &e.agent_id,
            AgentEvent::Observation(e) => &e.agent_id,
            AgentEvent::Decision(e) => &e.agent_id,
            AgentEvent::ToolCall(e) => &e.agent_id,
            AgentEvent::ToolResult(e) => &e.agent_id,
            AgentEvent::Message(e) => &e.agent_id,
            AgentEvent::StateChange(e) => &e.agent_id,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventSnapshot {
    pub snapshot_id: String,
    pub agent_id: String,
    pub version: u64,
    pub state: serde_json::Value,
    pub timestamp: u64,
}

pub trait EventStore: Send + Sync {
    fn append_event(&self, agent_id: &str, event: AgentEvent) -> Result<(), EventStoreError>;
    fn get_events(&self, agent_id: &str) -> Result<Vec<AgentEvent>, EventStoreError>;
    fn get_events_from(&self, agent_id: &str, from_version: u64) -> Result<Vec<AgentEvent>, EventStoreError>;
    fn create_snapshot(&self, snapshot: EventSnapshot) -> Result<(), EventStoreError>;
    fn get_snapshot(&self, agent_id: &str, version: u64) -> Result<Option<EventSnapshot>, EventStoreError>;
    fn get_latest_snapshot(&self, agent_id: &str) -> Result<Option<EventSnapshot>, EventStoreError>;
}

#[derive(Debug, thiserror::Error)]
pub enum EventStoreError {
    #[error(\"Event not found: {0}\")]
    NotFound(String),

    #[error(\"Storage error: {0}\")]
    StorageError(String),

    #[error(\"Serialization error: {0}\")]
    SerializationError(String),
}

pub type EventResult<T> = Result<T, EventStoreError>;

pub struct EventSourcingAgent<S: EventStore> {
    agent_id: String,
    store: S,
    version: u64,
    current_snapshot: Option<EventSnapshot>,
}

impl<S: EventStore> EventSourcingAgent<S> {
    pub fn new(agent_id: String, store: S) -> Self {
        Self {
            agent_id,
            store,
            version: 0,
            current_snapshot: None,
        }
    }

    pub async fn load_state(&mut self) -> EventResult<serde_json::Value> {
        if let Some(snapshot) = self.store.get_latest_snapshot(&self.agent_id)? {
            self.version = snapshot.version;
            self.current_snapshot = Some(snapshot.clone());
            return Ok(snapshot.state);
        }

        let events = self.store.get_events(&self.agent_id)?;
        if events.is_empty() {
            return Ok(serde_json::Value::Null);
        }

        let mut state = serde_json::Value::Null;
        for event in events {
            state = self.apply_event(state, &event);
            self.version += 1;
        }

        Ok(state)
    }

    pub fn record_event(&self, event: AgentEvent) -> EventResult<()> {
        self.store.append_event(&self.agent_id, event)?;
        Ok(())
    }

    pub fn create_snapshot(&self, state: serde_json::Value) -> EventResult<()> {
        let snapshot = EventSnapshot {
            snapshot_id: uuid::Uuid::now_v7().to_string(),
            agent_id: self.agent_id.clone(),
            version: self.version,
            state,
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        };
        self.store.create_snapshot(snapshot)
    }

    fn apply_event(&self, mut state: serde_json::Value, event: &AgentEvent) -> serde_json::Value {
        match event {
            AgentEvent::Thought(t) => {
                state[\"last_thought\"] = serde_json::Value::String(t.thought.clone());
            }
            AgentEvent::StateChange(s) => {
                if let Some(obj) = state.as_object_mut() {
                    obj.insert(s.state_key.clone(), s.new_value.clone().unwrap_or(serde_json::Value::Null));
                }
            }
            _ => {}
        }
        state
    }
}

pub struct EventProjector<S: EventStore> {
    store: S,
}

impl<S: EventStore> EventProjector<S> {
    pub fn new(store: S) -> Self {
        Self { store }
    }

    pub fn project_conversation(&self, agent_id: &str) -> EventResult<Vec<MessageEvent>> {
        let events = self.store.get_events(agent_id)?;
        let messages: Vec<MessageEvent> = events
            .into_iter()
            .filter_map(|e| {
                if let AgentEvent::Message(m) = e {
                    Some(m)
                } else {
                    None
                }
            })
            .collect();
        Ok(messages)
    }

    pub fn project_tool_usage(&self, agent_id: &str) -> EventResult<Vec<(ToolCallEvent, ToolResultEvent)>> {
        let events = self.store.get_events(agent_id)?;
        let mut tool_calls = Vec::new();
        let mut pending_call: Option<ToolCallEvent> = None;

        for event in events {
            match event {
                AgentEvent::ToolCall(call) => {
                    pending_call = Some(call);
                }
                AgentEvent::ToolResult(result) => {
                    if let Some(call) = pending_call.take() {
                        tool_calls.push((call, result));
                    }
                }
                _ => {}
            }
        }

        Ok(tool_calls)
    }
}
