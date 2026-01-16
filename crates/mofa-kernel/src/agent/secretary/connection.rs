//! 用户连接抽象
//!
//! 定义秘书与用户之间的通信接口

use async_trait::async_trait;
use tokio::sync::mpsc;

// =============================================================================
// 用户连接 Trait
// =============================================================================

/// 用户连接Trait
///
/// 定义秘书与用户之间的通信接口。
/// 不同的连接方式（WebSocket、TCP、Channel等）都可以实现这个trait。
///
/// # 类型参数
///
/// - `Input`: 用户发送的输入类型
/// - `Output`: 秘书发送的输出类型
///
/// # 示例
///
/// ```rust,ignore
/// struct WebSocketConnection {
///     ws: WebSocket,
/// }
///
/// #[async_trait]
/// impl UserConnection for WebSocketConnection {
///     type Input = UserMessage;
///     type Output = SecretaryResponse;
///
///     async fn receive(&self) -> anyhow::Result<Self::Input> {
///         let msg = self.ws.recv().await?;
///         Ok(serde_json::from_str(&msg)?)
///     }
///
///     async fn send(&self, output: Self::Output) -> anyhow::Result<()> {
///         self.ws.send(serde_json::to_string(&output)?).await?;
///         Ok(())
///     }
///
///     fn is_connected(&self) -> bool {
///         self.ws.is_open()
///     }
/// }
/// ```
#[async_trait]
pub trait UserConnection: Send + Sync {
    /// 用户输入类型
    type Input: Send + 'static;

    /// 秘书输出类型
    type Output: Send + 'static;

    /// 接收用户输入（阻塞）
    async fn receive(&self) -> anyhow::Result<Self::Input>;

    /// 尝试接收用户输入（非阻塞）
    ///
    /// 返回 `Ok(Some(input))` 表示收到输入
    /// 返回 `Ok(None)` 表示没有可用输入
    /// 返回 `Err(e)` 表示发生错误
    async fn try_receive(&self) -> anyhow::Result<Option<Self::Input>>;

    /// 发送输出给用户
    async fn send(&self, output: Self::Output) -> anyhow::Result<()>;

    /// 检查连接是否有效
    fn is_connected(&self) -> bool;

    /// 关闭连接
    async fn close(&self) -> anyhow::Result<()> {
        Ok(())
    }
}

// =============================================================================
// 基于通道的连接实现
// =============================================================================

/// 基于 mpsc 通道的连接
///
/// 用于进程内通信，如测试或单机应用
pub struct ChannelConnection<I, O> {
    /// 输入接收器
    input_rx: tokio::sync::Mutex<mpsc::Receiver<I>>,
    /// 输出发送器
    output_tx: mpsc::Sender<O>,
    /// 连接状态
    connected: std::sync::atomic::AtomicBool,
}

impl<I, O> ChannelConnection<I, O>
where
    I: Send + 'static,
    O: Send + 'static,
{
    /// 创建新的通道连接
    pub fn new(input_rx: mpsc::Receiver<I>, output_tx: mpsc::Sender<O>) -> Self {
        Self {
            input_rx: tokio::sync::Mutex::new(input_rx),
            output_tx,
            connected: std::sync::atomic::AtomicBool::new(true),
        }
    }

    /// 创建连接对
    ///
    /// 返回 (connection, input_tx, output_rx)
    /// - `connection`: 给秘书使用的连接
    /// - `input_tx`: 用户用来发送输入
    /// - `output_rx`: 用户用来接收输出
    pub fn new_pair(buffer_size: usize) -> (Self, mpsc::Sender<I>, mpsc::Receiver<O>) {
        let (input_tx, input_rx) = mpsc::channel(buffer_size);
        let (output_tx, output_rx) = mpsc::channel(buffer_size);

        let conn = Self::new(input_rx, output_tx);
        (conn, input_tx, output_rx)
    }
}

#[async_trait]
impl<I, O> UserConnection for ChannelConnection<I, O>
where
    I: Send + 'static,
    O: Send + 'static,
{
    type Input = I;
    type Output = O;

    async fn receive(&self) -> anyhow::Result<Self::Input> {
        let mut rx = self.input_rx.lock().await;
        rx.recv()
            .await
            .ok_or_else(|| anyhow::anyhow!("Channel closed"))
    }

    async fn try_receive(&self) -> anyhow::Result<Option<Self::Input>> {
        let mut rx = self.input_rx.lock().await;
        match rx.try_recv() {
            Ok(input) => Ok(Some(input)),
            Err(mpsc::error::TryRecvError::Empty) => Ok(None),
            Err(mpsc::error::TryRecvError::Disconnected) => {
                self.connected
                    .store(false, std::sync::atomic::Ordering::SeqCst);
                Err(anyhow::anyhow!("Channel disconnected"))
            }
        }
    }

    async fn send(&self, output: Self::Output) -> anyhow::Result<()> {
        self.output_tx
            .send(output)
            .await
            .map_err(|_| anyhow::anyhow!("Failed to send output"))
    }

    fn is_connected(&self) -> bool {
        self.connected
            .load(std::sync::atomic::Ordering::SeqCst)
            && !self.output_tx.is_closed()
    }

    async fn close(&self) -> anyhow::Result<()> {
        self.connected
            .store(false, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }
}

// =============================================================================
// 超时包装连接
// =============================================================================

/// 带超时的连接包装器
pub struct TimeoutConnection<C> {
    /// 内部连接
    inner: C,
    /// 接收超时（毫秒）
    receive_timeout_ms: u64,
    /// 发送超时（毫秒）
    send_timeout_ms: u64,
}

impl<C> TimeoutConnection<C> {
    /// 创建带超时的连接
    pub fn new(inner: C, receive_timeout_ms: u64, send_timeout_ms: u64) -> Self {
        Self {
            inner,
            receive_timeout_ms,
            send_timeout_ms,
        }
    }
}

#[async_trait]
impl<C> UserConnection for TimeoutConnection<C>
where
    C: UserConnection,
{
    type Input = C::Input;
    type Output = C::Output;

    async fn receive(&self) -> anyhow::Result<Self::Input> {
        tokio::time::timeout(
            tokio::time::Duration::from_millis(self.receive_timeout_ms),
            self.inner.receive(),
        )
        .await
        .map_err(|_| anyhow::anyhow!("Receive timeout"))?
    }

    async fn try_receive(&self) -> anyhow::Result<Option<Self::Input>> {
        self.inner.try_receive().await
    }

    async fn send(&self, output: Self::Output) -> anyhow::Result<()> {
        tokio::time::timeout(
            tokio::time::Duration::from_millis(self.send_timeout_ms),
            self.inner.send(output),
        )
        .await
        .map_err(|_| anyhow::anyhow!("Send timeout"))?
    }

    fn is_connected(&self) -> bool {
        self.inner.is_connected()
    }

    async fn close(&self) -> anyhow::Result<()> {
        self.inner.close().await
    }
}

// =============================================================================
// 连接工厂
// =============================================================================

/// 连接工厂Trait
///
/// 用于创建连接实例
#[async_trait]
pub trait ConnectionFactory: Send + Sync {
    /// 连接类型
    type Connection: UserConnection;

    /// 创建连接
    async fn create(&self) -> anyhow::Result<Self::Connection>;
}

// =============================================================================
// 测试
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_channel_connection() {
        let (conn, input_tx, mut output_rx) =
            ChannelConnection::<String, String>::new_pair(10);

        // 发送输入
        input_tx.send("Hello".to_string()).await.unwrap();

        // 接收输入
        let input = conn.receive().await.unwrap();
        assert_eq!(input, "Hello");

        // 发送输出
        conn.send("World".to_string()).await.unwrap();

        // 接收输出
        let output = output_rx.recv().await.unwrap();
        assert_eq!(output, "World");

        assert!(conn.is_connected());
    }

    #[tokio::test]
    async fn test_try_receive() {
        let (conn, _input_tx, _output_rx) =
            ChannelConnection::<String, String>::new_pair(10);

        // 没有输入时返回 None
        let result = conn.try_receive().await.unwrap();
        assert!(result.is_none());
    }
}
