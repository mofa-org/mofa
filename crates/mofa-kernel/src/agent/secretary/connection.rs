//! 用户连接抽象
//! User Connection Abstraction
//!
//! 定义秘书与用户之间的通信接口，具体实现位于 mofa-foundation。
//! Defines the communication interface between the secretary and users, with specific implementations in mofa-foundation.

use super::error::ConnectionError;
use async_trait::async_trait;

// =============================================================================
// 用户连接 Trait
// User Connection Trait
// =============================================================================

/// 用户连接Trait
/// User Connection Trait
///
/// 定义秘书与用户之间的通信接口。
/// Defines the communication interface between the secretary and the user.
/// 不同的连接方式（WebSocket、TCP、Channel等）都可以实现这个trait。
/// Different connection methods (WebSocket, TCP, Channel, etc.) can implement this trait.
///
/// # 类型参数
/// # Type Parameters
///
/// - `Input`: 用户发送的输入类型
/// - `Input`: The type of input sent by the user
/// - `Output`: 秘书发送的输出类型
/// - `Output`: The type of output sent by the secretary
///
/// # 示例
/// # Example
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
///     async fn receive(&self) -> Result<Self::Input, ConnectionError> {
///         let msg = self.ws.recv().await
///             .map_err(|e| ConnectionError::ReceiveFailed(e.to_string()))?;
///         serde_json::from_str(&msg)
///             .map_err(|e| ConnectionError::Serialization(e.to_string()))
///     }
///
///     async fn send(&self, output: Self::Output) -> Result<(), ConnectionError> {
///         let json = serde_json::to_string(&output)
///             .map_err(|e| ConnectionError::Serialization(e.to_string()))?;
///         self.ws.send(json).await
///             .map_err(|e| ConnectionError::SendFailed(e.to_string()))
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
    /// User input type
    type Input: Send + 'static;

    /// 秘书输出类型
    /// Secretary output type
    type Output: Send + 'static;

    /// 接收用户输入（阻塞）
    /// Receive user input (blocking)
    async fn receive(&self) -> Result<Self::Input, ConnectionError>;

    /// 尝试接收用户输入（非阻塞）
    /// Try to receive user input (non-blocking)
    ///
    /// 返回 `Ok(Some(input))` 表示收到输入
    /// Returns `Ok(Some(input))` indicating input received
    /// 返回 `Ok(None)` 表示没有可用输入
    /// Returns `Ok(None)` indicating no available input
    /// 返回 `Err(e)` 表示发生错误
    /// Returns `Err(e)` indicating an error occurred
    async fn try_receive(&self) -> Result<Option<Self::Input>, ConnectionError>;

    /// 发送输出给用户
    /// Send output to the user
    async fn send(&self, output: Self::Output) -> Result<(), ConnectionError>;

    /// 检查连接是否有效
    /// Check if the connection is valid
    fn is_connected(&self) -> bool;

    /// 关闭连接
    /// Close the connection
    async fn close(&self) -> Result<(), ConnectionError> {
        Ok(())
    }
}

// =============================================================================
// 连接工厂
// Connection Factory
// =============================================================================

/// 连接工厂Trait
/// Connection Factory Trait
///
/// 用于创建连接实例
/// Used to create connection instances
#[async_trait]
pub trait ConnectionFactory: Send + Sync {
    /// 连接类型
    /// Connection type
    type Connection: UserConnection;

    /// 创建连接
    /// Create connection
    async fn create(&self) -> Result<Self::Connection, ConnectionError>;
}
