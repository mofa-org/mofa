use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowVersion {
    pub version: String,
    pub created_at: i64,
    pub changelog: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorHandlingConfig {
    pub error_strategy: ErrorStrategy,
    pub fallback_workflow: Option<String>,
    pub error_notification: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ErrorStrategy {
    Retry,
    Skip,
    Fallback,
    Stop,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualEditorMetadata {
    pub position_x: f64,
    pub position_y: f64,
    pub width: f64,
    pub height: f64,
    pub color: Option<String>,
    pub collapsed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubWorkflowTemplate {
    pub workflow_id: String,
    pub version: String,
    pub parameters: HashMap<String, ParameterDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterDefinition {
    pub param_type: ParameterType,
    pub required: bool,
    pub default: Option<serde_json::Value>,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ParameterType {
    String,
    Number,
    Boolean,
    Object,
    Array,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowMonitoringConfig {
    pub enabled: bool,
    pub metrics_interval_ms: u64,
    pub log_execution: bool,
    pub track_latency: bool,
    pub alert_on_failure: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConditionalBranch {
    pub condition: String,
    pub branch_id: String,
    pub branch_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvancedWorkflowConfig {
    pub versioning: Option<WorkflowVersion>,
    pub error_handling: Option<ErrorHandlingConfig>,
    pub monitoring: Option<WorkflowMonitoringConfig>,
    pub visual_metadata: Option<HashMap<String, VisualEditorMetadata>>,
    pub templates: Option<HashMap<String, SubWorkflowTemplate>>,
}

impl Default for WorkflowVersion {
    fn default() -> Self {
        Self {
            version: "1.0.0".to_string(),
            created_at: chrono::Utc::now().timestamp(),
            changelog: "Initial version".to_string(),
        }
    }
}

impl Default for ErrorHandlingConfig {
    fn default() -> Self {
        Self {
            error_strategy: ErrorStrategy::Retry,
            fallback_workflow: None,
            error_notification: false,
        }
    }
}

impl Default for WorkflowMonitoringConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            metrics_interval_ms: 5000,
            log_execution: true,
            track_latency: true,
            alert_on_failure: true,
        }
    }
}

impl Default for AdvancedWorkflowConfig {
    fn default() -> Self {
        Self {
            versioning: Some(WorkflowVersion::default()),
            error_handling: Some(ErrorHandlingConfig::default()),
            monitoring: Some(WorkflowMonitoringConfig::default()),
            visual_metadata: None,
            templates: None,
        }
    }
}
