//! 工作流状态管理
//!
//! 管理工作流执行过程中的状态和数据传递

use serde::{Deserialize, Serialize};
use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 工作流数据值
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WorkflowValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Bytes(Vec<u8>),
    List(Vec<WorkflowValue>),
    Map(HashMap<String, WorkflowValue>),
    Json(serde_json::Value),
}

impl WorkflowValue {
    pub fn is_null(&self) -> bool {
        matches!(self, WorkflowValue::Null)
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            WorkflowValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            WorkflowValue::Int(i) => Some(*i),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            WorkflowValue::Float(f) => Some(*f),
            WorkflowValue::Int(i) => Some(*i as f64),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            WorkflowValue::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            WorkflowValue::Bytes(b) => Some(b),
            _ => None,
        }
    }

    pub fn as_list(&self) -> Option<&Vec<WorkflowValue>> {
        match self {
            WorkflowValue::List(l) => Some(l),
            _ => None,
        }
    }

    pub fn as_map(&self) -> Option<&HashMap<String, WorkflowValue>> {
        match self {
            WorkflowValue::Map(m) => Some(m),
            _ => None,
        }
    }

    pub fn as_json(&self) -> Option<&serde_json::Value> {
        match self {
            WorkflowValue::Json(j) => Some(j),
            _ => None,
        }
    }
}

impl From<bool> for WorkflowValue {
    fn from(v: bool) -> Self {
        WorkflowValue::Bool(v)
    }
}

impl From<i64> for WorkflowValue {
    fn from(v: i64) -> Self {
        WorkflowValue::Int(v)
    }
}

impl From<i32> for WorkflowValue {
    fn from(v: i32) -> Self {
        WorkflowValue::Int(v as i64)
    }
}

impl From<f64> for WorkflowValue {
    fn from(v: f64) -> Self {
        WorkflowValue::Float(v)
    }
}

impl From<String> for WorkflowValue {
    fn from(v: String) -> Self {
        WorkflowValue::String(v)
    }
}

impl From<&str> for WorkflowValue {
    fn from(v: &str) -> Self {
        WorkflowValue::String(v.to_string())
    }
}

impl From<Vec<u8>> for WorkflowValue {
    fn from(v: Vec<u8>) -> Self {
        WorkflowValue::Bytes(v)
    }
}

impl From<serde_json::Value> for WorkflowValue {
    fn from(v: serde_json::Value) -> Self {
        WorkflowValue::Json(v)
    }
}

/// 节点执行状态
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeStatus {
    /// 等待执行
    Pending,
    /// 等待依赖完成
    Waiting,
    /// 正在执行
    Running,
    /// 执行成功
    Completed,
    /// 执行失败
    Failed(String),
    /// 已跳过（条件不满足）
    Skipped,
    /// 已取消
    Cancelled,
}

impl NodeStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            NodeStatus::Completed
                | NodeStatus::Failed(_)
                | NodeStatus::Skipped
                | NodeStatus::Cancelled
        )
    }

    pub fn is_success(&self) -> bool {
        matches!(self, NodeStatus::Completed | NodeStatus::Skipped)
    }
}

/// 工作流执行状态
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkflowStatus {
    /// 未开始
    NotStarted,
    /// 正在运行
    Running,
    /// 已暂停
    Paused,
    /// 已完成
    Completed,
    /// 失败
    Failed(String),
    /// 已取消
    Cancelled,
}

/// 节点执行结果
#[derive(Debug, Clone)]
pub struct NodeResult {
    /// 节点 ID
    pub node_id: String,
    /// 执行状态
    pub status: NodeStatus,
    /// 输出数据
    pub output: WorkflowValue,
    /// 执行时长（毫秒）
    pub duration_ms: u64,
    /// 重试次数
    pub retry_count: u32,
    /// 错误信息
    pub error: Option<String>,
}

impl NodeResult {
    pub fn success(node_id: &str, output: WorkflowValue, duration_ms: u64) -> Self {
        Self {
            node_id: node_id.to_string(),
            status: NodeStatus::Completed,
            output,
            duration_ms,
            retry_count: 0,
            error: None,
        }
    }

    pub fn failed(node_id: &str, error: &str, duration_ms: u64) -> Self {
        Self {
            node_id: node_id.to_string(),
            status: NodeStatus::Failed(error.to_string()),
            output: WorkflowValue::Null,
            duration_ms,
            retry_count: 0,
            error: Some(error.to_string()),
        }
    }

    pub fn skipped(node_id: &str) -> Self {
        Self {
            node_id: node_id.to_string(),
            status: NodeStatus::Skipped,
            output: WorkflowValue::Null,
            duration_ms: 0,
            retry_count: 0,
            error: None,
        }
    }
}

/// 工作流上下文 - 在节点间传递数据
pub struct WorkflowContext {
    /// 工作流 ID
    pub workflow_id: String,
    /// 执行 ID（每次运行唯一）
    pub execution_id: String,
    /// 输入数据
    input: Arc<RwLock<WorkflowValue>>,
    /// 节点输出存储
    node_outputs: Arc<RwLock<HashMap<String, WorkflowValue>>>,
    /// 节点状态
    node_statuses: Arc<RwLock<HashMap<String, NodeStatus>>>,
    /// 全局变量
    variables: Arc<RwLock<HashMap<String, WorkflowValue>>>,
    /// 自定义数据存储
    custom_data: Arc<RwLock<HashMap<String, Box<dyn Any + Send + Sync>>>>,
    /// 检查点数据
    checkpoints: Arc<RwLock<Vec<CheckpointData>>>,
}

impl WorkflowContext {
    pub fn new(workflow_id: &str) -> Self {
        Self {
            workflow_id: workflow_id.to_string(),
            execution_id: uuid::Uuid::now_v7().to_string(),
            input: Arc::new(RwLock::new(WorkflowValue::Null)),
            node_outputs: Arc::new(RwLock::new(HashMap::new())),
            node_statuses: Arc::new(RwLock::new(HashMap::new())),
            variables: Arc::new(RwLock::new(HashMap::new())),
            custom_data: Arc::new(RwLock::new(HashMap::new())),
            checkpoints: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// 设置工作流输入
    pub async fn set_input(&self, input: WorkflowValue) {
        let mut i = self.input.write().await;
        *i = input;
    }

    /// 获取工作流输入
    pub async fn get_input(&self) -> WorkflowValue {
        self.input.read().await.clone()
    }

    /// 设置节点输出
    pub async fn set_node_output(&self, node_id: &str, output: WorkflowValue) {
        let mut outputs = self.node_outputs.write().await;
        outputs.insert(node_id.to_string(), output);
    }

    /// 获取节点输出
    pub async fn get_node_output(&self, node_id: &str) -> Option<WorkflowValue> {
        let outputs = self.node_outputs.read().await;
        outputs.get(node_id).cloned()
    }

    /// 获取多个节点的输出
    pub async fn get_node_outputs(&self, node_ids: &[&str]) -> HashMap<String, WorkflowValue> {
        let outputs = self.node_outputs.read().await;
        node_ids
            .iter()
            .filter_map(|id| outputs.get(*id).map(|v| (id.to_string(), v.clone())))
            .collect()
    }

    /// 设置节点状态
    pub async fn set_node_status(&self, node_id: &str, status: NodeStatus) {
        let mut statuses = self.node_statuses.write().await;
        statuses.insert(node_id.to_string(), status);
    }

    /// 获取节点状态
    pub async fn get_node_status(&self, node_id: &str) -> Option<NodeStatus> {
        let statuses = self.node_statuses.read().await;
        statuses.get(node_id).cloned()
    }

    /// 获取所有节点状态
    pub async fn get_all_node_statuses(&self) -> HashMap<String, NodeStatus> {
        self.node_statuses.read().await.clone()
    }

    /// 设置变量
    pub async fn set_variable(&self, name: &str, value: WorkflowValue) {
        let mut vars = self.variables.write().await;
        vars.insert(name.to_string(), value);
    }

    /// 获取变量
    pub async fn get_variable(&self, name: &str) -> Option<WorkflowValue> {
        let vars = self.variables.read().await;
        vars.get(name).cloned()
    }

    /// 设置自定义数据
    pub async fn set_custom<T: Send + Sync + 'static>(&self, key: &str, value: T) {
        let mut data = self.custom_data.write().await;
        data.insert(key.to_string(), Box::new(value));
    }

    /// 获取自定义数据
    pub async fn get_custom<T: Clone + Send + Sync + 'static>(&self, key: &str) -> Option<T> {
        let data = self.custom_data.read().await;
        data.get(key).and_then(|v| v.downcast_ref::<T>().cloned())
    }

    /// 创建检查点
    pub async fn create_checkpoint(&self, label: &str) {
        let checkpoint = CheckpointData {
            label: label.to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            node_outputs: self.node_outputs.read().await.clone(),
            node_statuses: self.node_statuses.read().await.clone(),
            variables: self.variables.read().await.clone(),
        };
        let mut checkpoints = self.checkpoints.write().await;
        checkpoints.push(checkpoint);
    }

    /// 恢复到检查点
    pub async fn restore_checkpoint(&self, label: &str) -> bool {
        let checkpoints = self.checkpoints.read().await;
        let checkpoint = checkpoints.iter().rev().find(|c| c.label == label).cloned();
        drop(checkpoints);

        if let Some(checkpoint) = checkpoint {
            let mut outputs = self.node_outputs.write().await;
            *outputs = checkpoint.node_outputs.clone();
            drop(outputs);

            let mut statuses = self.node_statuses.write().await;
            *statuses = checkpoint.node_statuses.clone();
            drop(statuses);

            let mut vars = self.variables.write().await;
            *vars = checkpoint.variables.clone();

            true
        } else {
            false
        }
    }

    /// 获取所有检查点标签
    pub async fn list_checkpoints(&self) -> Vec<String> {
        let checkpoints = self.checkpoints.read().await;
        checkpoints.iter().map(|c| c.label.clone()).collect()
    }
}

impl Clone for WorkflowContext {
    fn clone(&self) -> Self {
        Self {
            workflow_id: self.workflow_id.clone(),
            execution_id: self.execution_id.clone(),
            input: self.input.clone(),
            node_outputs: self.node_outputs.clone(),
            node_statuses: self.node_statuses.clone(),
            variables: self.variables.clone(),
            custom_data: self.custom_data.clone(),
            checkpoints: self.checkpoints.clone(),
        }
    }
}

/// 检查点数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointData {
    /// 检查点标签
    pub label: String,
    /// 创建时间戳
    pub timestamp: u64,
    /// 节点输出快照
    pub node_outputs: HashMap<String, WorkflowValue>,
    /// 节点状态快照
    pub node_statuses: HashMap<String, NodeStatus>,
    /// 变量快照
    pub variables: HashMap<String, WorkflowValue>,
}

/// 工作流执行历史记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionRecord {
    /// 执行 ID
    pub execution_id: String,
    /// 工作流 ID
    pub workflow_id: String,
    /// 开始时间
    pub started_at: u64,
    /// 结束时间
    pub ended_at: Option<u64>,
    /// 最终状态
    pub status: WorkflowStatus,
    /// 节点执行记录
    pub node_records: Vec<NodeExecutionRecord>,
    /// Workflow Execution Recording (Time Travel Debugger)
    pub recording: Option<crate::workflow::recorder::WorkflowRecording>,
}

/// 节点执行记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeExecutionRecord {
    /// 节点 ID
    pub node_id: String,
    /// 开始时间
    pub started_at: u64,
    /// 结束时间
    pub ended_at: u64,
    /// 执行状态
    pub status: NodeStatus,
    /// 重试次数
    pub retry_count: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_workflow_context() {
        let ctx = WorkflowContext::new("test_workflow");

        // 测试输入
        ctx.set_input(WorkflowValue::String("test input".to_string()))
            .await;
        let input = ctx.get_input().await;
        assert_eq!(input.as_str(), Some("test input"));

        // 测试节点输出
        ctx.set_node_output("node1", WorkflowValue::Int(42)).await;
        let output = ctx.get_node_output("node1").await;
        assert_eq!(output.unwrap().as_i64(), Some(42));

        // 测试变量
        ctx.set_variable("counter", WorkflowValue::Int(0)).await;
        let var = ctx.get_variable("counter").await;
        assert_eq!(var.unwrap().as_i64(), Some(0));

        // 测试检查点
        ctx.create_checkpoint("before_loop").await;
        ctx.set_variable("counter", WorkflowValue::Int(10)).await;
        ctx.restore_checkpoint("before_loop").await;
        let var = ctx.get_variable("counter").await;
        assert_eq!(var.unwrap().as_i64(), Some(0));
    }

    #[test]
    fn test_workflow_value_conversions() {
        let v: WorkflowValue = 42i64.into();
        assert_eq!(v.as_i64(), Some(42));

        let v: WorkflowValue = "hello".into();
        assert_eq!(v.as_str(), Some("hello"));

        let v: WorkflowValue = true.into();
        assert_eq!(v.as_bool(), Some(true));

        let v: WorkflowValue = 3.14f64.into();
        assert_eq!(v.as_f64(), Some(3.14));
    }
}
