//! 秘书核心引擎实现
//! Secretary core engine implementation
//!
//! 提供秘书Agent的核心事件循环机制。这是一个轻量级的引擎，
//! Provides the core event loop mechanism for the Secretary Agent. This is a lightweight engine,
//! 只负责事件循环和消息传递，具体的行为逻辑由 `SecretaryBehavior` 实现。
//! only responsible for the event loop and message passing; specific behavior logic is implemented by `SecretaryBehavior`.
//!
//! ## 设计理念
//! ## Design Philosophy
//!
//! - 最小化核心：只提供事件循环和连接管理
//! - Minimal core: provides only event loops and connection management
//! - 行为可插拔：通过 `SecretaryBehavior` trait 定义秘书行为
//! - Pluggable behavior: secretary behavior defined via the `SecretaryBehavior` trait
//! - 连接抽象：支持多种连接方式（通道、WebSocket 等）
//! - Connection abstraction: supports multiple connection methods (Channels, WebSocket, etc.)

use mofa_kernel::agent::types::error::{GlobalError, GlobalResult};
use tokio::sync::mpsc;
use tracing::Instrument;

// 使用 mofa-kernel 的核心抽象
// Use core abstractions from mofa-kernel
use mofa_kernel::agent::secretary::{SecretaryBehavior, SecretaryContext, UserConnection};

// =============================================================================
// Core 配置与状态 (Foundation 实现)
// Core Configuration and State (Foundation Implementation)
// =============================================================================

/// 秘书核心配置
/// Secretary core configuration
#[derive(Debug, Clone)]
pub struct SecretaryCoreConfig {
    /// 事件循环轮询间隔（毫秒）
    /// Event loop polling interval (milliseconds)
    pub poll_interval_ms: u64,

    /// 是否在启动时发送欢迎消息
    /// Whether to send a welcome message upon startup
    pub send_welcome: bool,

    /// 是否启用定时检查
    /// Whether to enable periodic checks
    pub enable_periodic_check: bool,

    /// 定时检查间隔（毫秒）
    /// Periodic check interval (milliseconds)
    pub periodic_check_interval_ms: u64,

    /// 最大连续错误次数（超过后停止）
    /// Maximum consecutive error count (stops after exceeding)
    pub max_consecutive_errors: u32,
}

impl Default for SecretaryCoreConfig {
    fn default() -> Self {
        Self {
            poll_interval_ms: 100,
            send_welcome: true,
            enable_periodic_check: true,
            periodic_check_interval_ms: 1000,
            max_consecutive_errors: 10,
        }
    }
}

/// 秘书核心状态
/// Secretary core state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreState {
    /// 初始化中
    /// Initializing
    Initializing,
    /// 运行中
    /// Running
    Running,
    /// 暂停
    /// Paused
    Paused,
    /// 已停止
    /// Stopped
    Stopped,
}

/// 秘书控制句柄
/// Secretary control handle
///
/// 用于从外部控制秘书的运行状态。
/// Used to control the secretary's running state from the outside.
#[derive(Clone)]
pub struct SecretaryHandle {
    /// 是否运行中
    /// Whether it is running
    running: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// 是否暂停
    /// Whether it is paused
    paused: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// 停止信号发送器
    /// Stop signal transmitter
    stop_tx: mpsc::Sender<()>,
}

impl SecretaryHandle {
    /// 创建新的控制句柄
    /// Create a new control handle
    pub fn new(stop_tx: mpsc::Sender<()>) -> Self {
        Self {
            running: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            paused: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            stop_tx,
        }
    }

    /// 获取 running 标志的克隆
    /// Get a clone of the running flag
    pub fn running_flag(&self) -> std::sync::Arc<std::sync::atomic::AtomicBool> {
        self.running.clone()
    }

    /// 获取 paused 标志的克隆
    /// Get a clone of the paused flag
    pub fn paused_flag(&self) -> std::sync::Arc<std::sync::atomic::AtomicBool> {
        self.paused.clone()
    }

    /// 检查是否运行中
    /// Check if running
    pub fn is_running(&self) -> bool {
        self.running.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// 检查是否暂停
    /// Check if paused
    pub fn is_paused(&self) -> bool {
        self.paused.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// 设置运行状态
    /// Set running status
    pub fn set_running(&self, running: bool) {
        self.running
            .store(running, std::sync::atomic::Ordering::SeqCst);
    }

    /// 暂停秘书
    /// Pause the secretary
    pub fn pause(&self) {
        self.paused.store(true, std::sync::atomic::Ordering::SeqCst);
    }

    /// 恢复秘书
    /// Resume the secretary
    pub fn resume(&self) {
        self.paused
            .store(false, std::sync::atomic::Ordering::SeqCst);
    }

    /// 停止秘书
    /// Stop the secretary
    pub async fn stop(&self) {
        self.running
            .store(false, std::sync::atomic::Ordering::SeqCst);
        let _ = self.stop_tx.send(()).await;
    }
}

// =============================================================================
// 秘书核心引擎
// Secretary Core Engine
// =============================================================================

/// 秘书核心引擎
/// Secretary Core Engine
///
/// 这是框架的核心组件，负责运行事件循环并协调各个组件。
/// This is the core component of the framework, responsible for running the event loop and coordinating components.
/// 秘书的具体行为由 `SecretaryBehavior` 实现定义。
/// Specific secretary behavior is defined by the `SecretaryBehavior` implementation.
///
/// # 类型参数
/// # Type Parameters
///
/// - `B`: 秘书行为实现
/// - `B`: Secretary behavior implementation
///
/// # 示例
/// # Example
///
/// ```rust,ignore
/// // 1. 实现 SecretaryBehavior
/// // 1. Implement SecretaryBehavior
/// struct MySecretary { /* ... */ }
/// impl SecretaryBehavior for MySecretary { /* ... */ }
///
/// // 2. 创建核心引擎
/// // 2. Create core engine
/// let core = SecretaryCore::new(MySecretary::new());
///
/// // 3. 创建连接
/// // 3. Create connection
/// let (conn, input_tx, output_rx) = ChannelConnection::new_pair(32);
///
/// // 4. 启动事件循环
/// // 4. Start event loop
/// let handle = core.start(conn).await;
///
/// // 5. 发送输入
/// // 5. Send input
/// input_tx.send(MyInput::Text("hello".to_string())).await?;
///
/// // 6. 接收输出
/// // 6. Receive output
/// while let Some(output) = output_rx.recv().await {
///     info!("Output: {:?}", output);
/// }
/// ```
pub struct SecretaryCore<B>
where
    B: SecretaryBehavior,
{
    /// 秘书行为实现
    /// Secretary behavior implementation
    behavior: B,

    /// 配置
    /// Configuration
    config: SecretaryCoreConfig,
}

impl<B> SecretaryCore<B>
where
    B: SecretaryBehavior + 'static,
{
    /// 创建新的秘书核心
    /// Create a new secretary core
    pub fn new(behavior: B) -> Self {
        Self {
            behavior,
            config: SecretaryCoreConfig::default(),
        }
    }

    /// 使用自定义配置创建
    /// Create with custom configuration
    pub fn with_config(behavior: B, config: SecretaryCoreConfig) -> Self {
        Self { behavior, config }
    }

    /// 获取配置的可变引用
    /// Get a mutable reference to the configuration
    pub fn config_mut(&mut self) -> &mut SecretaryCoreConfig {
        &mut self.config
    }

    /// 获取行为实现的引用
    /// Get a reference to the behavior implementation
    pub fn behavior(&self) -> &B {
        &self.behavior
    }

    /// 启动秘书（异步任务）
    /// Start secretary (asynchronous task)
    ///
    /// 返回一个控制句柄和一个 JoinHandle。
    /// Returns a control handle and a JoinHandle.
    pub async fn start<C>(
        self,
        connection: C,
    ) -> (SecretaryHandle, tokio::task::JoinHandle<GlobalResult<()>>)
    where
        C: UserConnection<Input = B::Input, Output = B::Output> + 'static,
    {
        let (stop_tx, stop_rx) = mpsc::channel(1);
        let handle = SecretaryHandle::new(stop_tx);
        let handle_clone = handle.clone();

        let span = tracing::info_span!("secretary.event_loop");
        let join_handle =
            tokio::spawn(
                async move { self.run_event_loop(connection, handle_clone, stop_rx).await }
                    .instrument(span),
            );

        (handle, join_handle)
    }

    /// 同步启动秘书（阻塞当前任务）
    /// Start secretary synchronously (blocking current task)
    pub async fn run<C>(self, connection: C) -> GlobalResult<()>
    where
        C: UserConnection<Input = B::Input, Output = B::Output> + 'static,
    {
        let (stop_tx, stop_rx) = mpsc::channel(1);
        let handle = SecretaryHandle::new(stop_tx);
        self.run_event_loop(connection, handle, stop_rx).await
    }

    /// 运行事件循环
    /// Run event loop
    async fn run_event_loop<C>(
        self,
        connection: C,
        handle: SecretaryHandle,
        mut stop_rx: mpsc::Receiver<()>,
    ) -> GlobalResult<()>
    where
        C: UserConnection<Input = B::Input, Output = B::Output>,
    {
        // 标记为运行中
        // Mark as running
        handle.set_running(true);

        // 创建上下文
        // Create context
        let mut ctx = SecretaryContext::new(self.behavior.initial_state());

        // 发送欢迎消息
        // Send welcome message
        if self.config.send_welcome
            && let Some(welcome) = self.behavior.welcome_message()
            && let Err(e) = connection.send(welcome).await
        {
            tracing::warn!("Failed to send welcome message: {}", e);
        }

        let mut consecutive_errors = 0u32;
        let mut last_periodic_check = std::time::Instant::now();

        // 主事件循环
        // Main event loop
        loop {
            // 检查停止信号
            // Check for stop signal
            tokio::select! {
                _ = stop_rx.recv() => {
                    tracing::info!("Received stop signal");
                    break;
                }
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(self.config.poll_interval_ms)) => {
                    // 继续循环
                    // Continue loop
                }
            }

            // 检查连接状态
            // Check connection status
            if !connection.is_connected() {
                tracing::info!("Connection closed");
                break;
            }

            // 检查是否暂停
            // Check if paused
            if handle.is_paused() {
                continue;
            }

            // 尝试接收用户输入
            // Attempt to receive user input
            match connection.try_receive().await {
                Ok(Some(input)) => {
                    consecutive_errors = 0;

                    // 处理输入
                    // Handle input
                    match self.behavior.handle_input(input, &mut ctx).await {
                        Ok(outputs) => {
                            for output in outputs {
                                if let Err(e) = connection.send(output).await {
                                    tracing::error!("Failed to send output: {}", e);
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!("Error handling input: {}", e);

                            // 尝试发送错误响应
                            // Attempt to send error response
                            if let Some(error_output) = self.behavior.handle_error(&e) {
                                let _ = connection.send(error_output).await;
                            }
                        }
                    }
                }
                Ok(None) => {
                    // 没有输入，执行定时检查
                    // No input, perform periodic check
                    if self.config.enable_periodic_check {
                        let elapsed = last_periodic_check.elapsed().as_millis() as u64;
                        if elapsed >= self.config.periodic_check_interval_ms {
                            last_periodic_check = std::time::Instant::now();

                            match self.behavior.periodic_check(&mut ctx).await {
                                Ok(outputs) => {
                                    for output in outputs {
                                        if let Err(e) = connection.send(output).await {
                                            tracing::warn!("Failed to send periodic output: {}", e);
                                        }
                                    }
                                }
                                Err(e) => {
                                    tracing::warn!("Periodic check error: {}", e);
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("Error receiving input: {}", e);
                    consecutive_errors += 1;

                    if consecutive_errors >= self.config.max_consecutive_errors {
                        tracing::error!(
                            "Too many consecutive errors ({}), stopping",
                            consecutive_errors
                        );
                        break;
                    }
                }
            }
        }

        // 清理
        // Cleanup
        handle.set_running(false);

        // 调用断开连接回调
        // Call disconnect callback
        if let Err(e) = self.behavior.on_disconnect(&mut ctx).await {
            tracing::warn!("Error in on_disconnect: {}", e);
        }

        Ok(())
    }
}

// =============================================================================
// 秘书核心构建器
// Secretary Core Builder
// =============================================================================

/// 秘书核心构建器
/// Secretary Core Builder
pub struct SecretaryCoreBuilder<B>
where
    B: SecretaryBehavior,
{
    behavior: B,
    config: SecretaryCoreConfig,
}

impl<B> SecretaryCoreBuilder<B>
where
    B: SecretaryBehavior + 'static,
{
    /// 创建构建器
    /// Create builder
    pub fn new(behavior: B) -> Self {
        Self {
            behavior,
            config: SecretaryCoreConfig::default(),
        }
    }

    /// 设置轮询间隔
    /// Set polling interval
    pub fn with_poll_interval(mut self, ms: u64) -> Self {
        self.config.poll_interval_ms = ms;
        self
    }

    /// 设置是否发送欢迎消息
    /// Set whether to send a welcome message
    pub fn with_welcome(mut self, send: bool) -> Self {
        self.config.send_welcome = send;
        self
    }

    /// 设置是否启用定时检查
    /// Set whether to enable periodic check
    pub fn with_periodic_check(mut self, enabled: bool) -> Self {
        self.config.enable_periodic_check = enabled;
        self
    }

    /// 设置定时检查间隔
    /// Set periodic check interval
    pub fn with_periodic_check_interval(mut self, ms: u64) -> Self {
        self.config.periodic_check_interval_ms = ms;
        self
    }

    /// 设置最大连续错误次数
    /// Set maximum consecutive error count
    pub fn with_max_consecutive_errors(mut self, max: u32) -> Self {
        self.config.max_consecutive_errors = max;
        self
    }

    /// 构建秘书核心
    /// Build secretary core
    #[must_use]
    pub fn build(self) -> SecretaryCore<B> {
        SecretaryCore::with_config(self.behavior, self.config)
    }
}

// =============================================================================
// 测试
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = SecretaryCoreConfig::default();
        assert_eq!(config.poll_interval_ms, 100);
        assert!(config.send_welcome);
        assert!(config.enable_periodic_check);
    }

    #[test]
    fn test_handle() {
        let (tx, _rx) = mpsc::channel(1);
        let handle = SecretaryHandle::new(tx);

        assert!(!handle.is_running());
        assert!(!handle.is_paused());

        handle.pause();
        assert!(handle.is_paused());

        handle.resume();
        assert!(!handle.is_paused());
    }
}
