//! 用户连接实现 (Foundation 层)
//! User connection implementation (Foundation layer)
//!
//! Kernel 仅定义 `UserConnection` 抽象，具体连接实现放在 Foundation 层。
//! Kernel only defines the `UserConnection` abstraction; specific implementations are placed in the Foundation layer.

use async_trait::async_trait;
use mofa_kernel::agent::secretary::{ConnectionError, UserConnection};
use tokio::sync::mpsc;

// =============================================================================
// 基于通道的连接实现
// Channel-based connection implementation
// =============================================================================

/// 基于 mpsc 通道的连接
/// Connection based on mpsc channels
///
/// 用于进程内通信，如测试或单机应用
/// Used for in-process communication, such as testing or standalone applications
pub struct ChannelConnection<I, O> {
    /// 输入接收器
    /// Input receiver
    input_rx: tokio::sync::Mutex<mpsc::Receiver<I>>,
    /// 输出发送器
    /// Output transmitter
    output_tx: mpsc::Sender<O>,
    /// 连接状态
    /// Connection status
    connected: std::sync::atomic::AtomicBool,
}

impl<I, O> ChannelConnection<I, O>
where
    I: Send + 'static,
    O: Send + 'static,
{
    /// 创建新的通道连接
    /// Create a new channel connection
    pub fn new(input_rx: mpsc::Receiver<I>, output_tx: mpsc::Sender<O>) -> Self {
        Self {
            input_rx: tokio::sync::Mutex::new(input_rx),
            output_tx,
            connected: std::sync::atomic::AtomicBool::new(true),
        }
    }

    /// 创建连接对
    /// Create a connection pair
    ///
    /// 返回 (connection, input_tx, output_rx)
    /// Returns (connection, input_tx, output_rx)
    /// - `connection`: 给秘书使用的连接
    /// - `connection`: Connection used by the secretary
    /// - `input_tx`: 用户用来发送输入
    /// - `input_tx`: Used by the user to send input
    /// - `output_rx`: 用户用来接收输出
    /// - `output_rx`: Used by the user to receive output
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

    async fn receive(&self) -> Result<Self::Input, ConnectionError> {
        let mut rx = self.input_rx.lock().await;
        rx.recv()
            .await
            .ok_or(ConnectionError::Closed)
    }

    async fn try_receive(&self) -> Result<Option<Self::Input>, ConnectionError> {
        let mut rx = self.input_rx.lock().await;
        match rx.try_recv() {
            Ok(input) => Ok(Some(input)),
            Err(mpsc::error::TryRecvError::Empty) => Ok(None),
            Err(mpsc::error::TryRecvError::Disconnected) => {
                self.connected
                    .store(false, std::sync::atomic::Ordering::SeqCst);
                Err(ConnectionError::Closed)
            }
        }
    }

    async fn send(&self, output: Self::Output) -> Result<(), ConnectionError> {
        self.output_tx
            .send(output)
            .await
            .map_err(|_| ConnectionError::SendFailed("Failed to send output".into()))
    }

    fn is_connected(&self) -> bool {
        self.connected.load(std::sync::atomic::Ordering::SeqCst) && !self.output_tx.is_closed()
    }

    async fn close(&self) -> Result<(), ConnectionError> {
        self.connected
            .store(false, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }
}

// =============================================================================
// 超时包装连接
// Timeout wrapper connection
// =============================================================================

/// 带超时的连接包装器
/// Connection wrapper with timeouts
pub struct TimeoutConnection<C> {
    /// 内部连接
    /// Inner connection
    inner: C,
    /// 接收超时（毫秒）
    /// Receive timeout (milliseconds)
    receive_timeout_ms: u64,
    /// 发送超时（毫秒）
    /// Send timeout (milliseconds)
    send_timeout_ms: u64,
}

impl<C> TimeoutConnection<C> {
    /// 创建带超时的连接
    /// Create a connection with timeouts
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

    async fn receive(&self) -> Result<Self::Input, ConnectionError> {
        tokio::time::timeout(
            tokio::time::Duration::from_millis(self.receive_timeout_ms),
            self.inner.receive(),
        )
        .await
        .map_err(|_| ConnectionError::Timeout)?
    }

    async fn try_receive(&self) -> Result<Option<Self::Input>, ConnectionError> {
        self.inner.try_receive().await
    }

    async fn send(&self, output: Self::Output) -> Result<(), ConnectionError> {
        tokio::time::timeout(
            tokio::time::Duration::from_millis(self.send_timeout_ms),
            self.inner.send(output),
        )
        .await
        .map_err(|_| ConnectionError::Timeout)?
    }

    fn is_connected(&self) -> bool {
        self.inner.is_connected()
    }

    async fn close(&self) -> Result<(), ConnectionError> {
        self.inner.close().await
    }
}

// =============================================================================
// 测试
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_channel_connection() {
        let (conn, input_tx, mut output_rx) = ChannelConnection::<String, String>::new_pair(10);

        // 发送输入
        // Send input
        input_tx.send("Hello".to_string()).await.unwrap();

        // 接收输入
        // Receive input
        let input = conn.receive().await.unwrap();
        assert_eq!(input, "Hello");

        // 发送输出
        // Send output
        conn.send("World".to_string()).await.unwrap();

        // 接收输出
        // Receive output
        let output = output_rx.recv().await.unwrap();
        assert_eq!(output, "World");

        assert!(conn.is_connected());
    }

    #[tokio::test]
    async fn test_try_receive() {
        let (conn, _input_tx, _output_rx) = ChannelConnection::<String, String>::new_pair(10);

        // 没有输入时返回 None
        // Returns None when there is no input
        let result = conn.try_receive().await.unwrap();
        assert!(result.is_none());
    }
}
