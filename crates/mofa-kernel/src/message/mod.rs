use crate::agent::AgentState;
use serde::{Deserialize, Serialize};

// 流类型枚举
// Stream type enumeration
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[non_exhaustive]
pub enum StreamType {
    MessageStream, // 消息流 - 异步消息传递
    // Message stream - Asynchronous message passing
    DataStream, // 数据流 - 连续数据传递
    // Data stream - Continuous data transfer
    EventStream, // 事件流 - 离散事件传递
    // Event stream - Discrete event passing
    CommandStream, // 命令流 - 控制命令传递
                   // Command stream - Control command passing
}

// 智能体通信消息协议
// Agent communication message protocol
#[derive(Debug, Serialize, Deserialize, Clone)]
#[non_exhaustive]
pub enum AgentMessage {
    TaskRequest {
        task_id: String,
        content: String,
    },
    TaskResponse {
        task_id: String,
        result: String,
        status: TaskStatus,
    },
    StateSync {
        agent_id: String,
        state: AgentState,
    },
    Event(AgentEvent),

    // 流相关消息
    // Stream related messages
    StreamMessage {
        stream_id: String,
        message: Vec<u8>,
        sequence: u64,
    }, // 流消息
    // Stream message
    StreamControl {
        stream_id: String,
        command: StreamControlCommand,
        metadata: std::collections::HashMap<String, String>,
    }, // 流控制消息
       // Stream control message
}

// 任务状态枚举
// Task status enumeration
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[non_exhaustive]
pub enum TaskStatus {
    Success,
    Failed,
    Pending,
}

// Agent 可感知的事件类型
// Event types perceivable by the Agent
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub enum AgentEvent {
    TaskReceived(TaskRequest), // 任务事件
    // Task event
    TaskPreempted(String), // 任务被抢占事件（参数：被抢占的任务ID）
    // Task preempted event (Param: preempted task ID)
    Shutdown, // 框架关闭事件
    // Framework shutdown event
    Custom(String, Vec<u8>), // 自定义事件
    // Custom event

    // 流相关事件
    // Stream related events
    StreamMessage {
        stream_id: String,
        message: Vec<u8>,
        sequence: u64,
    }, // 流消息事件
    // Stream message event
    StreamCreated {
        stream_id: String,
        stream_type: StreamType,
        metadata: std::collections::HashMap<String, String>,
    }, // 流创建事件
    // Stream creation event
    StreamClosed {
        stream_id: String,
        reason: String,
    }, // 流关闭事件
    // Stream closure event
    StreamSubscription {
        stream_id: String,
        subscriber_id: String,
    }, // 流订阅事件
    // Stream subscription event
    StreamUnsubscription {
        stream_id: String,
        subscriber_id: String,
    }, // 流取消订阅事件
       // Stream unsubscription event
}

// 扩展 TaskRequest，增加优先级和调度元数据
// Extend TaskRequest with priority and scheduling metadata
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct TaskRequest {
    pub task_id: String,
    pub content: String,
    pub priority: TaskPriority,
    pub deadline: Option<std::time::Duration>, // 任务截止时间
    // Task deadline duration
    pub metadata: std::collections::HashMap<String, String>,
}

// 任务优先级与调度元数据
// Task priority and scheduling metadata
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TaskPriority {
    Critical = 0,
    Highest = 1,
    High = 2,
    Medium = 3,
    Normal = 4,
    Low = 5,
}

// 实现 PartialOrd 用于优先级排序（Urgent > High > Medium > Low）
// Implement PartialOrd for priority sorting (Urgent > High > Medium > Low)
impl PartialOrd for TaskPriority {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TaskPriority {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Lower discriminant = higher priority (Critical=0 is highest).
        // Reverse the natural ordering so that higher-priority sorts Greater.
        let lhs = self.clone() as u8;
        let rhs = other.clone() as u8;
        lhs.cmp(&rhs).reverse()
    }
}

// 流控制命令
// Stream control commands
#[derive(Debug, Serialize, Deserialize, Clone)]
#[non_exhaustive]
pub enum StreamControlCommand {
    Create(StreamType), // 创建流
    // Create stream
    Close(String), // 关闭流（原因）
    // Close stream (reason)
    Subscribe, // 订阅流
    // Subscribe to stream
    Unsubscribe, // 取消订阅
    // Unsubscribe from stream
    Pause, // 暂停流
    // Pause stream
    Resume, // 恢复流
    // Resume stream
    Seek(u64), // 跳转到指定序列位置（仅适用于可重放流）
               // Seek to sequence position (for replayable streams only)
}

// 调度状态，用于跟踪任务抢占情况
// Scheduling status for tracking task preemption
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum SchedulingStatus {
    Pending,
    Running,
    Preempted, // 被高优先级任务抢占
    // Preempted by a higher priority task
    Completed,
    Failed,
}
