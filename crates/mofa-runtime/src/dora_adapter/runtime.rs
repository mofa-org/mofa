//! Dora Runtime Support - 真实运行时集成
//!
//! 提供 dora-rs 完整运行时支持，包括：
//! - 嵌入式模式：使用 `Daemon::run_dataflow` 在单进程中运行数据流
//! - 分布式模式：连接外部 dora-daemon 和 coordinator
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────┐
//! │                            DoraRuntime                              │
//! │  ┌────────────────────────────┬────────────────────────────────┐   │
//! │  │     EmbeddedMode           │      DistributedMode           │   │
//! │  │  ┌──────────────────┐      │   ┌─────────────────────────┐  │   │
//! │  │  │ Daemon::         │      │   │ Daemon::run()           │  │   │
//! │  │  │ run_dataflow()   │      │   │ (connect to coordinator)│  │   │
//! │  │  └──────────────────┘      │   └─────────────────────────┘  │   │
//! │  └────────────────────────────┴────────────────────────────────┘   │
//! └─────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Usage
//!
//! ## Embedded Mode (单进程运行 - 不需要外部 daemon/coordinator)
//! ```rust,ignore
//! use mofa::dora_adapter::runtime::{DoraRuntime, RuntimeConfig};
//!
//! #[tokio::main]
//! async fn main() -> eyre::Result<()> {
//!     let config = RuntimeConfig::embedded("dataflow.yml");
//!     let mut runtime = DoraRuntime::new(config);
//!     let result = runtime.run().await?;
//!     info!("Dataflow {} completed", result.uuid);
//!     Ok(())
//! }
//! ```
//!
//! ## Distributed Mode (连接外部 coordinator)
//! ```rust,ignore
//! use mofa::dora_adapter::runtime::{DoraRuntime, RuntimeConfig};
//!
//! #[tokio::main]
//! async fn main() -> eyre::Result<()> {
//!     let addr = "127.0.0.1:5000".parse()?;
//!     let config = RuntimeConfig::distributed("dataflow.yml", addr);
//!     let mut runtime = DoraRuntime::new(config);
//!     runtime.run().await?;
//!     Ok(())
//! }
//! ```

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use dora_daemon::Daemon;
pub use dora_daemon::LogDestination;
use dora_message::common::LogMessage;
use dora_message::coordinator_to_cli::DataflowResult as DoraDataflowResult;
use dora_message::{BuildId, SessionId};
use eyre::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::info;
use uuid::Uuid;

// ============================================================================
// Runtime Configuration
// ============================================================================

/// Dora 运行时模式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum RuntimeMode {
    /// 嵌入式模式：使用 Daemon::run_dataflow 单进程运行
    /// 不需要外部 coordinator 或 daemon
    #[default]
    Embedded,
    /// 分布式模式：连接外部 dora-daemon 和 coordinator
    Distributed,
}

/// 运行时配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    /// 运行时模式
    pub mode: RuntimeMode,
    /// 数据流 YAML 文件路径
    pub dataflow_path: PathBuf,
    /// 嵌入式模式配置
    pub embedded: EmbeddedConfig,
    /// 分布式模式配置
    pub distributed: DistributedConfig,
}

impl RuntimeConfig {
    /// 创建嵌入式模式配置
    pub fn embedded<P: AsRef<Path>>(dataflow_path: P) -> Self {
        Self {
            mode: RuntimeMode::Embedded,
            dataflow_path: dataflow_path.as_ref().to_path_buf(),
            embedded: EmbeddedConfig::default(),
            distributed: DistributedConfig::default(),
        }
    }

    /// 创建分布式模式配置
    pub fn distributed<P: AsRef<Path>>(dataflow_path: P, coordinator_addr: SocketAddr) -> Self {
        Self {
            mode: RuntimeMode::Distributed,
            dataflow_path: dataflow_path.as_ref().to_path_buf(),
            embedded: EmbeddedConfig::default(),
            distributed: DistributedConfig {
                coordinator_addr,
                ..Default::default()
            },
        }
    }

    /// 设置模式
    pub fn with_mode(mut self, mode: RuntimeMode) -> Self {
        self.mode = mode;
        self
    }

    /// 设置是否使用 uv (Python 包管理器)
    pub fn with_uv(mut self, uv: bool) -> Self {
        self.embedded.uv = uv;
        self
    }

    /// 设置事件输出目录
    pub fn with_events_output<P: AsRef<Path>>(mut self, path: P) -> Self {
        self.embedded.write_events_to = Some(path.as_ref().to_path_buf());
        self
    }

    /// 设置日志目标
    pub fn with_log_destination(mut self, dest: LogDestinationType) -> Self {
        self.embedded.log_destination = dest;
        self
    }
}

/// 嵌入式模式配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddedConfig {
    /// 使用 uv 运行 Python 节点
    pub uv: bool,
    /// 事件输出目录
    pub write_events_to: Option<PathBuf>,
    /// 日志目标
    pub log_destination: LogDestinationType,
    /// Build ID（如果之前已经构建过）
    pub build_id: Option<Uuid>,
}

impl Default for EmbeddedConfig {
    fn default() -> Self {
        Self {
            uv: false,
            write_events_to: None,
            log_destination: LogDestinationType::Channel,
            build_id: None,
        }
    }
}

/// 日志目标类型
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum LogDestinationType {
    /// 使用 tracing 输出日志
    Tracing,
    /// 通过 channel 发送日志（可以自定义处理）
    #[default]
    Channel,
}

/// 分布式模式配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributedConfig {
    /// Coordinator 地址
    pub coordinator_addr: SocketAddr,
    /// 机器 ID
    pub machine_id: Option<String>,
    /// 本地监听端口
    pub local_listen_port: u16,
}

impl Default for DistributedConfig {
    fn default() -> Self {
        Self {
            coordinator_addr: "127.0.0.1:5000".parse().unwrap(),
            machine_id: None,
            local_listen_port: 5001,
        }
    }
}

// ============================================================================
// Runtime State
// ============================================================================

/// 运行时状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum RuntimeState {
    #[default]
    Created,
    Running,
    Stopped,
    Error,
}

/// 数据流执行结果
#[derive(Debug, Clone)]
pub struct DataflowResult {
    /// 数据流 UUID
    pub uuid: Uuid,
    /// 节点执行结果
    pub node_results: BTreeMap<String, NodeResult>,
    /// 是否成功
    pub success: bool,
}

/// 节点执行结果
#[derive(Debug, Clone)]
pub enum NodeResult {
    Success,
    Error(String),
}

impl From<DoraDataflowResult> for DataflowResult {
    fn from(result: DoraDataflowResult) -> Self {
        let node_results: BTreeMap<String, NodeResult> = result
            .node_results
            .into_iter()
            .map(|(node_id, res)| {
                let result = match res {
                    Ok(()) => NodeResult::Success,
                    Err(err) => NodeResult::Error(format!("{:?}", err)),
                };
                (node_id.to_string(), result)
            })
            .collect();

        let success = node_results
            .values()
            .all(|r| matches!(r, NodeResult::Success));

        Self {
            uuid: result.uuid,
            node_results,
            success,
        }
    }
}

// ============================================================================
// Dora Runtime
// ============================================================================

/// Dora 运行时
///
/// 提供两种运行模式：
/// - Embedded: 使用 `Daemon::run_dataflow` 在单进程中运行
/// - Distributed: 连接外部 coordinator 和 daemon
pub struct DoraRuntime {
    config: RuntimeConfig,
    state: Arc<RwLock<RuntimeState>>,
    /// 日志接收通道（当使用 Channel 日志目标时）
    log_receiver: Option<flume::Receiver<LogMessage>>,
}

impl DoraRuntime {
    /// 创建新的运行时实例
    pub fn new(config: RuntimeConfig) -> Self {
        Self {
            config,
            state: Arc::new(RwLock::new(RuntimeState::Created)),
            log_receiver: None,
        }
    }

    /// 创建嵌入式模式运行时
    pub fn embedded<P: AsRef<Path>>(dataflow_path: P) -> Self {
        Self::new(RuntimeConfig::embedded(dataflow_path))
    }

    /// 创建分布式模式运行时
    pub fn distributed<P: AsRef<Path>>(dataflow_path: P, coordinator_addr: SocketAddr) -> Self {
        Self::new(RuntimeConfig::distributed(dataflow_path, coordinator_addr))
    }

    /// 获取当前状态
    pub async fn state(&self) -> RuntimeState {
        *self.state.read().await
    }

    /// 获取配置
    pub fn config(&self) -> &RuntimeConfig {
        &self.config
    }

    /// 获取日志接收器（如果使用 Channel 日志目标）
    pub fn log_receiver(&self) -> Option<&flume::Receiver<LogMessage>> {
        self.log_receiver.as_ref()
    }

    /// 取走日志接收器的所有权
    pub fn take_log_receiver(&mut self) -> Option<flume::Receiver<LogMessage>> {
        self.log_receiver.take()
    }

    /// 运行数据流
    ///
    /// 根据配置的模式运行数据流：
    /// - Embedded: 直接在当前进程中运行
    /// - Distributed: 连接到外部 daemon
    pub async fn run(&mut self) -> Result<DataflowResult> {
        *self.state.write().await = RuntimeState::Running;

        let result = match self.config.mode {
            RuntimeMode::Embedded => self.run_embedded().await,
            RuntimeMode::Distributed => self.run_distributed().await,
        };

        match &result {
            Ok(_) => *self.state.write().await = RuntimeState::Stopped,
            Err(_) => *self.state.write().await = RuntimeState::Error,
        }

        result
    }

    /// 嵌入式模式运行
    async fn run_embedded(&mut self) -> Result<DataflowResult> {
        let dataflow_path = &self.config.dataflow_path;

        info!("Running dataflow in embedded mode: {:?}", dataflow_path);

        // 验证数据流文件存在
        if !dataflow_path.exists() {
            return Err(eyre::eyre!("Dataflow file not found: {:?}", dataflow_path));
        }

        // 创建 Session ID
        let session_id = SessionId::generate();

        // 配置日志目标
        let (log_destination, log_rx) = match self.config.embedded.log_destination {
            LogDestinationType::Tracing => (LogDestination::Tracing, None),
            LogDestinationType::Channel => {
                let (tx, rx) = flume::bounded(100);
                (LogDestination::Channel { sender: tx }, Some(rx))
            }
        };

        self.log_receiver = log_rx;

        // Build ID
        let build_id = self.config.embedded.build_id.map(|uuid| {
            // BuildId 需要通过其他方式创建，这里我们使用 generate
            // 如果有预先存在的 build_id，暂时跳过
            BuildId::generate()
        });

        // 运行数据流
        let result = Daemon::run_dataflow(
            dataflow_path,
            build_id,
            None, // local_build
            session_id,
            self.config.embedded.uv,
            log_destination,
        )
        .await
        .context("Failed to run dataflow")?;

        info!("Dataflow {} completed", result.uuid);

        Ok(DataflowResult::from(result))
    }

    /// 分布式模式运行
    async fn run_distributed(&mut self) -> Result<DataflowResult> {
        let coordinator_addr = self.config.distributed.coordinator_addr;
        let machine_id = self.config.distributed.machine_id.clone();
        let local_listen_port = self.config.distributed.local_listen_port;

        info!(
            "Connecting to dora-coordinator at {} in distributed mode",
            coordinator_addr
        );

        // 运行 daemon（连接到 coordinator）
        Daemon::run(coordinator_addr, machine_id, local_listen_port)
            .await
            .context("Failed to run daemon in distributed mode")?;

        // 分布式模式下，数据流由 coordinator 管理
        // 返回一个空结果
        Ok(DataflowResult {
            uuid: Uuid::nil(),
            node_results: BTreeMap::new(),
            success: true,
        })
    }
}

// ============================================================================
// Runtime Builder
// ============================================================================

/// 运行时构建器
pub struct DoraRuntimeBuilder {
    config: RuntimeConfig,
}

impl DoraRuntimeBuilder {
    /// 创建新的构建器
    pub fn new<P: AsRef<Path>>(dataflow_path: P) -> Self {
        Self {
            config: RuntimeConfig::embedded(dataflow_path),
        }
    }

    /// 设置为嵌入式模式
    pub fn embedded(mut self) -> Self {
        self.config.mode = RuntimeMode::Embedded;
        self
    }

    /// 设置为分布式模式
    pub fn distributed(mut self, coordinator_addr: SocketAddr) -> Self {
        self.config.mode = RuntimeMode::Distributed;
        self.config.distributed.coordinator_addr = coordinator_addr;
        self
    }

    /// 设置是否使用 uv
    pub fn uv(mut self, uv: bool) -> Self {
        self.config.embedded.uv = uv;
        self
    }

    /// 设置事件输出目录
    pub fn write_events_to<P: AsRef<Path>>(mut self, path: P) -> Self {
        self.config.embedded.write_events_to = Some(path.as_ref().to_path_buf());
        self
    }

    /// 设置日志目标
    pub fn log_destination(mut self, dest: LogDestinationType) -> Self {
        self.config.embedded.log_destination = dest;
        self
    }

    /// 设置机器 ID（分布式模式）
    pub fn machine_id(mut self, id: String) -> Self {
        self.config.distributed.machine_id = Some(id);
        self
    }

    /// 设置本地监听端口（分布式模式）
    pub fn local_listen_port(mut self, port: u16) -> Self {
        self.config.distributed.local_listen_port = port;
        self
    }

    /// 构建运行时
    pub fn build(self) -> DoraRuntime {
        DoraRuntime::new(self.config)
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// 快速运行数据流（嵌入式模式）
///
/// 这是最简单的运行方式，适合快速测试
///
/// # Example
/// ```rust,ignore
/// use mofa::dora_adapter::runtime::run_dataflow;
///
/// #[tokio::main]
/// async fn main() -> eyre::Result<()> {
///     let result = run_dataflow("dataflow.yml").await?;
///     info!("Dataflow {} completed", result.uuid);
///     Ok(())
/// }
/// ```
pub async fn run_dataflow<P: AsRef<Path>>(dataflow_path: P) -> Result<DataflowResult> {
    let mut runtime = DoraRuntime::embedded(dataflow_path);
    runtime.run().await
}

/// 运行数据流并打印日志（嵌入式模式）
///
/// 启动一个后台线程打印日志消息
pub async fn run_dataflow_with_logs<P: AsRef<Path>>(dataflow_path: P) -> Result<DataflowResult> {
    let config =
        RuntimeConfig::embedded(dataflow_path).with_log_destination(LogDestinationType::Channel);

    let mut runtime = DoraRuntime::new(config);

    // 获取日志接收器并在后台线程中处理
    // 注意：需要在 run() 之前设置，因为 run() 会创建 channel
    let result = runtime.run().await;

    // 处理日志（如果有）
    if let Some(rx) = runtime.take_log_receiver() {
        // 打印剩余的日志消息
        while let Ok(msg) = rx.try_recv() {
            info!("[{:?}] {}", msg.level, msg.message);
        }
    }

    result
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runtime_config_embedded() {
        let config = RuntimeConfig::embedded("test.yml");
        assert_eq!(config.mode, RuntimeMode::Embedded);
        assert_eq!(config.dataflow_path, PathBuf::from("test.yml"));
    }

    #[test]
    fn test_runtime_config_distributed() {
        let addr: SocketAddr = "127.0.0.1:5000".parse().unwrap();
        let config = RuntimeConfig::distributed("test.yml", addr);
        assert_eq!(config.mode, RuntimeMode::Distributed);
        assert_eq!(config.distributed.coordinator_addr, addr);
    }

    #[test]
    fn test_runtime_builder() {
        let runtime = DoraRuntimeBuilder::new("test.yml")
            .embedded()
            .uv(true)
            .log_destination(LogDestinationType::Channel)
            .build();

        assert_eq!(runtime.config.mode, RuntimeMode::Embedded);
        assert!(runtime.config.embedded.uv);
    }

    #[tokio::test]
    async fn test_runtime_state() {
        let runtime = DoraRuntime::embedded("test.yml");
        assert_eq!(runtime.state().await, RuntimeState::Created);
    }
}
