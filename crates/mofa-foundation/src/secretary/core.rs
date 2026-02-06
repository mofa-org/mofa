//! 秘书核心引擎实现
//!
//! 提供秘书Agent的核心事件循环机制。这是一个轻量级的引擎，
//! 只负责事件循环和消息传递，具体的行为逻辑由 `SecretaryBehavior` 实现。
//!
//! ## 设计理念
//!
//! - 最小化核心：只提供事件循环和连接管理
//! - 行为可插拔：通过 `SecretaryBehavior` trait 定义秘书行为
//! - 连接抽象：支持多种连接方式（通道、WebSocket 等）

use tokio::sync::mpsc;

// 使用 mofa-kernel 的核心抽象
use mofa_kernel::agent::secretary::{SecretaryBehavior, SecretaryContext, UserConnection};

// =============================================================================
// Core 配置与状态 (Foundation 实现)
// =============================================================================

/// 秘书核心配置
#[derive(Debug, Clone)]
pub struct SecretaryCoreConfig {
    /// 事件循环轮询间隔（毫秒）
    pub poll_interval_ms: u64,

    /// 是否在启动时发送欢迎消息
    pub send_welcome: bool,

    /// 是否启用定时检查
    pub enable_periodic_check: bool,

    /// 定时检查间隔（毫秒）
    pub periodic_check_interval_ms: u64,

    /// 最大连续错误次数（超过后停止）
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreState {
    /// 初始化中
    Initializing,
    /// 运行中
    Running,
    /// 暂停
    Paused,
    /// 已停止
    Stopped,
}

/// 秘书控制句柄
///
/// 用于从外部控制秘书的运行状态。
#[derive(Clone)]
pub struct SecretaryHandle {
    /// 是否运行中
    running: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// 是否暂停
    paused: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// 停止信号发送器
    stop_tx: mpsc::Sender<()>,
}

impl SecretaryHandle {
    /// 创建新的控制句柄
    pub fn new(stop_tx: mpsc::Sender<()>) -> Self {
        Self {
            running: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            paused: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            stop_tx,
        }
    }

    /// 获取 running 标志的克隆
    pub fn running_flag(&self) -> std::sync::Arc<std::sync::atomic::AtomicBool> {
        self.running.clone()
    }

    /// 获取 paused 标志的克隆
    pub fn paused_flag(&self) -> std::sync::Arc<std::sync::atomic::AtomicBool> {
        self.paused.clone()
    }

    /// 检查是否运行中
    pub fn is_running(&self) -> bool {
        self.running.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// 检查是否暂停
    pub fn is_paused(&self) -> bool {
        self.paused.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// 设置运行状态
    pub fn set_running(&self, running: bool) {
        self.running
            .store(running, std::sync::atomic::Ordering::SeqCst);
    }

    /// 暂停秘书
    pub fn pause(&self) {
        self.paused.store(true, std::sync::atomic::Ordering::SeqCst);
    }

    /// 恢复秘书
    pub fn resume(&self) {
        self.paused
            .store(false, std::sync::atomic::Ordering::SeqCst);
    }

    /// 停止秘书
    pub async fn stop(&self) {
        self.running
            .store(false, std::sync::atomic::Ordering::SeqCst);
        let _ = self.stop_tx.send(()).await;
    }
}

// =============================================================================
// 秘书核心引擎
// =============================================================================

/// 秘书核心引擎
///
/// 这是框架的核心组件，负责运行事件循环并协调各个组件。
/// 秘书的具体行为由 `SecretaryBehavior` 实现定义。
///
/// # 类型参数
///
/// - `B`: 秘书行为实现
///
/// # 示例
///
/// ```rust,ignore
/// // 1. 实现 SecretaryBehavior
/// struct MySecretary { /* ... */ }
/// impl SecretaryBehavior for MySecretary { /* ... */ }
///
/// // 2. 创建核心引擎
/// let core = SecretaryCore::new(MySecretary::new());
///
/// // 3. 创建连接
/// let (conn, input_tx, output_rx) = ChannelConnection::new_pair(32);
///
/// // 4. 启动事件循环
/// let handle = core.start(conn).await;
///
/// // 5. 发送输入
/// input_tx.send(MyInput::Text("hello".to_string())).await?;
///
/// // 6. 接收输出
/// while let Some(output) = output_rx.recv().await {
///     info!("Output: {:?}", output);
/// }
/// ```
pub struct SecretaryCore<B>
where
    B: SecretaryBehavior,
{
    /// 秘书行为实现
    behavior: B,

    /// 配置
    config: SecretaryCoreConfig,
}

impl<B> SecretaryCore<B>
where
    B: SecretaryBehavior + 'static,
{
    /// 创建新的秘书核心
    pub fn new(behavior: B) -> Self {
        Self {
            behavior,
            config: SecretaryCoreConfig::default(),
        }
    }

    /// 使用自定义配置创建
    pub fn with_config(behavior: B, config: SecretaryCoreConfig) -> Self {
        Self { behavior, config }
    }

    /// 获取配置的可变引用
    pub fn config_mut(&mut self) -> &mut SecretaryCoreConfig {
        &mut self.config
    }

    /// 获取行为实现的引用
    pub fn behavior(&self) -> &B {
        &self.behavior
    }

    /// 启动秘书（异步任务）
    ///
    /// 返回一个控制句柄和一个 JoinHandle。
    pub async fn start<C>(
        self,
        connection: C,
    ) -> (SecretaryHandle, tokio::task::JoinHandle<anyhow::Result<()>>)
    where
        C: UserConnection<Input = B::Input, Output = B::Output> + 'static,
    {
        let (stop_tx, stop_rx) = mpsc::channel(1);
        let handle = SecretaryHandle::new(stop_tx);
        let handle_clone = handle.clone();

        let join_handle = tokio::spawn(async move {
            
            self.run_event_loop(connection, handle_clone, stop_rx).await
        });

        (handle, join_handle)
    }

    /// 同步启动秘书（阻塞当前任务）
    pub async fn run<C>(self, connection: C) -> anyhow::Result<()>
    where
        C: UserConnection<Input = B::Input, Output = B::Output> + 'static,
    {
        let (stop_tx, stop_rx) = mpsc::channel(1);
        let handle = SecretaryHandle::new(stop_tx);
        self.run_event_loop(connection, handle, stop_rx).await
    }

    /// 运行事件循环
    async fn run_event_loop<C>(
        self,
        connection: C,
        handle: SecretaryHandle,
        mut stop_rx: mpsc::Receiver<()>,
    ) -> anyhow::Result<()>
    where
        C: UserConnection<Input = B::Input, Output = B::Output>,
    {
        // 标记为运行中
        handle.set_running(true);

        // 创建上下文
        let mut ctx = SecretaryContext::new(self.behavior.initial_state());

        // 发送欢迎消息
        if self.config.send_welcome
            && let Some(welcome) = self.behavior.welcome_message()
                && let Err(e) = connection.send(welcome).await {
                    tracing::warn!("Failed to send welcome message: {}", e);
                }

        let mut consecutive_errors = 0u32;
        let mut last_periodic_check = std::time::Instant::now();

        // 主事件循环
        loop {
            // 检查停止信号
            tokio::select! {
                _ = stop_rx.recv() => {
                    tracing::info!("Received stop signal");
                    break;
                }
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(self.config.poll_interval_ms)) => {
                    // 继续循环
                }
            }

            // 检查连接状态
            if !connection.is_connected() {
                tracing::info!("Connection closed");
                break;
            }

            // 检查是否暂停
            if handle.is_paused() {
                continue;
            }

            // 尝试接收用户输入
            match connection.try_receive().await {
                Ok(Some(input)) => {
                    consecutive_errors = 0;

                    // 处理输入
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
                            if let Some(error_output) = self.behavior.handle_error(&e) {
                                let _ = connection.send(error_output).await;
                            }
                        }
                    }
                }
                Ok(None) => {
                    // 没有输入，执行定时检查
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
        handle.set_running(false);

        // 调用断开连接回调
        if let Err(e) = self.behavior.on_disconnect(&mut ctx).await {
            tracing::warn!("Error in on_disconnect: {}", e);
        }

        Ok(())
    }
}

// =============================================================================
// 秘书核心构建器
// =============================================================================

/// 秘书核心构建器
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
    pub fn new(behavior: B) -> Self {
        Self {
            behavior,
            config: SecretaryCoreConfig::default(),
        }
    }

    /// 设置轮询间隔
    pub fn with_poll_interval(mut self, ms: u64) -> Self {
        self.config.poll_interval_ms = ms;
        self
    }

    /// 设置是否发送欢迎消息
    pub fn with_welcome(mut self, send: bool) -> Self {
        self.config.send_welcome = send;
        self
    }

    /// 设置是否启用定时检查
    pub fn with_periodic_check(mut self, enabled: bool) -> Self {
        self.config.enable_periodic_check = enabled;
        self
    }

    /// 设置定时检查间隔
    pub fn with_periodic_check_interval(mut self, ms: u64) -> Self {
        self.config.periodic_check_interval_ms = ms;
        self
    }

    /// 设置最大连续错误次数
    pub fn with_max_consecutive_errors(mut self, max: u32) -> Self {
        self.config.max_consecutive_errors = max;
        self
    }

    /// 构建秘书核心
    pub fn build(self) -> SecretaryCore<B> {
        SecretaryCore::with_config(self.behavior, self.config)
    }
}

// =============================================================================
// 测试
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
