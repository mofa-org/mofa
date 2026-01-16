//! dora-rs 适配层
//!
//! 该模块提供 MoFA 框架与 dora-rs 的集成适配，包括：
//! - DoraNode 封装：智能体生命周期管理
//! - DoraOperator 封装：插件能力抽象
//! - DoraDataflow 封装：多智能体协同数据流
//! - DoraChannel 封装：跨智能体通信通道
//! - DoraRuntime 封装：完整运行时支持（嵌入式/分布式）

pub mod channel;
mod dataflow;
mod error;
mod node;
mod operator;
pub mod runtime;

pub use channel::{ChannelConfig, ChannelManager, DoraChannel, MessageEnvelope};
pub use dataflow::{DataflowBuilder, DataflowConfig, DataflowState, DoraDataflow, NodeConnection};
pub use error::{DoraError, DoraResult};
pub use node::{DoraAgentNode, DoraNodeConfig, NodeEventLoop};
pub use operator::{
    DoraPluginOperator, MoFAOperator, OperatorChain, OperatorConfig, OperatorInput, OperatorOutput,
    PluginOperatorAdapter,
};

// 导出运行时支持（真实 dora API）
pub use runtime::{
    run_dataflow,
    run_dataflow_with_logs,
    // 运行时
    DataflowResult,
    DistributedConfig,
    DoraRuntime,
    DoraRuntimeBuilder,
    EmbeddedConfig,
    LogDestination,
    LogDestinationType,
    // 配置
    NodeResult,
    // 状态和结果
    RuntimeConfig,
    // 辅助函数
    RuntimeMode,
    RuntimeState,
};
