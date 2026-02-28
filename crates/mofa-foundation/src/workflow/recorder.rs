use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowRecording {
    pub execution_id: Uuid,
    pub workflow_id: String,
    pub steps: Vec<ExecutionStep>,
    pub total_duration: Duration,
}

impl WorkflowRecording {
    pub fn new(execution_id: Uuid, workflow_id: String) -> Self {
        Self {
            execution_id,
            workflow_id,
            steps: Vec::new(),
            total_duration: Duration::default(),
        }
    }

    pub fn add_step(&mut self, step: ExecutionStep) {
        self.steps.push(step);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum StepStatus {
    Running,
    Success,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStep {
    pub node_id: String,
    pub input: Option<Value>,
    pub output: Option<Value>,
    pub timestamp: Duration, // Relative to start
    pub duration: Duration,
    pub status: StepStatus,
    pub error: Option<String>,
}

pub struct WorkflowRecorder {
    recording: WorkflowRecording,
    start_time: std::time::Instant,
}

impl WorkflowRecorder {
    pub fn new(execution_id: Uuid, workflow_id: String) -> Self {
        Self {
            recording: WorkflowRecording::new(execution_id, workflow_id),
            start_time: std::time::Instant::now(),
        }
    }

    pub fn record_step(&mut self, step: ExecutionStep) {
        self.recording.add_step(step);
    }

    pub fn finalize(&mut self) -> WorkflowRecording {
        self.recording.total_duration = self.start_time.elapsed();
        self.recording.clone()
    }

    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }
    
    pub fn get_recording(&self) -> &WorkflowRecording {
        &self.recording
    }
}
